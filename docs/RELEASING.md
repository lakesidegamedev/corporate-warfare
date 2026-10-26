# Versioning and release procedure

## Version numbers

Corporate Warfare uses `MAJOR.MINOR.PATCH`. The owner's rule decides which digit moves:

| Change | Digit | Example |
|---|---|---|
| Minor feature updates, fixes, balance and copy changes | **PATCH** | 1.0.0 to 1.0.1 |
| A big feature: a new system, screen family or mechanic players will notice | **MINOR** | 1.0.1 to 1.1.0 |
| A wide feature release or upgrade: a redesign, a new game model, an incompatible save | **MAJOR** | 1.1.0 to 2.0.0 |

When one release mixes sizes, the largest change decides. A release that cannot read the
previous release's saves is always MAJOR.

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

## Hotfixes

A fix for a released version is a PATCH: branch from the release tag, fix, add the changelog
entry, release `X.Y.(Z+1)`, and make sure `main` contains the fix.
