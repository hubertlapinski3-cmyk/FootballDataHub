"""Fetch raw WhoScored match data for the seven covered leagues (step 1 of the nightly ingest).

Run on the self-hosted runner, where a real Chromium on a normal connection gets past the bot wall that
blocks GitHub's own runners. For every league in scripts/leagues.json it

  1. opens the tournament page and collects match ids (the page lists recent results; with --backfill it
     also walks the monthly fixtures feed from the start of the season),
  2. fetches each id not yet in <out>/<league>/ with the pinned scraper, keeps only finished matches,
  3. saves matchCentreData as <out>/<league>/<match_id>.json.gz (the format `matchhub ingest` reads).

Step 2 of the ingest (`matchhub ingest`) turns those files into packets, the catalog and the scouting index.

usage: python scripts/ingest_leagues.py --out work/raw [--league slug] [--max-matches N] [--backfill]
       python scripts/ingest_leagues.py --selftest
"""

import argparse
import datetime
import gzip
import json
import pathlib
import re
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
FINISHED = ("FT", "AET", "PEN", "AP")


def match_ids_from_html(html: str) -> list[str]:
    """Match ids linked from a tournament page (/matches/<id>/live|show|matchreport/...)."""
    seen, out = set(), []
    for m in re.finditer(r"(?i)/matches/(\d{6,9})/(?:live|show|matchreport|preview)", html):
        if m.group(1) not in seen:
            seen.add(m.group(1))
            out.append(m.group(1))
    return out


def stage_id_from_html(html: str):
    m = re.search(r"""(?i)["']?stageId["']?\s*[:=]\s*["']?(\d{3,8})""", html)
    return m.group(1) if m else None


def match_ids_from_feed(text: str) -> list[str]:
    """Candidate ids from a monthly fixtures feed: the first number of each inner array. The feed is
    not strict JSON; candidates are verified later by fetching, so over-matching only costs a skip."""
    seen, out = set(), []
    for m in re.finditer(r"\[\s*(\d{6,9})\s*,", text):
        if m.group(1) not in seen:
            seen.add(m.group(1))
            out.append(m.group(1))
    return out


def months_since(season_start: datetime.date, today: datetime.date) -> list[str]:
    y, m, out = season_start.year, season_start.month, []
    while (y, m) <= (today.year, today.month):
        out.append(f"{y}{m:02d}")
        y, m = (y + 1, 1) if m == 12 else (y, m + 1)
    return out


def is_finished(data: dict) -> bool:
    """A match that is still being played has partial events: only keep finished ones."""
    el = str(data.get("elapsed", "")).upper()
    return not el or any(el.startswith(f) for f in FINISHED)


def selftest() -> int:
    html = '<a href="/matches/1900123/live/england-premier-league-2026-2027-x-y">x</a><a href="/Matches/1900124/Show/z">y</a> {stageId: 24533, other: 1}'
    assert match_ids_from_html(html) == ["1900123", "1900124"], match_ids_from_html(html)
    assert stage_id_from_html(html) == "24533"
    assert stage_id_from_html('"stageId":"9001"') == "9001"
    assert match_ids_from_feed("[[1900123,1,'a'],[1900124,3,'b'],[12,1]]") == ["1900123", "1900124"]
    assert months_since(datetime.date(2026, 8, 1), datetime.date(2026, 11, 26)) == ["202608", "202609", "202610", "202611"]
    assert months_since(datetime.date(2026, 11, 1), datetime.date(2027, 2, 3)) == ["202611", "202612", "202701", "202702"]
    assert is_finished({"elapsed": "FT"}) and is_finished({}) and not is_finished({"elapsed": "67"})
    cfg = json.loads((HERE / "leagues.json").read_text())
    assert len(cfg["leagues"]) == 7 and len({l["slug"] for l in cfg["leagues"]}) == 7
    for l in cfg["leagues"]:
        assert "verify" not in l, f"{l['slug']} is still unverified"
        m = re.search(r"(?i)/regions/(\d+)/tournaments/(\d+)", l["url"])
        assert m and (int(m.group(1)), int(m.group(2))) == (l["region_id"], l["tournament_id"]), l["slug"]
        assert l["url"].startswith("https://www.whoscored.com/"), l["slug"]
        if "stage_id" in l:
            assert f"/stages/{l['stage_id']}/" in l["url"] and f"/seasons/{l['season_id']}/" in l["url"], l["slug"]
    print("selftest ok")
    return 0


def discover(page, league: dict, backfill: bool, season_start: datetime.date) -> list[str]:
    page.goto(league["url"], timeout=60_000, wait_until="domcontentloaded")
    page.wait_for_timeout(3000)
    html = page.content()
    ids = match_ids_from_html(html)
    if backfill:
        stage = league.get("stage_id") or stage_id_from_html(html)
        if not stage:
            print(f"  {league['slug']}: no stage id on the tournament page; backfill limited to the page's recent results", file=sys.stderr)
        else:
            for ym in months_since(season_start, datetime.date.today()):
                url = f"/tournamentsfeed/{stage}/Fixtures/?d={ym}&isAggregate=false"
                try:
                    text = page.evaluate("async (u) => { const r = await fetch(u, {headers: {'X-Requested-With': 'XMLHttpRequest'}}); return await r.text(); }", url)
                    ids += [i for i in match_ids_from_feed(text) if i not in ids]
                except Exception as e:  # a missing month is not fatal
                    print(f"  {league['slug']} {ym}: {e}", file=sys.stderr)
                time.sleep(1.5)
    return ids


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="work/raw")
    ap.add_argument("--league", default="all")
    ap.add_argument("--max-matches", type=int, default=0, help="stop after this many newly fetched matches (0 = no limit)")
    ap.add_argument("--backfill", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()
    if args.selftest:
        return selftest()

    # the pinned scraper is installed by the workflow (see ingest-leagues.yml)
    from football_match_report.io.whoscored import UA, _extract_match_centredata
    from playwright.sync_api import sync_playwright

    def fetch_match(page, url):
        """Same extraction as scrape_whoscored.fetch_match, on the open page (one browser for the whole run)."""
        resp = page.goto(url, timeout=60_000, wait_until="domcontentloaded")
        try:
            page.wait_for_function("document.documentElement.innerHTML.includes('matchCentreData')", timeout=30_000)
        except Exception:
            pass
        page.wait_for_timeout(1500)
        html = page.content()
        data = _extract_match_centredata(html)
        if data is None:
            low = html.lower()
            wall = any(k in low for k in ("incapsula", "captcha", "access denied", "request unsuccessful"))
            raise RuntimeError(f"HTTP {resp.status if resp else '?'}: " + ("bot wall" if wall else "no matchCentreData (not played yet or layout change)"))
        return data

    cfg = json.loads((HERE / "leagues.json").read_text())
    start_year = int(cfg["season"].split("-")[0])
    season_start = datetime.date(start_year, 8, 1)
    leagues = [l for l in cfg["leagues"] if args.league in ("all", l["slug"])]
    if not leagues:
        print(f"unknown league {args.league!r}", file=sys.stderr)
        return 2
    out = pathlib.Path(args.out)
    fetched, failed = 0, 0
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True, args=["--disable-blink-features=AutomationControlled"])
        ctx = browser.new_context(user_agent=UA, locale="en-GB", viewport={"width": 1366, "height": 900})
        page = ctx.new_page()
        for lg in leagues:
            d = out / lg["slug"]
            d.mkdir(parents=True, exist_ok=True)
            try:
                ids = discover(page, lg, args.backfill, season_start)
            except Exception as e:
                print(f"{lg['slug']}: discovery failed: {e}", file=sys.stderr)
                failed += 1
                continue
            new = [i for i in ids if not (d / f"{i}.json.gz").exists()]
            print(f"{lg['slug']}: {len(ids)} candidate ids, {len(new)} new")
            for i in new:
                if args.max_matches and fetched >= args.max_matches:
                    break
                try:
                    data = fetch_match(page, f"https://www.whoscored.com/Matches/{i}/Live")
                    data["matchId"] = int(i)
                    if not data.get("events") or not is_finished(data):
                        print(f"  {i}: not finished or no events, skipped")
                        continue
                    with gzip.open(d / f"{i}.json.gz", "wt", encoding="utf-8") as fh:
                        json.dump(data, fh, separators=(",", ":"))
                    fetched += 1
                    print(f"  {i}: saved")
                except Exception as e:
                    failed += 1
                    print(f"  {i}: {e}", file=sys.stderr)
                time.sleep(2.0)  # one polite request at a time
            if args.max_matches and fetched >= args.max_matches:
                break
        browser.close()
    print(f"fetched {fetched} new matches, {failed} failures")
    # nothing new and failures only: let the workflow show it
    return 1 if fetched == 0 and failed > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
