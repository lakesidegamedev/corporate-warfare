# Current checkpoint

Date: 2026-10-25. Version: 1.0.0 (engine, workspace crates and iOS marketing version).
Branch: `release/1.0.0` (PR #70). Kanban: 254781.

## What exists now

- Rust engine: checked units and ledger, deterministic session/replay/saves, weekly economy
  (labour with layoffs, production, utilities, one-pass markets, freight, contracts, gateways),
  corporate systems (ownership, boards, exchange, issuance/dividends/buybacks/IPO, credit with
  arrears, banking, insurance, deals and antitrust, unions, lobbying, insolvency), R&D/brand,
  rival AI, a 24-card event deck with real triggers, macro regimes, and a five-year score with
  continue.
- CLI: `doctor`, `run`, `diagnose`, `report`, `sweep` (three reference policies plus a
  no-action baseline, JSON, invariant checks, rival archetype telemetry), `replay`. Diagnostic
  examples in `crates/cw-session/examples/` (`event_census`, `dissolution_census`,
  `firm_trace`).
- SwiftUI app: portfolio, group tree, company hub for every corporate family, regions, markets,
  inbox, cash bridge, onboarding, score breakdown, save recovery with rolling last-good and a
  reviewed diagnostic bundle, lifecycle stop-and-save.
- Release track: app icon, privacy manifest and portable audit, metadata drafts, fail-closed
  signed release workflow with bounded App Store Connect processing check.

## 1.0.0 engine pass (this branch)

Auditing the open requirements against `docs/rules/` turned up defects in both open and
previously closed systems. All are fixed, with hand-computed fixtures:

- Deals and antitrust (R21/R22). Review now runs before settlement, tenders are prorated and paid
  from escrow, and squeeze-out pays minorities. The player's tender is settled from escrow
  (before, it minted cash). Groups are judged at the ultimate controller, so a shell cannot
  bypass review. Remedies are checked for feasibility, and mergers pay remaining minorities.
- Lobbying (R24). Cooldown and fee are group-wide, and a won training grant no longer pays twice.
  The backlash penalty goes through its event card only.
- Rival AI (R28). Strategic slots are scored and capped at four actions. A token player stake no
  longer freezes a rival, and rivals use public data only. Rival M&A works: on `main` it never
  settled once. Rivals follow the rules' solvency order instead of idling seasonal plant, and
  expand only into observed demand.
- Events (R29). Fourteen of the 24 cards could never open; every card now has a real trigger.
  Handlers no longer delete goods in transit or reach other firms. Macro regimes come from the
  macro stream, answering needs authority over the company, and tender cards address the
  investor.
- Score (R34). Control counts controlled holdings once and ignores passive stakes. Conduct counts
  only the player's own record.
- Liquidation (R25, reopened). Asset sales minted proceeds, lenders were never paid, and dead
  firms kept cash and contracts. It is now a conserving waterfall.
- Credit and governance (R17/R18, reopened). A missed principal payment counted the debt twice;
  it is now arrears. Passed board votes now actually authorise. The loan-quote exploit is
  closed: a borrower could name its own rate, term and size.
- Retooled plants now return to service (before, retooling bricked them).

## Verified

- Local at `3fa5a32`: `cargo test --locked --workspace`, `cargo clippy --workspace --all-targets
  -D warnings`, `cargo fmt --check`, `tools/plan_check.py`, `tools/design_check.py` and the tools
  unit tests all pass. The canonical cross-platform hash is re-pinned in Rust and Swift to
  `1d5209e2...`; hosted Apple smoke must confirm it on the simulator.
- Balance (R36): 860 careers at `3fa5a32` (`evidence/balance-report.md`). Zero invariant
  failures, zero early stops, deterministic. Guided five-year conservative median score is 62.
  Rival five-year dissolutions are 17 of 600 firm-runs, against about 99 on `main`, and no
  industry dies out every run (agriculture was 40/40).
- Requirements: 33 of 42 done with evidence (R01-R31 and R34, R36). Packets: 71 of 117
  checklist steps done. Fifteen packets have every step evidenced but are held `active` until
  the upstream packets P06, P07, P09, P14, P15, P19 and P20 get their own line-by-line checklist
  audit.

## Remaining, in critical-path order

1. **Apple signing / TestFlight (R40 signing step, R41).** The studio App Store Connect key
   used for the other Lakeside iOS apps is the intended key, but its issuer ID and team ID are
   stored only as secrets elsewhere and GitHub never returns secret values. The owner must
   create the App Store Connect app record for `net.lakesidegames.corporatewarfare` (the API
   cannot) and supply the issuer ID and team ID. Then the four secrets go into the `appstore`
   environment and the `Release` workflow is dispatched with version `1.0.0`
   (`apple/RELEASE.md`).
2. **Human and device gates.** R32/R33/R35 phone acceptance and accessibility, R37 A15-class
   device timing, R38 three independent testers, R42 installed acceptance flight, and the R39
   review of final assets, privacy, rating and export answers. None of these can be done from
   Linux.
3. **Packet audit.** A line-by-line audit of P06, P07, P09, P14, P15, P19 and P20 against their
   tests, then close the fifteen held packets.
4. **UI test target.** `apple/UITests/AppFlowUITests.swift` hung the simulator runner and stays
   outside the default scheme.

Resume with:
`cd /root/projects/corporate-warfare && git pull && cargo test --locked --workspace`, then read
this file, `evidence/balance-report.md` and `apple/RELEASE.md`.
