# Contributor and agent instructions

Corporate Warfare is an offline economic strategy game. This repository holds the Rust engine,
the content, the command line tools, the Compose apps and the Apple apps. Start with
[README.md](README.md).

## Authority

Read `docs/planning/README.md` (product, decisions, status and the requirement ledger) and
`docs/rules/README.md` plus the exact rule pages for the area you change before implementing.
`docs/planning/execution.json` tracks work items. The historical `docs/PLAN.md` is superseded.
Owner answers take precedence. Distinguish confirmed choices, proposed defaults and verified
evidence, and never silently drop scope. Record approved design changes in
`docs/planning/DECISIONS.md` and update the related rule page, parameters and tests together.

## Development

- No servers, databases or network calls inside simulation. No live game data access.
- Integer and fixed-point authoritative rules, checked arithmetic, explicit settlement order.
- Same validated commands and information rules for player and AI.
- All statements and valuation views derive from consistent ledger and ownership definitions.
- Keep the simulation independent of AI and UI; the session orchestrates both.
- Tests need independent economic oracles; do not mirror code or bless unexplained goldens.
- Do not claim feature completion without requirement evidence and platform or human checks.
- Keep generated bindings and artifacts out of source. Pin matching generator and runtime.
- Any change to a saved field bumps the save schema and keeps a frozen layout copy with a
  fixture test (`docs/rules/INTERFACE.md`). Saves from every earlier release must keep loading.
- A reused design from a reference game needs a written grant from its owner; A House Divided's
  sector catalogue, strategies, commodities and formulas are granted (`DECISIONS.md`).
- Never commit credentials, signing keys, personal data or production database snapshots.

## Branches, worktrees and commits

- Work on a short-lived branch in its own git worktree under `worktrees/<branch>`. Check
  `git worktree list` first, commit explicit files (not `git add .`), and leave a worktree clean:
  commit it, or delete it once merged. Do not remove a worktree someone else has locked.
- Commit subjects use Conventional Commits (`feat:`, `fix:`, `test:`, `docs:`, `chore:`,
  `refactor:`), present tense, under 72 characters, with a body that explains why.
- Use plain punctuation in code, comments, copy and commits: no em or en dashes.

## Checks

Run the smallest relevant check while you work, for example `cargo test -p cw-session <name>`,
and the full set before a pull request:

```
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 tools/plan_check.py
python3 tools/design_check.py
python3 tools/changelog_check.py
```

Full builds are heavy: run one at a time, and on a shared machine give them a lower priority
(`nice -n 10`) and your own `CARGO_TARGET_DIR`. Apple builds need macOS and run in CI; a Linux
run is never evidence of device behaviour. Compose checks are listed in `compose/README.md`.

## Changelog and releases

- Every pull request that changes code, content, tooling or workflows adds a `CHANGELOG.md`
  entry under Unreleased in the same pull request. CI enforces this and the version agreement
  between `Cargo.toml`, `apple/project.yml` and the changelog.
- Versioning, changelog and release steps live in `docs/RELEASING.md`. Patch for minor updates
  and fixes, minor for big features, major for wide feature releases or upgrades.
- Releases are tagged `vX.Y.Z` on `main` with the changelog section as the GitHub release notes.
