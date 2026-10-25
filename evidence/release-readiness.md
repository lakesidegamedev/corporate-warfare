# Release readiness map

Updated 2026-10-25 against main `981bf91`. This maps every release requirement to its real
evidence or its concrete blocker. It is not a completion claim: a requirement is only done when
its ledger entry names reviewed evidence that satisfies the gate.

| Req | Gate | State | Evidence or blocker |
|---|---|---|---|
| R01 | automated | blocked | Official 2004 BEA county-to-EA crosswalk unrecoverable (BEA redirects/404, archive 429). Codes/names verified, county rows absent. |
| R02 | automated | blocked | Depends on R01; dataset is labelled engineering calibration. |
| R03 | automated | done | `evidence/R03-ledger.json`; ledger/units/farm worked example. |
| R04 | automated | done | `evidence/R04-commands.json`; handles, authority, rollback, duplicate IDs. |
| R05 | cross-platform | done | `evidence/R05-cross-platform.json`; Apple smoke run 40206065805 compiled the app, ran 12 simulator tests and matched hash `6731ed7c...` on main `981bf91`. |
| R06 | automated | active | Household disposable baseline, 1% savings supplement, paid-wage income and finite gateways are implemented with market tests. R02 still blocks release sign-off. |
| R07 | automated | active | Labour allocation, 2% search, tier offers, wage clamp, training and paid/deferred payroll are tested. R06/R02 block the chain. |
| R08 | automated | active | Production, inputs, capacity and service expiry are covered by the 18 industry scenarios. |
| R09 | automated | active | One-pass allocator, worked examples and full-tick invariants are tested; release chain still depends on the blocked baseline. |
| R10 | automated | active | Contracts, shipments, freight, carrier priority and spot-import tests exist. |
| R11 | automated | active | Construction escrow, lead time and capacity tests exist. |
| R12 | automated | done | `evidence/R12-career.json`; control-loss continuity and one management contract with cooldown pass. |
| R13 | automated | active | Ownership, cycle rejection, 100-level/100-child traversal, internal investment and two-sided intercompany-loan elimination are tested. Unrealised profit in unsold intercompany inventory still needs a fixture. |
| R14 | automated | pending | Intercompany loans and transfer contracts implemented; internal profit elimination needs a fixture. |
| R15 | automated | pending | Exchange, finite float, impact, valuation implemented; needs a holder-transfer/look-through fixture. |
| R16 | automated | pending | Issuance, dividend, buyback, IPO implemented with treasury exclusion; needs share/vote conservation at scale. |
| R17 | automated | pending | Boards, seats, CEO, votes implemented; needs deadline/deadlock fixtures. |
| R18 | automated | pending | Loans, bonds, grades, covenants implemented; needs principal/coupon edge fixtures. |
| R19 | automated | active | Deposit-loan-withdrawal, bank-run and industry banking fixtures pass. |
| R20 | automated | active | Underwriting, reserves, catastrophe and claim-payment fixtures pass. |
| R21 | automated | active | Funded tender/minority payout and legal-merger lifecycle fixtures pass. |
| R22 | automated | pending | HHI, review, remedy implemented; needs concentration threshold fixtures. |
| R23 | automated | active | Organisation, strike deadline and binding-settlement fixture passes. |
| R24 | automated | pending | Lobby requests, cooldown, saved draw, reputation implemented; needs repeat/backlash fixtures. |
| R25 | automated | active | Grace, liquidation waterfall, parent guarantee and sibling-retention fixtures pass. |
| R26 | automated | active | 18 industry scenarios, funded defense procurement and 7 research fixtures exist; phone playability per industry is not yet evidenced. |
| R27 | automated | active | R&D/brand/quality rules and tests exist; marketing demand now funds media. |
| R28 | automated | pending | Bounded rival proposals, cohorts and wage/build guards implemented; needs archetype win/lose fixtures. |
| R29 | automated | active | 24+ distinct cards are validated and every choice is opened and resolved in isolation with ledger checks; R24/R28 and the blocked chain still prevent ledger sign-off. |
| R30 | automated | done | `evidence/R30-saves.json`; eight SaveLifecycleTests passed on the macOS runner, including atomic replace, interrupted write, last-good fallback and bounded slots. |
| R31 | cross-platform | done | `evidence/R31-integration.json`; simulator XCTest command/save/load/error. |
| R32 | human | pending | Core screens exist; the smallest-phone offline route has not run because the app is uncompiled since PR #11 (Actions billing). |
| R33 | human | pending | Every corporate family has a phone flow and a session-level workflow test; the phone route has not been compiled or run since PR #11. |
| R34 | automated | pending | Score components, bounded weights, pause and continue are implemented and tested; R29 and the blocked chain prevent ledger sign-off. |
| R35 | human | pending | Seven-step onboarding implemented; accessibility audit and final assets pending. |
| R36 | automated | active | 1,000 five-year and 100 thirty-year conservative runs are in `evidence/balance-report.md`; no errors or rare crash cliff, but the passive policy loses control in the median thirty-year seed and no human has judged difficulty. |
| R37 | device | blocked | Representative/grown/stress fixtures and a Linux engine baseline exist (evidence/performance-baseline.md); A15-class device measurement needs a signed build. |
| R38 | human | blocked | Needs three independent testers; none recruited. |
| R39 | human | active | Icon, privacy manifest and release notes exist; owner review of metadata/rating/attribution pending. |
| R40 | cross-platform | active | Unsigned archive passes; signed export blocked on Apple credentials. |
| R41 | apple | blocked | No App Store Connect key/app record. |
| R42 | device | blocked | Depends on R41 and a real iPhone. |

## Global blockers

1. GitHub Actions is running on the now-public repository. Verify and Apple smoke are green on
   main `981bf91`; the remaining hosted work is the signed Release workflow.
2. Apple signing/TestFlight credentials are absent. The app record and the four App Store Connect
   secrets are still required; `apple/RELEASE.md` and `evidence/handoff-muse.md` list them.
3. No physical device and no independent human testers for R37, R38 and R42.
4. The official corridor crosswalk is unavailable, so R01/R02 stay blocked behind real data.
