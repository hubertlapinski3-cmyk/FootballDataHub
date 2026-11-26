"""Build the trimmed Reep player lookup bundled with Data Hub.

Source: withqwerty/reep v0 register (CC0 1.0), frozen 2026-06-21.
Keeps players with a date of birth: names (primary name, full name and aliases),
date of birth, nationality, position and WhoScored id.

usage: python scripts/trim_reep.py <reep/data dir> crates/hub-core/data/reep_players.tsv.gz
"""

import csv
import gzip
import sys
from collections import defaultdict

src, out = sys.argv[1], sys.argv[2]
alias = defaultdict(set)
with open(f"{src}/names.csv", encoding="utf-8") as fh:
    for r in csv.DictReader(fh):
        if r["alias"]:
            alias[r["reep_id"]].add(r["alias"].strip())

n = 0
with open(f"{src}/people.csv", encoding="utf-8") as fh, gzip.open(out, "wt", encoding="utf-8", compresslevel=9) as w:
    w.write("names\tdob\tnationality\tposition\twhoscored\n")
    for r in csv.DictReader(fh):
        if r["type"] != "player" or not r["date_of_birth"]:
            continue
        names = [r["name"].strip()]
        for x in [r["full_name"].strip(), *sorted(alias.get(r["reep_id"], ()))]:
            if x and x not in names:
                names.append(x)
        names = [x.replace("\t", " ").replace("|", " ") for x in names if x]
        if not names:
            continue
        row = ["|".join(names), r["date_of_birth"], r["nationality"], r["position"], r["key_whoscored"]]
        w.write("\t".join(c.replace("\t", " ").replace("\n", " ") for c in row) + "\n")
        n += 1
print(n, "players")
