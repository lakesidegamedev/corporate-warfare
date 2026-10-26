# Corporate Warfare agent instructions

Read `/root/AGENTS.md` and `/root/.claude/CLAUDE.md` on this host. This is a new private
repo, not either old Corporate Warfare project. Preserve `/root/misc/corpgame` and
`/root/misc/corporate-warfare-sandbox` and all live services.

## Authority

Read `docs/planning/README.md`, PRODUCT, DECISIONS, STATUS and the requirement ledger.
Read `docs/rules/README.md` and the exact rules before implementation; use
`docs/planning/execution.json` for work-item tracking. The historical `docs/PLAN.md`
is superseded. Owner answers take precedence. Distinguish
confirmed choices, proposed defaults and verified evidence. Never silently drop scope.

This preparation task is owned by Codex. The later implementation is intended for the
owner's DeepSeek goal session; do not launch it on your own. Provider names are literal.
External coding providers must use the host's provider runner and provenance controls.

## Development

- No servers, databases or network calls inside simulation. No live game data access.
- Integer/fixed-point authoritative rules, checked arithmetic, explicit settlement order.
- Same validated commands and information rules for player and AI.
- All statements/valuation views derive from consistent ledger and ownership definitions.
- Keep sim independent of AI and UI; session orchestrates both.
- Tests need independent economic oracles; do not mirror code or bless unexplained goldens.
- Do not claim feature completion without requirement evidence and platform/human checks.
- Keep generated bindings/artifacts out of source. Pin matching generator/runtime.
- The owner granted free reuse of A House Divided's sector catalogue, strategies, commodities
  and formulas (2026-10-26, DECISIONS.md). Other reference games still need a grant.
- Never commit credentials, signing keys, personal data or production database snapshots.

Use feature worktrees at `worktrees/<branch>` for concurrent/long-lived work. Check
`git worktree list` first. Commit explicit files and finish each session clean, with
current owner and next move. Never remove locked worktrees.

## Checks on this VPS

Targeted lightweight checks are fine. The scheduler requires a registered task worktree
with a `.git` link file, not the main checkout. Create one after inspecting worktree
ownership, then enqueue full build/test/clippy/sweeps:

```
git worktree list
git worktree add worktrees/<task> -b <branch>
/root/bin/lakeside-check-queue enqueue --workdir /root/projects/corporate-warfare/worktrees/<task> --label cw-verify --priority normal -- cargo test --locked --workspace
```

Use the actual registered worktree path. Inspect `list` and `status <job-id>`, handle
all terminal states and retain evidence. Do not reserve excessive heaps or bypass host
admission. Apple builds require the Mac runner, never a claim of Linux device validation.

## Progress and handoff

Product slug: `corporate-warfare`. Preparation kanban item: 254772. Start product work
with kanban_brief and claim/add task, then report meaningful progress. Ask owner decisions
in chat; record answers afterward. GitHub issues define repo task content, ops kanban
tracks execution. Update STATUS and requirements, commit, and keep the next move concrete.

No agent delegation by default. Do not send external messages or invite testers without
explicit instruction. Routine implementation within approved scope does not need another
permission request.

## Standing procedure

- Versioning, changelog and release steps live in `docs/RELEASING.md`. Patch for minor updates
  and fixes, minor for big features, major for wide feature releases or upgrades.
- Every pull request that changes code, content, tooling or workflows adds a `CHANGELOG.md`
  entry under Unreleased in the same pull request. CI enforces this and the version agreement
  between `Cargo.toml`, `apple/project.yml` and the changelog.
- Releases are tagged `vX.Y.Z` on `main` with the changelog section as the GitHub release
  notes.

