# Versioning and release procedure

## Version numbers

Corporate Warfare uses `MAJOR.MINOR.PATCH`. The owner's rule decides which digit moves:

| Change | Digit | Example |
|---|---|---|
| Minor feature updates, fixes, balance and copy changes | **PATCH** | 1.0.0 to 1.0.1 |
| A big feature: a new system, screen family or mechanic players will notice | **MINOR** | 1.0.1 to 1.1.0 |
| A wide feature release or upgrade: a redesign, a new game model, an incompatible save | **MAJOR** | 1.1.0 to 2.0.0 |

When one release mixes sizes, the largest change decides. The owner has the final say on
the digit. Breaking save compatibility is a strong signal for a larger bump, and the changelog
must say so whenever a release cannot read the previous release's saves.

One version number covers the workspace crates (`Cargo.toml`), the engine version reported by
`cw-cli doctor`, and the iOS marketing version (`apple/project.yml`). CI checks that they agree
with the newest released heading in `CHANGELOG.md`. The iOS build number is separate and
increases with every upload.

## Changelog

- Every pull request that touches `crates/`, `apple/`, `content/`, `tools/` or
  `.github/workflows/` adds an entry under `## [Unreleased]` in `CHANGELOG.md`. CI enforces
  this. A pull request that genuinely needs no entry (for example a comment-only change) says
  `changelog: none` in its description and the check accepts it.
- Group entries under Keep a Changelog headings: **Added**, **Changed**, **Deprecated**,
  **Removed**, **Fixed**, **Security**.
- Write for a player or a maintainer reading later: say what changed and why it matters, not
  which file moved. Name the defect a fix closes.
- Do not predict pull request numbers. Reference issues or requirements (`R21`) where useful.
- Plain punctuation: no em or en dashes.

## Branches and commits

- `main` is always releasable. Work happens on short-lived branches in registered worktrees
  under `worktrees/<branch>`, merged by squash pull request after Verify and Apple smoke pass.
- Commit subjects use Conventional Commits (`feat:`, `fix:`, `test:`, `docs:`, `chore:`,
  `refactor:`), present tense, under 72 characters, with a body that explains the why.
- Heavy checks on the shared host go through the check scheduler (see `AGENTS.md`).

## Cutting a release

1. Decide the version from the table above using the Unreleased entries.
2. On a release branch:
   - set `version` in the root `Cargo.toml` and run `cargo build` to refresh `Cargo.lock`;
   - set `MARKETING_VERSION` in `apple/project.yml` and the version assertion in
     `apple/Tests/EngineBridgeTests.swift`;
   - move the Unreleased entries under `## [X.Y.Z] - YYYY-MM-DD`, leave an empty Unreleased
     section, and update the comparison links at the bottom;
   - refresh `docs/planning/STATUS.md` and any requirement evidence the release changes.
3. Open the release pull request. Merge only when Verify and Apple smoke are green.
4. Tag the merge commit `vX.Y.Z` and push the tag. Create the GitHub release with the
   changelog section as its notes.
5. For a TestFlight build, dispatch the `Release` workflow on the tag with the new version and
   the next build number (`apple/RELEASE.md`). Record the Apple build state in
   `evidence/releases/`.

## Windows setup program

Desktop installers come from the release tasks (`packageReleaseMsi`, `packageReleaseDeb`, `createReleaseDistributable`), which run ProGuard with `compose/shared/desktop-rules.pro`. If a library is added and the release build starts failing or a screen misbehaves only in the installed game, check those keep rules first. The desktop jobs stay on JDK 17: jpackage 21 changed its Windows installer template, and `compose/shared/packaging/windows/main.wxs` is a copy of the 17 one (the 21 build fails with an unresolved JpCaDll reference), so moving to 21 means rebasing that file first.

Each Windows build produces `CorporateWarfare-<version>-setup.exe` (Inno Setup 6, script and
checks under `compose/shared/packaging/windows/`, pictures from `tools/installer_art.py`) beside
the MSI. `release-others.yml` and `preview.yml` build and attach it; the PR workflow builds it and
runs `test-setup.ps1` (install, downgrade, upgrade, silent uninstall, saves survive). The updater
prefers an asset ending `-setup.exe` and falls back to `.msi`, so keep that name. The MSI stays for
the 1.9 cycle because installed 1.8 games only know how to update through it. The setup program is
unsigned, so Windows SmartScreen warns on first run until signing is added.

## Hotfixes

A fix for a released version is a PATCH: branch from the release tag, fix, add the changelog
entry, release `X.Y.(Z+1)`, and make sure `main` contains the fix.
