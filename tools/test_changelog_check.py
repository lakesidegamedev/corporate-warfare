import tempfile
import unittest
from pathlib import Path

import changelog_check as cc

GOOD = """# Changelog

## [Unreleased]

## [1.1.0] - 2026-11-01

## [1.0.0] - 2026-10-25
"""


def repo(changelog, cargo="1.1.0", ios="1.1.0"):
    root = Path(tempfile.mkdtemp())
    (root / "apple").mkdir()
    (root / "CHANGELOG.md").write_text(changelog)
    (root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{cargo}"\n')
    (root / "apple/project.yml").write_text(f'settings:\n  MARKETING_VERSION: "{ios}"\n')
    return root


class ChangelogCheck(unittest.TestCase):
    def test_a_consistent_repository_passes(self):
        self.assertEqual(cc.check(repo(GOOD)), [])

    def test_versions_must_match_the_newest_release(self):
        errors = cc.check(repo(GOOD, cargo="1.0.0"))
        self.assertTrue(any("Cargo.toml" in e for e in errors))
        errors = cc.check(repo(GOOD, ios="1.2.0"))
        self.assertTrue(any("project.yml" in e for e in errors))

    def test_unreleased_and_ordering_are_required(self):
        errors = cc.check(repo(GOOD.replace("## [Unreleased]\n", "")))
        self.assertIn("missing ## [Unreleased] section", errors)
        swapped = "# C\n\n## [Unreleased]\n\n## [1.0.0] - 2026-10-25\n\n## [1.1.0] - 2026-11-01\n"
        self.assertTrue(any("newest first" in e for e in cc.check(repo(swapped))))
        undated = "# C\n\n## [Unreleased]\n\n## [1.1.0]\n"
        self.assertTrue(any("needs a date" in e for e in cc.check(repo(undated))))

    def test_code_changes_need_a_changelog_entry(self):
        root = repo(GOOD)
        self.assertTrue(cc.check(root, ["crates/cw-sim/src/lib.rs"]))
        self.assertEqual(cc.check(root, ["crates/cw-sim/src/lib.rs", "CHANGELOG.md"]), [])
        self.assertEqual(cc.check(root, ["docs/planning/STATUS.md"]), [])
        self.assertEqual(cc.check(root, ["apple/Sources/X.swift"], "Notes\nchangelog: none\n"), [])


if __name__ == "__main__":
    unittest.main()
