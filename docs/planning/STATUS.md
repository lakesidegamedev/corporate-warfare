# Current checkpoint

Date: 2026-10-25. Owner: DeepSeek goal session. Main: `319fe31`.
Kanban: 254781. Merged PRs: #8 through #62.

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
  board limits gate large debt, capex, issuance and mergers, antitrust remedies are real
  divestitures, bond maturity pays the final coupon, private firm data needs information rights,
  bank runs pay depositors pro rata, insurance claims settle the following week, and the
  release verifier checks the compiled asset catalog while export uses the App Store Connect
  key, the clock pauses before grace, strikes and imminent deadlines, quality investment is a
  bounded funded lever, defense demand runs through funded 52-week procurement contracts with
  deposits, accepted delivery, deferred unmet demand and a two-week penalty grace, and labour
  income pays twenty percent tax into a real receipt series that recomputes the three fiscal
  budgets from the trailing thirteen-week mean.

## Verified

- Local: 213 tests pass; shared scheduler job `20261025T113141Z-7b3791c9` passed on `main`; `cargo fmt --check` and `cargo clippy --workspace --all-targets -D
  warnings` clean; design and plan validators pass; shared scheduler jobs passed
  (`20261025T070655Z-5df8bceb` latest). Defense procurement has six unit tests and its
  industry budget scenario; fiscal budgets now follow actual labour and corporate receipts.
  Labour tax is withheld on wages firms actually paid, attributed pro rata across employing
  regions; deferred payroll is an overdue liability and is not taxed. Withholding is a regional
  aggregate rather than per-worker, and transfer support still draws from the training budget;
  both are named refinements. Household baselines and opening savings use disposable income,
  and a wage-shortfall week may draw at most one percent of savings into demand. Transfer
  support uses each unemployed tier's baseline wage and reports any unfunded need. The hostile
  session sequence now covers 540 commands over twenty weeks.
- Corridor at seed 3: 0 rival failures at 52 weeks, 4-5 at 260; score 61 at week 260. Widened
  runs: 1,000 five-year seeds score 56/62/62 with dissolved median 5 (3-10); 100 thirty-year
  seeds have dissolved median 10 (6-15) and the conservative policy loses control in the
  median seed without founding a replacement.
- Engine baseline on this host: representative (31 firms, 290 facilities) p50 0.55 ms; grown
  (217/2,030) p50 6.46 ms; stress (992/9,280) p50 82.98 ms. Device timing still required.
- Hosted Verify and Apple smoke are green on `main` `319fe31`: the SwiftUI app compiles for the
  simulator and device archive, the Rust bridge links, and 14 XCTests pass with hash
  `6731ed7c...`. The newer suite includes real `AppStore` coverage for new career, selection,
  advance, stop-save/restore and a rejected phone command. The repository is public and Actions
  runs normally.

## Blockers

1. **Apple signing and TestFlight.** The repository is public and hosted Verify/Apple smoke are
   green, but no App Store Connect app record or signing secrets exist. Release run 40209622603
   failed closed with `blocked: missing signing secrets: APPLE_API_ISSUER APPLE_API_KEY
   APPLE_API_KEY_CONTENT APPLE_DEVELOPMENT_TEAM`. Owner/Muse must add those four secrets to the
   `appstore` environment for `net.lakesidegames.corporatewarfare` and confirm the internal tester
   group.
2. **Geography/data gate.** The official crosswalk is still missing; R01/R02 and dependent
   packets remain blocked pending an owner-supplied file or an approved custom corridor.
3. **Device and human gates.** No physical iPhone measurements (R37), no three independent
   testers (R38) and no TestFlight installation (R42).


## Next moves, in order

1. Owner/Muse supplies the App Store Connect app record and the four signing secrets named in
   `evidence/handoff-muse.md`; dispatch `Release` and poll processing.
2. Install the processed build on a real iPhone and run the offline acceptance route.
3. Owner decides the geography: supply the official crosswalk or approve a named custom corridor.
   Until then R01/R02 and their dependants cannot be signed off.
4. With a signed build: device profiling (R37), three playtests (R38), then R41/R42.

Resume with:
`cd /root/projects/corporate-warfare && git pull && cargo test --locked --workspace`
then read this file and `evidence/release-readiness.md`.
