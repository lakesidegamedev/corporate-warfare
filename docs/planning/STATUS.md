# Current checkpoint

Date: 2026-10-25. Owner: DeepSeek goal session. Main: `5b87b3b`.
Kanban: 254781. Merged PRs: #8 through #43.

## What exists now

- Rust engine: checked units and ledger, deterministic session/replay/saves, weekly economy
  (labour with layoffs, production, utilities, one-pass markets, freight, contracts, gateways),
  corporate systems (ownership, boards, exchange, issuance/dividends/buybacks/IPO, credit,
  banking, insurance, deals and antitrust, unions, lobbying, insolvency), R&D/brand, rival AI,
  events (rivals answer their own cards), five-year score with continue.
- CLI: `doctor`, `run`, `diagnose`, `report`, `sweep` (three reference policies, JSON, invariant
  checks), `replay`.
- SwiftUI app: portfolio, group tree, company hub for every corporate family, regions, markets,
  inbox, cash bridge, onboarding, score breakdown, save recovery with rolling last-good and a
  reviewed diagnostic bundle, lifecycle stop-and-save.
- Release track: app icon, privacy manifest and portable audit, metadata drafts, fail-closed
  signed release workflow with bounded App Store Connect processing check.
- Evidence: ledger/units, commands, cross-platform session hash, industry scenarios, adversarial
  saves, balance distributions, corporate lifecycles, brand demand, workflows, persistent-investor
  recovery, and a Linux engine baseline for representative/grown/stress fixtures.
- Release aids: static accessibility audit, portable privacy audit, owner-action runbook, and a
  single recommended next action with its reason on the portfolio.
- Macro: demand and supply modifiers now apply, macro recessions/booms come from the macro
  stream, optional cards expire, and the technology frontier advances yearly.
- Corporate correctness: union ballots form bargaining units, parent guarantees are called up to
  their cap, bond trades price accrued interest, loan interest carries sub-cents, lobbying has a
  post-decision group cooldown, tax modifiers expire, legal mergers move the whole entity,
  construction service is capacity-bounded once per week, asset purchases settle without shares,
  and board limits gate large debt, capex, issuance and mergers.

## Verified

- Local: 171 tests pass; shared scheduler job `20261025T075056Z-dd9c6d94` passed on `main`; `cargo fmt --check` and `cargo clippy --workspace --all-targets -D
  warnings` clean; design and plan validators pass; shared scheduler jobs passed
  (`20261025T070655Z-5df8bceb` latest).
- Corridor at seed 3: 0 rival failures at 52 weeks, 4-5 at 260 across seeds, 10 over thirty
  years; guided firm solvent and in control at every horizon; score 61 at week 260.
- Engine baseline on this host: representative (31 firms, 290 facilities) p50 0.55 ms; grown
  (217/2,030) p50 6.46 ms; stress (992/9,280) p50 82.98 ms. Device timing still required.
- Hosted Verify and Apple smoke last ran green at PR #11. Since PR #12 GitHub Actions fails in
  seconds for billing, so every Swift change after the onboarding PR is syntax-checked only and
  has **not** been compiled by Xcode or run in a simulator.

## Blockers

1. **GitHub Actions billing.** Every workflow fails before starting. This blocks Apple compile,
   simulator XCTest, the current canonical-hash verification and all hosted gates. Owner must
   restore billing or raise the spending limit.
2. **Apple signing and TestFlight.** No repository/org secrets and no App Store Connect key for
   `net.lakesidegames.corporatewarfare`. Exact secrets and owner actions are in
   `apple/RELEASE.md`. Without this there is no signed archive, upload, processed build or
   TestFlight installation.
3. **Geography/data gate.** The official 2004 BEA county crosswalk is unrecoverable from
   official sources. R01/R02 stay blocked, and every economy packet/requirement that depends on
   them is transitively blocked in the ledger even though the systems are implemented against
   labelled fixtures. A custom region scheme or an owner-supplied crosswalk is a product
   decision, not something to invent.
4. **Device and human gates.** No physical iPhone measurements (R37), no three independent
   testers (R38), no TestFlight installation (R42).

## Next moves, in order

1. Restore GitHub Actions, then run Apple smoke on `main`: this is the first real compile and
   simulator run of the corporate flows, recovery layer and score sheet.
2. Owner supplies Apple team/app record/secrets or names an approved studio key; dispatch the
   release workflow; poll processing; install on an iPhone and run the acceptance route.
3. Owner decides the geography: supply the official crosswalk or approve a named custom corridor.
   Until then R01/R02 and their dependants cannot be signed off.
4. With a signed build: device profiling (R37), three playtests (R38), then R41/R42.

Resume with:
`cd /root/projects/corporate-warfare && git pull && cargo test --locked --workspace`
then read this file and `evidence/release-readiness.md`.
