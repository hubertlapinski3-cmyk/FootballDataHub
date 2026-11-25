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

from football_match_report.io.whoscored import fetch_match


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
