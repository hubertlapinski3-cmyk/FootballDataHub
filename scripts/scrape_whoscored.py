"""Scrape one WhoScored match with headless Chromium and store matchCentreData.

Uses the Playwright scraper from ricardoherediaj/football-analytics-tutorials
(`football_match_report.io.whoscored.fetch_match`), pinned in the
scrape-whoscored workflow. Output: <out_dir>/<match_id>.json.gz, which the
Data Hub app and CLI read through the GitHub relay.

usage: python scripts/scrape_whoscored.py <whoscored url or id> <out_dir>
"""

import gzip
import json
import pathlib
import re
import sys
from urllib.parse import urlparse

from football_match_report.io.whoscored import UA, _extract_match_centredata


def fetch_match(url: str) -> dict:
    """Headless Chromium fetch with diagnostics (what the page actually was) on failure."""
    from playwright.sync_api import sync_playwright

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True, args=["--disable-blink-features=AutomationControlled"])
        ctx = browser.new_context(user_agent=UA, locale="en-GB", viewport={"width": 1366, "height": 900})
        page = ctx.new_page()
        resp = page.goto(url, timeout=60_000, wait_until="domcontentloaded")
        try:
            page.wait_for_function("document.documentElement.innerHTML.includes('matchCentreData')", timeout=30_000)
        except Exception:
            pass
        page.wait_for_timeout(2000)
        html = page.content()
        title = page.title()
        status = resp.status if resp else None
        browser.close()
    data = _extract_match_centredata(html)
    if data is None:
        low = html.lower()
        hint = "bot wall (Incapsula/captcha)" if any(k in low for k in ("incapsula", "captcha", "access denied", "request unsuccessful")) else "no matchCentreData in page (match not played yet, wrong id or layout change)"
        print(f"scrape failed: HTTP {status}, title={title!r}, html={len(html)} bytes, likely: {hint}", file=sys.stderr)
        print("page start: " + re.sub(r"\s+", " ", html[:400]), file=sys.stderr)
        raise SystemExit(4)
    return data


def main() -> int:
    raw = sys.argv[1].strip()
    out = pathlib.Path(sys.argv[2])
    if raw.isdigit():
        match_id = raw
        url = f"https://www.whoscored.com/Matches/{match_id}/Live"
    else:
        host = urlparse(raw).hostname or ""
        if not host.endswith("whoscored.com"):
            print(f"refusing non-WhoScored URL host: {host!r}", file=sys.stderr)
            return 2
        m = re.search(r"(?i)/matches/(\d+)", raw)
        if not m:
            print("no match id in URL", file=sys.stderr)
            return 2
        match_id = m.group(1)
        url = raw

    data = fetch_match(url)  # headless Chromium, waits for the match centre
    data["matchId"] = int(match_id)
    if not data.get("events"):
        print("matchCentreData has no events (match not played yet?)", file=sys.stderr)
        return 3

    out.mkdir(parents=True, exist_ok=True)
    path = out / f"{match_id}.json.gz"
    with gzip.open(path, "wt", encoding="utf-8") as fh:
        json.dump(data, fh, separators=(",", ":"))
    print(match_id)
    return 0


if __name__ == "__main__":
    sys.exit(main())
