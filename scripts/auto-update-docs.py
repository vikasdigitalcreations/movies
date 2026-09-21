"""Keeps the docs true after the auto-update workflow re-vendors MovieBox-Tui.

    python scripts/auto-update-docs.py --old 0.1.22 --new 0.1.23 --next 1.3.3 \
        --before 15 --after 15 --total 17 --notes-file upstream-notes.txt

Only the files that state the vendored version as a current fact are rewritten
(README, CLAUDE, SETUP_GUIDE, BUILD, TECHNICAL, MASTERPLAN). PROGRESS, CHANGELOG and
API keep older versions on purpose -- they describe history -- so they are left alone
except for the new CHANGELOG entry this script adds.
"""
import argparse
import datetime
import os
import re
import sys

CURRENT_FACT_FILES = ["README.md", "CLAUDE.md", "SETUP_GUIDE.md", "BUILD.md", "TECHNICAL.md", "MASTERPLAN.md"]
CHANGELOG_ANCHOR = "Newest first. Dates are the day the work landed.\n"


def read(path):
    """The text with LF endings, and whether the file used CRLF so it is written back the same way."""
    with open(path, encoding="utf-8", newline="") as f:
        raw = f.read()
    return raw.replace("\r\n", "\n"), "\r\n" in raw


def write(path, text, crlf):
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text.replace("\n", "\r\n") if crlf else text)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--old", required=True, help="vendored version being replaced, e.g. 0.1.22")
    ap.add_argument("--new", required=True, help="upstream version now vendored, e.g. 0.1.23")
    ap.add_argument("--next", required=True, help="MovieBox version this ships as, e.g. 1.3.3")
    ap.add_argument("--before", required=True, type=int, help="survey: playable titles with the old vendor")
    ap.add_argument("--after", required=True, type=int, help="survey: playable titles with the new vendor")
    ap.add_argument("--total", required=True, type=int, help="survey: titles sampled")
    ap.add_argument("--notes-file", default="", help="upstream release notes, quoted in the changelog")
    ap.add_argument("--root", default=".", help="repository root")
    ap.add_argument("--date", default=datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d"))
    a = ap.parse_args()
    os.chdir(a.root)

    if a.old != a.new:
        for path in CURRENT_FACT_FILES:
            if not os.path.exists(path):
                continue
            text, crlf = read(path)
            # "v0.1.22" only: a bare "0.1.22" could be part of some other number.
            updated = re.sub(rf"(?<![\d.])v{re.escape(a.old)}(?![\d.])", f"v{a.new}", text)
            if updated != text:
                write(path, updated, crlf)
                print(f"updated {path}")

    upstream = ""
    if a.notes_file and os.path.exists(a.notes_file):
        with open(a.notes_file, encoding="utf-8", errors="replace") as f:
            lines = [ln.rstrip() for ln in f.read().splitlines() if ln.strip()]
        upstream = "\n".join(f"  > {ln}" for ln in lines[:12])

    entry = [
        f"## {a.date} — {a.next} (automatic)",
        "",
        "### Changed",
        f"- Vendored MovieBox-Tui v{a.old} → v{a.new}, a clean tree replacement made by the "
        "auto-update workflow (`.github/workflows/auto-update.yml`). Before publishing it ran the "
        f"unit tests, type-checked the UI and surveyed {a.total} popular titles: {a.before} played "
        f"with v{a.old}, {a.after} play with v{a.new}. It also downloaded and muxed a sample through "
        "the DASH downloader.",
    ]
    if upstream:
        entry += ["- What upstream says changed:", upstream]
    entry.append("")

    log, crlf = read("CHANGELOG.md")
    if CHANGELOG_ANCHOR not in log:
        print("CHANGELOG.md has no anchor line; entry not added", file=sys.stderr)
        return 1
    log = log.replace(CHANGELOG_ANCHOR, CHANGELOG_ANCHOR + "\n" + "\n".join(entry), 1)
    write("CHANGELOG.md", log, crlf)
    print("added the CHANGELOG entry")
    return 0


if __name__ == "__main__":
    sys.exit(main())
