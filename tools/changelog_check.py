#!/usr/bin/env python3
"""Changelog and version gate.

- `CHANGELOG.md` has an `## [Unreleased]` section and released headings `## [X.Y.Z] - DATE`
  in descending order.
- The workspace version (`Cargo.toml`) and the iOS marketing version (`apple/project.yml`)
  equal the newest released heading.
- With `--base <ref>`, a change set touching code, content, tooling or workflows must also
  change `CHANGELOG.md`, unless the pull request body says `changelog: none` (pass it with
  `--body-file`).
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GATED = ("crates/", "apple/", "compose/", "content/", "tools/", ".github/workflows/")
HEADING = re.compile(r"^## \[(Unreleased|\d+\.\d+\.\d+)\](?: - (\d{4}-\d{2}-\d{2}))?\s*$")


def parse(text):
    """Return (has_unreleased, released versions as tuples in file order, errors)."""
    errors = []
    unreleased = False
    versions = []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.startswith("## "):
            continue
        match = HEADING.match(line)
        if not match:
            errors.append(f"line {number}: malformed heading {line!r}")
            continue
        if match.group(1) == "Unreleased":
            if versions:
                errors.append("Unreleased must come before every released version")
            unreleased = True
        else:
            if not match.group(2):
                errors.append(f"line {number}: version {match.group(1)} needs a date")
            versions.append(tuple(int(part) for part in match.group(1).split(".")))
    if not unreleased:
        errors.append("missing ## [Unreleased] section")
    if versions != sorted(versions, reverse=True) or len(set(versions)) != len(versions):
        errors.append("released versions must be unique and newest first")
    return unreleased, versions, errors


def workspace_version(root):
    text = (root / "Cargo.toml").read_text()
    match = re.search(r'^\[workspace\.package\][^\[]*?^version\s*=\s*"([^"]+)"', text, re.M | re.S)
    if not match:
        match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    return match.group(1) if match else None


def marketing_version(root):
    match = re.search(r'MARKETING_VERSION:\s*"([^"]+)"', (root / "apple/project.yml").read_text())
    return match.group(1) if match else None


def gated(paths):
    return [p for p in paths if p.startswith(GATED)]


def check(root, changed=None, body=""):
    errors = []
    _, versions, parse_errors = parse((root / "CHANGELOG.md").read_text())
    errors += parse_errors
    if versions:
        newest = ".".join(map(str, versions[0]))
        for label, value in (("Cargo.toml", workspace_version(root)), ("apple/project.yml", marketing_version(root))):
            if value != newest:
                errors.append(f"{label} version {value} does not match newest changelog release {newest}")
    if changed is not None and gated(changed) and "CHANGELOG.md" not in changed:
        if "changelog: none" not in body.lower():
            errors.append("code, content or tooling changed without a CHANGELOG.md entry "
                          "(add one under Unreleased, or write 'changelog: none' in the PR body)")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="git ref to diff against (pull requests)")
    parser.add_argument("--body-file", help="file holding the pull request body")
    args = parser.parse_args()
    changed = None
    if args.base:
        output = subprocess.run(["git", "diff", "--name-only", f"{args.base}...HEAD"],
                                cwd=ROOT, check=True, capture_output=True, text=True).stdout
        changed = [line for line in output.splitlines() if line]
    body = Path(args.body_file).read_text() if args.body_file and Path(args.body_file).exists() else ""
    errors = check(ROOT, changed, body)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Changelog and versions consistent.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
