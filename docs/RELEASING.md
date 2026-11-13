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
- Run heavy checks one at a time (see `AGENTS.md`).

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

## Cutting 2.0.0

The owner decides the cut and the digit. Everything below is the checklist for it. Nothing in
this section has been run: 2.0.0 is prepared, not cut. The changelog's Unreleased section is
already written as the 2.0 changelog.

Digit: the table above makes 2.0 a MAJOR (a new game model with careers, rivals that build and
a new layout). Saves are not a reason for it: save layout 4 reads every save from 1.0 on, and
`crates/cw-session/tests/save_history.rs` proves the 1.9.0 and 2.0 preview saves load, play 52
weeks and save again. If the owner picks another digit, change only the version strings below.

### 1. A preview first

A preview is a pre-release build of the exact branch, with the Unreleased changelog as its notes.

1. Make sure `feat/2.0` has green `rust-and-plan`, `compose` and `emulator` checks on
   its head, and that `CHANGELOG.md` and `docs/planning/STATUS.md` describe it.
2. Build the preview (the previous one is `v2.0.0-preview.1`, so the next number is 2):

   ```
   gh workflow run preview.yml -R lakesidegamedev/corporate-warfare \
     -f ref=feat/2.0 -f version=2.0.0 -f preview=2
   gh run list -R lakesidegamedev/corporate-warfare --workflow preview.yml --limit 1
   ```

   It builds the Windows setup program and MSI, the Linux package and AppImage and the Mac disk
   image, and attaches them to a pre-release `v2.0.0-preview.2`.
3. Install it and check, by hand, on the platforms you can reach: open a save from 1.9.0 and play
   a few weeks; start a career at the helm of a company and read the first quarter's letter; wait
   for a raid warning and defend; watch a failed company's plants go to auction and bid; open
   Openings, Auctions and Raids; try the tutorial from a fresh settings file; run the Windows
   setup program over an installed 1.9.0 and confirm the update keeps saves.
4. Fix what turns up with new pull requests into `feat/2.0` and build the next preview. Cut only
   from a preview whose checks and hand checks passed.

### 2. The release branch

From an up to date `feat/2.0`:

```
git switch -c release/2.0.0 feat/2.0
# 1. version strings, all three must agree with the new heading
sed -i 's/^version = "1.9.0"/version = "2.0.0"/' Cargo.toml          # the [workspace.package] line
sed -i 's/MARKETING_VERSION: "1.9.0"/MARKETING_VERSION: "2.0.0"/' apple/project.yml
sed -i 's/XCTAssertEqual(info.version, "1.9.0")/XCTAssertEqual(info.version, "2.0.0")/' apple/Tests/EngineBridgeTests.swift
cargo build -p cw-cli                                                  # refreshes Cargo.lock
# 2. changelog: turn the Unreleased heading into the release, keep an empty Unreleased above it
#    "## [Unreleased]"  ->  "## [Unreleased]" + blank + "## [2.0.0] - YYYY-MM-DD"
#    and add the comparison links at the bottom if the file keeps them
python3 tools/changelog_check.py
# 3. status
$EDITOR docs/planning/STATUS.md                                        # 2.0.0 released, next move
git commit -am "chore(release): 2.0.0"
```

Check the old version string is gone everywhere that should change and nowhere that should not:
`git grep -n '1\.9\.0' -- Cargo.toml apple compose` (the Kotlin tests use 1.9.0 as sample data and
stay). The iOS build number (`CURRENT_PROJECT_VERSION`) is chosen at upload, see step 4.

### 3. The pull requests

The release branch goes to `main` as one pull request: open it, add the `ci:apple` label so the
macOS job runs, wait for `rust-and-plan`, `compose`, `emulator` and `unsigned-apple`, then squash
merge as for 1.9.0.

```
gh pr create -R lakesidegamedev/corporate-warfare --base main --head release/2.0.0 \
  --title "Corporate Warfare 2.0.0" --body-file <(awk '/^## \[2.0.0\]/{f=1;next} /^## \[/{f=0} f' CHANGELOG.md)
gh pr edit <N> -R lakesidegamedev/corporate-warfare --add-label ci:apple
gh pr checks <N> -R lakesidegamedev/corporate-warfare --watch
gh api -X PUT repos/lakesidegamedev/corporate-warfare/pulls/<N>/merge -f merge_method=squash
```

### 4. Tag, GitHub release, store builds

```
git fetch origin && git tag -a v2.0.0 origin/main -m "Corporate Warfare 2.0.0"   # origin/main is the merge
git push origin v2.0.0
awk '/^## \[2.0.0\]/{f=1;next} /^## \[/{f=0} f' CHANGELOG.md > /tmp/notes-2.0.0.md
gh release create v2.0.0 -R lakesidegamedev/corporate-warfare --title "Corporate Warfare 2.0.0" \
  --notes-file /tmp/notes-2.0.0.md --latest
```

Publishing the release runs `release-others.yml`: it builds the signed Android bundle and APK
and the Windows, Linux and Mac packages. Watch it with
`gh run list --workflow release-others.yml --limit 1`; it does not run for a pre-release.

Corporate Warfare is a paid game, so a full release does NOT put its installers on the public
release. The workflow has an input `attach_installers` that defaults to off, and a published-release
run never sets it. What it always does, whatever the input:

- keeps every installer as a workflow artifact (`installers-android`, `installers-ubuntu-latest`,
  `installers-windows-latest`, `installers-mac`, kept 90 days), each with a `SHA256SUMS-*.txt` file
  listing the SHA-256 of every file in it, for the Lakeside side to fetch into its own storage;
- builds and signs exactly as before.

To fetch them: `gh run download <run-id> -R lakesidegamedev/corporate-warfare -D <empty folder>`,
then check each file against its sums file.

To attach the installers to the public release anyway (for example to re-publish a free build), run
the workflow by hand: `gh workflow run release-others.yml -R lakesidegamedev/corporate-warfare
-f tag=v2.0.0 -f attach_installers=true`. Tester previews (`preview.yml`) still attach their
installers to the pre-release; that stays until the owner decides otherwise. Existing releases are
never edited by these workflows.

A release with no installer attached is normal. The in-game updater then says "this version is
delivered through the Lakeside launcher or your store" instead of showing an error. A game started
from the Lakeside launcher has its own updater switched off; see `docs/rules/LAUNCHER.md`.

iPhone, iPad and Mac (App Store Connect build): dispatch the Release workflow on the tag with
the next unused build number (the last one is recorded under `evidence/releases/`):

```
gh workflow run release.yml -R lakesidegamedev/corporate-warfare --ref v2.0.0 \
  -f version=2.0.0 -f build=<next build number> -f confirm=UPLOAD
```

Then follow `apple/RELEASE.md` (processing, VALID, installed, acceptance) and record the Apple
build state in `evidence/releases/`. Do not call it available before the workflow says VALID.

### 5. What the owner does by hand

These cannot be done by a workflow or an agent.

App Store Connect:
- Create the 2.0.0 version record on the app and attach the VALID build.
- Paste "What's New for 2.0" and, if wanted, the description from `apple/metadata/app-store-metadata.md`.
  Update the review notes (`apple/metadata/review-notes.md`) and the age rating answers if the
  game changed (raids and auctions are the same kind of content as before).
- Upload the iPhone 6.9-inch, iPad 13-inch and Mac Catalyst screenshot sets rendered from the
  tagged commit (`apple/RELEASE.md`, Screenshots). Do not edit the images.
- Confirm the privacy answers still match `store/declarations.md` (nothing changed in what the
  app sends) and submit for review. Choose manual or automatic release after approval.
- Add the build to the TestFlight group first if you want a final device check; builds expire
  after ninety days.

Google Play Console:
- Create a production (or staged) release and upload `CorporateWarfare-2.0.0-android.aab` from
  the GitHub release. The version code for 2.0.0 is 20000, above every earlier build and every
  2.0.0 preview.
- Paste the short and full description and "What's new in 2.0" from `store/play/listing.md`
  (500 characters at most for What's new).
- Confirm the data safety form still matches `store/declarations.md`, review the screenshots and
  start the rollout.

After both stores are live: attach the store links to the GitHub release notes if wanted, close
the milestone and issues the release fixed, and delete `feat/2.0` and `release/2.0.0` once they
are merged (keep the tag).

### 6. If something is wrong after the cut

A fix for a released version is a PATCH (see Hotfixes below): branch from `v2.0.0`, fix, add the
changelog entry, release `2.0.1`, and make sure `main` contains the fix. A store build cannot be
recalled; stop the staged Play rollout or remove the build from sale in App Store Connect while the
fix is built.

### Open questions for the owner

- The Windows MSI was kept for 1.9 because installed 1.8 games only update through it. Installed
  1.9 games prefer the setup program, so 2.0 could drop the MSI. Until the owner decides, the
  workflows still build both.
- The new table layout is Compose desktop only (Windows, Linux, Mac disk image); the store copy
  does not claim it for the Apple apps.

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
