# Release readiness map

Updated 2026-10-25 against main `30a545c`. This maps every release requirement to its real
evidence or its concrete blocker. It is not a completion claim: a requirement is only done when
its ledger entry names reviewed evidence that satisfies the gate.

| Req | Gate | State | Evidence or blocker |
|---|---|---|---|
| R01 | automated | blocked | Official 2004 BEA county-to-EA crosswalk unrecoverable (BEA redirects/404, archive 429). Codes/names verified, county rows absent. |
| R02 | automated | blocked | Depends on R01; dataset is labelled engineering calibration. |
| R03 | automated | done | `evidence/R03-ledger.json`; ledger/units/farm worked example. |
| R04 | automated | done | `evidence/R04-commands.json`; handles, authority, rollback, duplicate IDs. |
| R05 | cross-platform | active | Linux hash pinned and batching/replay tested; iOS matched at PR #11. Current hash `5437029ff...` needs a Mac-runner pass after Actions billing is restored. |
| R06 | automated | pending | Households, background employment, government budgets and finite gateways implemented; needs an independent reconciliation fixture. |
| R07 | automated | pending | Labour allocation, 2% search, tier offers and wage clamp implemented with finiteness tests. |
| R08 | automated | pending | Production, inputs, capacity, service expiry implemented; industry fixtures cover the recipes. |
| R09 | automated | pending | One-pass allocator and the four worked examples are tested; needs a cash/stock conservation fixture through the full tick. |
| R10 | automated | pending | Contracts, shipments, freight, shortfall penalties implemented; carrier/freight tests exist. |
| R11 | automated | pending | Build/expand/retool/mothball/dispose, escrow, lead time implemented; construction test covers escrow split. |
| R12 | automated | pending | Investor, guided start, recovery contract implemented; needs a control-loss continuity fixture. |
| R13 | automated | pending | Ownership, cycle rejection, consolidation implemented; needs deep/wide group fixtures at scale. |
| R14 | automated | pending | Intercompany loans and transfer contracts implemented; internal profit elimination needs a fixture. |
| R15 | automated | pending | Exchange, finite float, impact, valuation implemented; needs a holder-transfer/look-through fixture. |
| R16 | automated | pending | Issuance, dividend, buyback, IPO implemented with treasury exclusion; needs share/vote conservation at scale. |
| R17 | automated | pending | Boards, seats, CEO, votes implemented; needs deadline/deadlock fixtures. |
| R18 | automated | pending | Loans, bonds, grades, covenants implemented; needs principal/coupon edge fixtures. |
| R19 | automated | pending | Banking liquidity/capital/run implemented; needs a deposit-loan-withdrawal chain fixture. |
| R20 | automated | pending | Underwriting, reserves, claims, catastrophe implemented; needs severity/claim fixtures. |
| R21 | automated | pending | Deal state machine, escrow, tender, squeeze-out, merger implemented; needs full lifecycle fixtures. |
| R22 | automated | pending | HHI, review, remedy implemented; needs concentration threshold fixtures. |
| R23 | automated | pending | Unions, bargaining, strikes, fallback implemented; needs deadline/strike fixtures. |
| R24 | automated | pending | Lobby requests, cooldown, saved draw, reputation implemented; needs repeat/backlash fixtures. |
| R25 | automated | pending | Grace, restructuring, liquidation waterfall implemented; needs secured/unsecured fixtures. |
| R26 | automated | active | 18 industry scenarios and 7 research fixtures exist; phone playability per industry is not yet evidenced. |
| R27 | automated | active | R&D/brand/quality rules and tests exist; marketing demand now funds media. |
| R28 | automated | pending | Bounded rival proposals, cohorts and wage/build guards implemented; needs archetype win/lose fixtures. |
| R29 | automated | pending | 24 cards and seeded firms exist; every choice is resolved in tests. |
| R30 | automated | active | Checksum, bounds, adversarial rejection and Swift recovery code exist; the Swift lifecycle suite has not run on the Mac runner. |
| R31 | cross-platform | done | `evidence/R31-integration.json`; simulator XCTest command/save/load/error. |
| R32 | human | pending | Core screens exist; the smallest-phone offline route has not run because the app is uncompiled since PR #11 (Actions billing). |
| R33 | human | pending | Every corporate family has a phone flow and a session-level workflow test; the phone route has not been compiled or run since PR #11. |
| R34 | automated | pending | Score at week 260 works (seed 3 total 62); needs explainability and continue fixtures. |
| R35 | human | pending | Seven-step onboarding implemented; accessibility audit and final assets pending. |
| R36 | automated | active | 640-run sweep and distributions in `evidence/balance-report.md`; rival outcome variance is now non-zero but score variance is still flat for a passive investor. |
| R37 | device | blocked | Representative/grown/stress fixtures and a Linux engine baseline exist (evidence/performance-baseline.md); A15-class device measurement needs a signed build. |
| R38 | human | blocked | Needs three independent testers; none recruited. |
| R39 | human | active | Icon, privacy manifest and release notes exist; owner review of metadata/rating/attribution pending. |
| R40 | cross-platform | active | Unsigned archive passes; signed export blocked on Apple credentials. |
| R41 | apple | blocked | No App Store Connect key/app record. |
| R42 | device | blocked | Depends on R41 and a real iPhone. |

## Global blockers

1. GitHub Actions billing fails before job start, so hosted gates and the current iOS hash
   verification cannot run.
2. Apple signing/TestFlight credentials are absent. `apple/RELEASE.md` lists the exact secrets
   and owner actions.
3. No physical device and no independent human testers for R37, R38 and R42.
4. The official corridor crosswalk is unavailable, so R01/R02 stay blocked behind real data.
