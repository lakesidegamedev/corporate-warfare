# Current checkpoint

Date: 2026-10-27. Released: **1.6.0**, business cycle, central bank, government and laws, globe menu, scenarios, difficulty, Command Center, research, economy card, desktop layout, sliders, load game, tutorial and in-game updates. Before that **1.5.0**, whole-world globe map, scenario cards, Stats screen with five-year history, aggressive rivals, new logo and icons, speed control. Before that **1.4.0**, stock exchange and commodity board, valuation fix, and
the first Android, Windows, Linux and Mac (Catalyst) builds (PRs #88, #89, #90). Kanban: 254808.
Saves from 1.3.1 load.

Open: Google Play listing and upload-key enrolment (owner), Mac App Store/TestFlight needs the
macOS platform on the App Store Connect record (owner), Mac and Windows builds are unsigned.

# 1.3.1 checkpoint (history)

## 1.3.1

Owner on 1.3.0: "I just lose money constantly. Pricing posture from AHD didn't really make the
transition either. And it's probably not this sector only." Chose all four fixes and a margin
of about 20 percent. Built: pricing posture per good (auto, -20..+20 percent vs market,
cost-plus) with a CEO Office pricing panel; 21 thin strategies rebalanced by crew to about
17-24 percent after depreciation; seasonal farm crews; staged build payment. Guided career at
week 104 averages $1.44M company cash over eight seeds (1.3.0: $976K); rival dissolutions 1 in
600 firm-runs over five years.

# 1.3.0 checkpoint (history)

## 1.3.0

Owner direction (2026-10-26): the economy was not active enough for supply and demand to
matter and the player always lost money; add anonymous market snapshots every five weeks
(Lakeside ops endpoint, always on, anonymous); act on a design audit that scored the interface
5.5/10. Built:

- Corridor commodity exchange for goods, wider outside band, cheaper freight, sell-through
  pricing and plant-by-plant production planning for rivals and (automatically) the player.
- Defects fixed: unrecorded contract, power and construction sales; a catastrophe-mothballed
  plant kept full payroll with no way back (now reopenable); contract buyers took goods they
  could not pay for; the rival growth cap was never rescaled for 1.2.0's smaller plants.
- Rivals enter plain gaps; the corridor now produces fuel.
- Anonymous market snapshots every five weeks to the ops ingest, with a switch in the app.
- Interface: compact headers, hard tab bar edge, grouped figures, quarterly profit with a cost
  breakdown, scannable Markets with signals and filters, Inbox as a feed, Portfolio led by net
  worth with a cash outlook.

Verified at `fa1e915`: workspace tests through the check scheduler, hosted Verify and Apple
smoke with UI snapshots. Player cash at week 104 averages $976K over eight seeds; rival
dissolutions 4 in 600 firm-runs over five years.

Open:

1. The corridor's new fuel producer still exports part of its output while the corridor
   imports, because freight puts its delivered price above imports.
2. Farm seasonality is harsh for the guided player.
3. Second-level company screens still use the older layout; visual identity (P2 of the audit).
4. Human and device gates from 1.0.0 remain: R32/R33/R35, R37, R38, R39, R42.

# 1.2.0 checkpoint (history)

## 1.2.0

Owner direction (DECISIONS.md, 2026-10-26): economic regions inside the states, A House
Divided sector divisions with plants inside them, the map showing only the player's divisions
with plant counts, cheaper plants, and the most ambitious update so far. Built:

- 31 county-built economic regions (Census 2000, SAIPE 2000); states as the political layer
  with two specialized sectors each; statewide service markets; per-state public works and
  gateways.
- Divisions: entry fee, founding discount, one strategy per division, division retools,
  market share and dominance pricing, home and specialization input factors, sprawl by
  division.
- Market raids with rival AI use; market news.
- Board objectives with a management bonus; the score rebased on growth, expansion and
  leadership; an expander reference policy.
- Plants a fifth of their 1.1.0 size; tax loss carryforward.
- App: region map with owned sector circles and plant badges, region boards, division build
  flow with a full cost quote, raid sheet, board goal strip, CEO Office divisions.

Verified at `6a36dcb`: `cargo test --locked --workspace` through the check scheduler and
hosted Verify and Apple smoke (Swift compiles, bridge and snapshot tests pass). Balance (R36)
regenerated: `evidence/balance-report.md`, zero errors; the no-action career scores 36 and
the expander 59.

Open:

1. Reference policies need a sell-through rule (the thirty-year expander finding).
2. Rival dissolutions 3.7 percent, construction weakest.
3. Human and device gates from 1.0.0 remain: R32/R33/R35, R37, R38, R39, R42.

# 1.1.0 checkpoint (history)


Owner direction (DECISIONS.md, 2026-10-26): play AHD corporations single-player over a real
map of eight states, keeping this engine. Built:

- World: eight playable states (IL, IN, KY, MI, NY, OH, PA, WV) with 2000 Census population,
  wage, land and demand indices and AHD-style conditions (power grid, roads, broadband,
  skill, crime, cost of living); 28 freight lanes from great-circle distances.
- Corporations: AHD sector types with 53 named operating strategies over 34 goods, primary
  and secondary sector focus with transition and cooldown, logistics strength and sprawl,
  retooling within a sector, depleting deposits for extraction, and founding with
  headquarters, strategy and secondary focus.
- Markets: smoothed gateway imports as "room to build"; rivals build where imports point and
  retool out of glut; sellers trade wherever their goods sit; a paced outside economy.
- App: dark cartographic design system; vector map (Census, TIGER, Natural Earth) as home
  with presence, room, wage and skill layers; state boards; build flow; CEO office; commodity
  board Markets; restyled legacy screens; CI renders iPhone and iPad snapshots for review.

Verified at `6586ab6`: `cargo test --locked --workspace` through the check scheduler, hosted
Verify and Apple smoke (Swift compiles, engine bridge and snapshot tests pass, the
cross-platform hash `840de364...` matches on the simulator). Balance (R36) regenerated:
`evidence/balance-report.md`, zero errors, 8 of 600 rival five-year firm-runs dissolve.

Next:

1. 1.1.1: rescore against the opening position and add a map-aware reference policy
   (owner decision in DECISIONS.md), then rerun R36.
2. TestFlight build of 1.1.0 via Muse or the `Release` workflow (`apple/RELEASE.md`).
3. Human and device gates from 1.0.0 remain: R32/R33/R35, R37, R38, R39, R42.

# 1.0.0 checkpoint (history)

Date: 2026-10-25. Version: 1.0.0. Branch: `release/1.0.0` (PR #70).

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
