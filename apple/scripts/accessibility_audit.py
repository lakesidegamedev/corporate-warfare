#!/usr/bin/env python3
"""Static accessibility audit for the SwiftUI sources.

This is a review aid, not a human accessibility sign-off. It flags patterns that commonly break
Dynamic Type, VoiceOver or 44pt targets so a reviewer can inspect them on a device.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCES = ROOT / "apple/Sources"


def main() -> int:
    files = sorted(SOURCES.rglob("*.swift"))
    fixed_fonts = []
    line_limits = []
    narrow_frames = []
    for path in files:
        body = path.read_text(errors="ignore")
        for number, line in enumerate(body.splitlines(), 1):
            if re.search(r"\.font\(\.system\(size:", line):
                fixed_fonts.append(f"{path.name}:{number}")
            if ".lineLimit(1)" in line:
                line_limits.append(f"{path.name}:{number}")
            match = re.search(r"frame\(width:\s*(\d+)", line)
            if match and int(match.group(1)) < 44:
                narrow_frames.append(f"{path.name}:{number} ({match.group(1)}pt)")
    dynamic_type = sum(
        1 for path in files if ".dynamicTypeSize" in path.read_text(errors="ignore")
    )
    labels = sum(
        path.read_text(errors="ignore").count("accessibilityLabel")
        for path in files
    )
    hints = sum(
        path.read_text(errors="ignore").count("accessibilityHint")
        for path in files
    )
    print(f"swift_files={len(files)} accessibility_labels={labels} hints={hints} dynamic_type_guards={dynamic_type}")
    for title, rows in (
        ("fixed font sizes (prefer text styles)", fixed_fonts),
        ("single-line limits (check truncation)", line_limits),
        ("frames narrower than 44pt", narrow_frames),
    ):
        print(f"{title}: {len(rows)}")
        for row in rows[:12]:
            print(f"  {row}")
    # Advisory only: nothing here certifies VoiceOver order, contrast, motion or AX text layout.
    return 0


if __name__ == "__main__":
    sys.exit(main())
