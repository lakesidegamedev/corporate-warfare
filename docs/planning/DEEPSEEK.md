# DeepSeek execution handoff

This is a ready-to-use goal brief for the owner to invoke in the actual DeepSeek session.
This preparation session does not start DeepSeek or create an autonomous goal on its behalf.

## Goal brief

Build Corporate Warfare to RELEASE_ACCEPTED as defined in PRODUCT.md: a polished premium
offline iOS Great Lakes business simulation, Capitalism Lab/AHD depth, circa-2000 start,
persistent investor, broad corporate mechanics, effectively unlimited subsidiary play,
open-ended sandbox and a five-year scored scenario, installed and tested via TestFlight.
Read AGENTS.md and the planning index before editing. Respect confirmed owner choices,
explicit proposals, actual geography and the complete requirements ledger. Preserve old
projects. Never connect this game to AHD production databases or services.

## Execution loop

1. Read STATUS, owner decisions and requirement states. Inspect git/worktrees, active
   checks and the product kanban. Understand any concurrent ownership before touching it.
2. Pick the earliest small unblocked packet whose dependencies are satisfied. State its
   acceptance criterion and identify an independent test oracle.
3. Implement the full behaviour, smallest useful tests and necessary UI/data changes.
   Build vertical slices early. Use shared engine commands in UI and AI.
4. Run targeted checks; queue full builds/tests/sweeps through the host scheduler.
   On failure, keep the packet incomplete and fix root cause. Do not reset goldens or
   weaken the gate merely to obtain green checks.
5. Commit explicit files, push the branch and use hosted CI/PR gates as configured.
   Never weaken protection. Keep one coherent work owner and next move.
6. Update requirement evidence with tested commit, procedure, artifact and limitations.
   Mark done only after dependencies and the relevant platform/human gate have evidence.
7. Update STATUS and report material progress to the ops kanban. Continue to the next
   unblocked packet without asking routine implementation permission.

## Context-loss checkpoint

Maintain STATUS with current branch/commit, packet, exact next command/task, changed
contracts, checks queued/running, recent failures and named blockers. Keep it short.
Commit it before a session ends. Do not use a dirty worktree, chat history or model memory
as the only store of current work. GitHub issue text defines repository tasks; kanban
tracks current lane/progress. Synchronise updates without duplicating contradictory status.

## Blockers and anti-drift

Missing Apple credentials block signing, not engine/UI/data work. Missing human testers
block human acceptance, not deterministic checks. An inaccessible official crosswalk
blocks real-data signoff, not clearly labelled fixtures. Work independently where possible,
record blocker, and ask only the exact owner action that is genuinely needed.

After two unsuccessful implementation approaches to the same defect, preserve a minimal
reproduction, compare assumptions to contracts and narrow the next investigation. Avoid
another broad rewrite. If a prerequisite cannot be obtained, stop its dependent packet
and advance another track. Do not spin forever on retries or stop the whole goal while
independent requirements remain. Every external waiter needs a deadline and all terminal
failure states; a hanging poll is not progress.

Do not inflate breadth with generic industry labels, fake charts, scripted outcomes,
infinite external liquidity, no-op actions, tutorial-only economics or screenshots
without real commands. Do not report inaccessible device tests as passed. Do not
silently defer bonds, banking, insurance, labour or regulatory systems named in scope.

## Completion criteria

All release requirements have reviewed passing evidence. Hosted required checks pass.
No unresolved release-blocking defects. All source is committed. Fresh install,
versioned saves, offline gameplay, investor recovery, long-run operation and actual
TestFlight acceptance are established. Any remaining Apple-only dependency is named
and leaves the goal incomplete at SOFTWARE_READY, never falsely complete.

Do not mark a goal complete because time/context/token budget is running low or because
a scaffold compiles. Report remaining work with a resumable checkpoint.
