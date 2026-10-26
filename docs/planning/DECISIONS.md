# Decision register

| ID | State | Decision | Rationale / revisit condition |
|---|---|---|---|
| D01 | Owner confirmed | Polished Great Lakes first; national later | Finish a coherent iOS product before expansion. |
| D02 | Owner confirmed | Investor persists across company loss | Makes control and bankruptcy meaningful without ending career. |
| D03 | Owner confirmed | Open-ended plus five-year score | Both empire-building and bounded sessions. |
| D04 | Owner confirmed | Premium offline | No mandatory account, server or ads. |
| D05 | Owner confirmed direction | Rust + SwiftUI, weekly simulation, regional model | Challenge if early packaging or economic evidence warrants it. |
| D06 | Proposed | January 2000, 2004 BEA boundary vintage | Baseline vintage is promising; source coverage still unproved. |
| D07 | Proposed | All named corporate systems in bounded release versions | Owner said most if not all; do not silently omit one. |
| D08 | Proposed | No circular equity ownership in first release | Cycle rejection avoids recursive valuation/control exploits; broad subsidiary trees remain allowed. |
| D09 | Proposed | One ordinary voting share class initially | Separates votes from ownership in the model; dual-class issuance can wait for a justified requirement. |
| D10 | Proposed | Quarterly/annual finance using a 52-week game calendar | Clear integer cadence; UI calls it a game calendar, not exact civil weeks. |
| D11 | Proposed | No iCloud in first release; export/import saves | Avoid a new entitlement/conflict domain before local recovery is reliable. |
| D12 | Proposed | Relative CI performance checks plus device absolute targets | Shared runners cannot prove A15 wall-time limits. |
| D13 | Proposed | Session coordinator owns AI plus simulation | Prevents cyclical crate dependencies and preserves one command path. |

Do not use this register to invent owner approval. For changes, add date, evidence and
consequences, update affected contracts/requirements, and retain the old decision history.

## Architect-selected implementation baseline, 2026-10-25

The owner clarified that DeepSeek should execute the thinking done here. The exact
initial algorithms, economic constants, 52-week calendar, one-pass rationing, funded
banking, one share class, five-seat boards,90-percent fictional squeeze-out, regulator
thresholds, thirteen-week reporting, one-percent aggregate game scale, no first-release
reinsurance and bounded policy model are now prescribed in docs/rules/. These are
author-selected defaults, not newly claimed owner quotations. Implement first; change
only with documented economic/test evidence and architect decision. Product choices
D01-D05 remain owner authority. Source observations remain distinct from calibration.

## Implementation decisions, 2026-10-25 (DeepSeek goal session)

These are engineering choices inside the approved design, recorded with their evidence and
revisit conditions. None changes an owner product decision.

| ID | State | Decision | Consequence / revisit condition |
|---|---|---|---|
| D14 | Implemented | Saves use the pinned postcard payload with a BLAKE3 checksum, length bounds and no compression in v0.1. | Build stays pure Rust on every target and the 64 MB bound is enforced directly. Revisit with measured save sizes; adding zstd must preserve the checksum-over-payload and bounds rules. |
| D15 | Implemented | Spot books are per destination region; inter-region trade runs through contracts and shipments with freight and lead time. | No stock teleports and the one-pass allocator stays bounded. Cross-region ad-hoc spot can be added later without changing conservation rules. |
| D16 | Blocked fixture | Corridor uses the verified 2004 EA codes and names with no county membership rows, plus labelled calibration. | R01/R02 cannot be signed off until a provenance-traceable crosswalk is recovered. Never replace the fixture with invented counties. |
| D17 | Implemented | Within the 32-account chart, bank customer loans and customer deposit claims are carried in `Receivables`. | Keeps one chart for all legal entities and prevents double-counting outside cash. Documented in the banking module; revisit if a separate receivable class is justified. |
| D18 | Implemented | The full command catalogue crosses FFI as JSON with a typed rejection record; frequent phone flows also have typed methods. | Keeps the boundary small while every family is reachable and validated by the engine. Revisit if DTO cost or type safety on a hot path warrants typed methods for more intents. |

## Corridor viability calibration, 2026-10-25 (second pass)

Measured with `cw-cli report` and `cw-cli run` on seed 3. These are authored game-calibration
changes, not historical claims. Revisit after the P33 distributions are published.

| Change | Before | After | Evidence |
|---|---|---|---|
| Seeded firms start fully staffed | new firms ramped from zero; wages paid for idle capacity while revenue lagged | 23 of 31 firms dissolved at week 52 | `crates/cw-sim/src/economy/seed.rs` test `seeded_operating_firms_start_fully_staffed` |
| Baseline supply contracts split by supplier capacity and renewable yearly | one arbitrary supplier per buyer, contracts expired at week 52 | 2 of 31 dissolved at week 52 | industry and simulation tests |
| Gateway buy side for every physical good, above input cover and excluding contract-committed stock | producers without a local buyer earned nothing | above | `gateway_export_never_sells_stock_promised_to_contracts` |
| Layoffs on a lower production target with one week severance | workers retained forever, so loss-making firms could not cut payroll | above | labour tests |
| Rival wage raises capped at 110 percent of prevailing | weekly +5 percent compounded into a wage spiral | above | `wage_offers_never_chase_above_the_regional_ceiling` |
| Media crew 5/10/10, carrier 400 lots and 40/10/2 crew, generator 100 fuel and 12/12/3 crew | service firms structurally unprofitable at corridor demand | media and carriers still thin, measured weekly | `cw-cli report` |
| Gateway capacity sized to the corridor's weekly volume | imported goods were capped at 20 percent of input demand, starving retail | retail revenue up from 409k to 684k per week | `cw-cli report` |

Still failing at week 260: 16 of 31 firms dissolved and the guided firm restructures. The next
pass must price outputs to cover delivered input cost plus payroll at realistic utilisation, and
must give media and carriers demand that scales with corridor activity. Do not call the economy
viable until a 260-week run keeps most rivals alive and the guided firm solvent under a stated
reference policy.

### Viability calibration result, 2026-10-25

After the second viability pass and this calibration, seed 3 measures:

| Horizon | Dissolved | Past due | Guided firm | Score |
|---|---|---|---|---|
| 52 weeks | 0 of 31 | 0 | operating, $783k cash | n/a |
| 260 weeks | 3 of 31 | 2 | operating, $935k cash | 62 (return 50, resilience 100, control 20, conduct 100) |
| 1,560 weeks | 11 of 31 | 0 | operating, $3.5M cash, fair value up 2.7x | n/a |

Calibration changes behind this (all authored game calibration, not historical facts):
operating plants run at planned full capacity by default; the gateway quotes five percent either
side of the reference index and is sized to the corridor's weekly volume; freight is ten cents
per lot-kilometre; marketing is three percent of trailing revenue for firms with revenue; media,
carrier and generator crews were right-sized; rival expansion stops at three blocks; a profitable
firm restores capacity and marketing.

Remaining: eleven rivals fail across thirty years with no entry, and the score's control
component is low because the investor keeps most wealth in personal cash. That is a playtest and
balance question (P33, P35), not a code defect.

## P33 balance harness and first distributions, 2026-10-25 (DeepSeek goal session)

Engineering choices and defect fixes inside the approved design. No product decision changed.
Evidence: `evidence/balance-report.md` and `evidence/balance/*.json`.

| ID | State | Decision | Consequence / revisit condition |
|---|---|---|---|
| D19 | Implemented | Reference policies live in `crates/cw-cli/src/policy.rs`, decide every four weeks from read models, and submit ordinary commands through `Session::submit`; they never write world state. | Conservative/growth/acquirer are distinct from the rival `cw-ai` cohorts and use the same validator. Revisit constants after human playtests; keep the validator-only rule. |
| D20 | Fixed | Ordinary facilities no longer subtract output from a zero ore deposit. `production` only consumes a deposit when one exists. | 27 negative facility deposits on seed 1 at week 260 became 0; the pinned fixture hash moved `5437029f…` to `3ef82f5b…`. Rust and Swift constants updated; iOS XCTest still pending on the Mac runner. |
| D21 | Fixed | `FoundFirm` uses the same 40% construction-service / 60% equipment-escrow split as `BuildFacility`. | A founded firm no longer carries work in progress plus negative restricted cash. Seed 3 founder conservative at week 260 moved from fair value 0 and score 50 to fair value $117 and score 58. |
| D22 | Open | The first distribution report publishes `apply_macro_price_drift` as a no-op and unresolved optional event cards as the root causes of near-zero seed variance. | 100/100 conservative and growth seeds are identical; do not claim a seed distribution until nominal price drift and event resolution are implemented and this report is re-run. |

Policy iteration is recorded in the report: the first draft throttled output to 85/95 percent
and lost about $8.7k a week against a demand-limited plant; the published policies hold full
capacity and cut discretionary marketing/research instead. This is a harness correction, not a
content tune.

### Rival event responses, 2026-10-25

Optional event cards addressed to a rival now resolve themselves through the same handlers and
ledger postings as player choices, using the event stream to pick among the legal options. Player
cards stay open for an explicit answer. This removes the inert event deck and gives seeds genuine
rival-outcome variance: guided seeds 1-5 at week 260 dissolve 6/4/4/4/4 rivals instead of a single
identical outcome, while the guided investor and score stay stable because the investor makes no
choices. Mandatory player decisions still pause the clock.

### First insurance policy concentration, 2026-10-25

The prescribed 40 percent single-region concentration limit made the first policy impossible: with
no existing book, one region is necessarily 100 percent of coverage, so every non-specialist
insurer refused all business. The check now allows the policy that opens the book and enforces
the 40 percent limit on every later policy. Specialist mode still doubles the capital requirement.
Scenario: `corporate_lifecycle.rs::insurer_binds_and_pays_a_claim_from_reserves`.

### Demand, supply and macro regimes, 2026-10-25

Event and macro modifiers were stored but never applied; crop loss was also implemented as a
demand shock even though the rule is a yield loss. Now:

- Demand modifiers compose once per active cause and scale household and industrial demand for
  their product or, when unscoped, the whole region. Expired modifiers are pruned weekly.
- Supply modifiers scale physical output only. Crop-loss choices use supply, not demand.
- Recessions and booms are drawn from the macro stream at quarter boundaries (authored five
  percent chance per quarter, one or two quarters long) instead of being handed to a regional
  firm card, which let a single rival force a corridor-wide recession.
- Optional cards expire at their deadline with an explicit no-action outcome instead of waiting
  forever and blocking new cards.
- The technology frontier advances one level per game year, capped at twenty.

Measured after the change: guided seed 3 at 260 weeks has 4-5 rival failures and the investor
survives with score 61; at 1,560 weeks 10 rivals fail and the guided firm is operating with 60
percent control. The pinned canonical hash moved to `a3266a99...` and the iOS constant is updated
pending a Mac-runner run.

### Paid skill training, 2026-10-25

Labour now upgrades at most 0.1 percent of a tier's workforce each week, moving workers from low
to mid and mid to high, paid from an explicit weekly firm budget at an authored $500 per worker.
The regional training grant covers twenty percent from the finite government training budget when
active. Training never creates workers: total headcount is conserved and the wage bill rises with
the tier mix. Exposed as `SetTrainingBudget` with a Finance form showing the cost and the cap.

### Bond trading price, 2026-10-25

Bond orders previously transferred principal for one cent per unit and always credited the
investor regardless of the requested actor. Bonds now trade dirty: principal plus coupon accrued
since issue (or the last coupon) at the 52-week rule, paid to the seller; the buyer's investment
basis includes the accrued share. Non-investor orders are refused rather than silently rerouted.
Test: `commands::tests::bond_trades_pay_accrued_interest_on_top_of_principal`.

### Union ballots and explicit guarantees, 2026-10-25

- A successful ballot (organisation at or above 5,000) now forms the bargaining unit and opens a
  demand at the rule's wage floor; a failed ballot returns to idle organisation drift. Before
  this, a ballot card left the firm in a permanent no-op state and bargaining never began.
- Parent guarantees are now called when a liquidated subsidiary still has unpaid claims: the
  guarantor pays from its own cash up to min(claim, cap), never more, and an unrelated sibling is
  never charged. Tests: `unions::tests::a_successful_ballot_forms_the_bargaining_unit` and
  `corporate_lifecycle.rs::a_parent_guarantee_backs_a_failed_subsidiary_only_up_to_its_cap`.

### Finance and policy correctness, 2026-10-25

- The lobbying cooldown now applies after a request resolves and spans the controlled group and
  region, not only unresolved requests on one firm.
- A regional tax modifier applies only while its 52-week window is open.
- The nominal price index uses the macro inflation rate instead of a hardcoded 200 bps.
- Loan interest carries sub-cent remainders across weeks: 100,000,000 cents at 8 percent posts
  exactly 2,000,000 cents over thirteen weeks instead of losing cents to weekly rounding.

Tests: `credit::tests::weekly_interest_carry_posts_the_exact_term_amount` and
`policy::tests::the_cooldown_survives_resolution_and_spans_the_group`. Canonical hash re-pinned to
`893ccbe0...` because the persisted interest carry is part of state.

### Legal merger settlement, 2026-10-25

The merger path previously moved a book value to `OtherAssets` and stranded the target's cash,
debt, facilities, contracts, loans, bonds and policies on a shell. A merger now:

- transfers every target account balance into the buyer, removes the buyer's investment in the
  target and books the difference to retained earnings, so both ledgers stay balanced and the
  target winds down to zero;
- reattaches facilities, supply contracts, loans, bonds and insurance policies to the survivor;
- retires the target shares and marks it merged.

Test: `corporate_lifecycle.rs::a_legal_merger_moves_assets_debt_facilities_and_contracts`.
Asset purchases remain a separate path and are not merged into this rule.

### Construction service capacity, 2026-10-25

Build orders paid local contractors without any capacity bound while public works also bought their
weekly construction output, so the same capacity could be sold twice. Build service now pays named
contractors only up to their remaining weekly construction capacity, records the committed lots on
the world, and charges the remainder to the finite outside construction sector. Public works
subtracts those commitments before buying stock, and commitments clear at the end of the week.
Test: `commands::tests::construction_service_is_bounded_by_contractor_capacity`; canonical hash
re-pinned to `8e0bf569...`.

### Board authority for large issuance and mergers, 2026-10-25

Boards are apportioned from the seeded cap table at opening, not left empty until the first
thirteen-week meeting. A large issuance (more than ten percent of the issue) and a legal merger
now require the controller to hold at least three of the five seats; small operating issuance stays
with management. Tests: `commands::tests::large_issuance_needs_three_board_seats`, the merger
lifecycle test, and the session workflow test. Canonical hash re-pinned to `c14127ee...`.

### Asset purchase settlement, 2026-10-25

`DealKind::AssetPurchase` previously fell through the share-purchase path and transferred shares.
It now has its own settlement: the buyer pays the agreed price from escrow, receives the target's
facilities, inventory, fixed assets and work in progress at book value, and books the difference
to other income/loss. Shares, cash, debt and contracts stay with the target, which is the
distinction from a share purchase or merger. v0.1 assumes no liabilities are attached to the
asset bundle; the deal preview must say so. Unused escrow (including the two percent cushion) is
refunded. Test: `deals::tests::an_asset_purchase_moves_assets_but_keeps_shares_and_debt`.

### Board limits are enforced, 2026-10-25

The existing thresholds (debt above 60 percent of tangible assets, a single capex above 25 percent
of total assets) are now checked on build, expansion, loan request and loan acceptance, in addition
to large issuance and mergers. A controlled firm needs three controller-held board seats; a widely
held firm with no controller is run by its management in v0.1, because no holder can seat a
majority. The board is still reapportioned every thirteen weeks. 260-week seed 3 is unchanged at
four rival failures and score 61.

### Antitrust remedies are real divestitures, 2026-10-25

An accepted remedy previously only changed a threshold and left the buyer in full control. Review
approval now sells enough target shares to outside holders to take the buyer below half, pays the
buyer fair value, keeps total shares conserved, and excludes the target's sales from the buyer's
group aggregation once control is gone. Concentration also now uses the union of the buyer and
target groups, so a controlled target is never counted twice. Tests:
`deals::tests::a_remedy_divests_enough_shares_to_remove_control` and the corrected
`concentration_is_group_aware`.

### Bond maturity and coupon carry, 2026-10-25

A bond reaching maturity dropped the coupon that fell due on the same date, and quarterly coupons
rounded independently each quarter. Maturity now pays the final coupon before principal, and
coupons use a persisted sub-cent carry. Test:
`credit::tests::a_bond_pays_its_final_coupon_before_principal`.

### Information rights, 2026-10-25

The session exposed every private firm's cash, equity, facilities, cap table, loans and contracts
to the UI. Private data now requires a management right, public disclosure (a completed IPO), or
a diligence that has reached its reveal date for a deal funded by the investor's group. Firm
summaries for private rivals are masked (cash and equity zero, grade hidden, facility count zero,
`restricted` true) while public marks and fair estimates remain visible. The finance screen says a
private company is restricted instead of showing zeros. Tests:
`private_firm_data_needs_management_public_disclosure_or_diligence` and
`completed_diligence_grants_access_to_a_private_target`.

### Bank runs pay actual depositors, 2026-10-25

A run previously reduced the bank's deposit liability and cash without touching depositors'
claims, losing money without a counterparty. Banks now track per-depositor claims in the bank
model; deposits and withdrawals maintain them, and a run pays available cash pro rata, credits
each depositor and reduces their claim. Unpaid claims remain liabilities. Test:
`banking::tests::a_run_pays_depositors_pro_rata_and_reduces_their_claims`.

### Insurance claims settle the following week, 2026-10-25

An incident recognised its reserve and paid immediately from insurer cash, contrary to the rule
that reserves are recognised at the incident and paid next week. Claims now carry a payable week;
`settle_claims` runs at the start of the insurance pass, pays from actual cash, credits the insured
and reduces the reserve. Unpaid reserves stay as liabilities. Tests:
`insurance::tests::a_claim_is_reserved_now_and_paid_next_week` and the updated industry insurer
scenario.

### Pause policy, 2026-10-25

The clock now stops for the investor when a controlled firm is three weeks into default grace, a
first strike starts, a board vote or tender/board deadline is within one week, a mandatory event
opens, or the week-260 score arrives. Advisory pauses are dismissible; mandatory event pauses must
be answered through `RespondEvent`. The CLI's scripted driver clears advisory pauses so balance
sweeps still reach their horizon, and only stops for mandatory events and the score. Tests:
`tick::tests::the_clock_pauses_before_default_grace_expires` and
`the_clock_pauses_on_an_imminent_board_vote`.

### Product quality investment, 2026-10-25

Quality came only from technology. `SetQualityBudget` now funds a weekly per-product quality
investment: each $1,000 adds one basis point, at most twenty-five a week, capped at technology
quality plus the prescribed 1,000 bps ceiling. It is an operating expense, never a guaranteed
sale, and is exposed through a Finance form with a product picker. Test:
`research::tests::quality_investment_is_bounded_and_funded`; canonical hash re-pinned to
`2ee957c5...`.

### Defense procurement, 2026-10-25

Defense demand is now funded procurement rather than a government spot book. Each operative
defense producer in a region receives a 52-week contract at reference price for its recipe
capacity, with a ten percent deposit held as government prepaid expense and supplier unearned
revenue. Settlement accepts only output the budget and treasury can fund, earns the deposit
first, pays the balance on delivery, preserves deferred quantity and charges five percent of
undelivered value after a two-week grace. New and acquired defense plants can win contracts.
Weekly delivery plus grace is the milestone mechanism; no separate milestone object is invented.
The fiscal budgets remain seeded baselines because weekly labour-income tax is not yet posted,
so a trailing-receipt mean would be fabricated. Evidence: `evidence/defense-procurement.md`.

### Labour income tax and trailing budgets, 2026-10-25

Taxable earned labour income now pays twenty percent to the government each week. Corporate
tax payments join the same receipt total, and the procurement, healthcare and training budgets
are recomputed from the trailing thirteen-week mean at the close of every week. Transfer support
is paid after tax and is not taxed. The seed starts with thirteen baseline receipt weeks. Labour
tax is withheld on gross earned wages because the household model has no per-worker cash ledger;
paid-wage-only withholding and the transfer-support budget remain named gaps. Evidence:
`evidence/fiscal-labour.md`.

### Paid-wage labour tax base, 2026-10-25

Labour withholding and household income now use wages firms actually paid in the week,
attributed pro rata across the regions of their employing facilities. Deferred wages are an
overdue liability, not household cash, and are not taxed. Severance stays outside the labour
tax base. The fiscal receipt window, budgets and twenty percent rate are unchanged.
Evidence: `evidence/fiscal-labour.md`; hash re-pinned to `0f6b4a86...`.

### Household disposable baseline and savings supplement, 2026-10-25

The opening household baseline is disposable income after twenty percent labour tax. Baseline
lots and initial savings use that net figure. A week with income below the disposable baseline
may draw at most one percent of savings into the spending budget; the supplement is deducted
when committed and unspent category budgets return to savings. Old saves default to no
supplement. Evidence: `evidence/household-demand.md`; hash re-pinned to `f75e42c2...`.

### Transfer support by tier and unpaid need, 2026-10-25

Background transfer support is now calculated from each tier's baseline wage and reports the
unfunded remainder in `government.unpaid_transfer_need_cents`. The treasury and residual
training/welfare budget still cap payment. Background employers normally absorb the residual
workforce, so support is usually zero; the mechanism is correct when unemployment exists.
Evidence: `evidence/transfer-support.md`; hash `6731ed7c...`.

### Owner-approved authored corridor, 2026-10-25

The owner approved authored regions as the product geography: the best-effort roster is
acceptable and exact historical membership is not required for this release. The eleven region
areas are now frozen as an owner-approved authored corridor. Region codes and names remain the
best-effort roster; population, wage, demand, distance and membership are labelled game
estimates. No county rows, BEA crosswalk or historical observation is claimed. R01/R02 and P01/P05
are evaluated against this approved authored scheme, with the limitation visible in the data,
evidence and store copy.

### Owner direction for 2.0.0: single-player AHD corporations on a real map, 2026-10-26

The owner judged 1.0.0 to read as an operations app rather than an economic strategy game, and
directed a redesign: Corporate Warfare should play as single-player A House Divided
corporations (sector types with operating strategies, plant building, corporation setup) with a
rich, real map. Decisions, all owner-confirmed:

1. Playable units are US states, a Great Lakes and Ohio Valley set: IL, IN, KY, MI, NY, OH, PA,
   WV, drawn with county detail. This supersedes the eleven authored economic areas.
2. Keep Corporate Warfare's deterministic physical engine and adopt AHD's player-facing model
   on top of it.
3. AHD's sector catalogue, strategies, commodities and formulas may be reused freely.
4. A real map (geography, not satellite imagery), bundled and offline.
5. Target iPhone and iPad with SwiftUI now; other platforms later.
6. Versioning: patch for minor updates, minor for big features, major for wide releases. The
   redesign ships as 2.0.0.

The AHD margin modifiers become physical effects in this engine: sector focus changes input use
and adds a coordination overhead, and state conditions change output, freight cost and
shrinkage, each paid to a real counterparty.

