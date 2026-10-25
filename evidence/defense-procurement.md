# Defense procurement

Rule source: `docs/rules/CONTENT-PACING.md` (funded procurement, ten percent deposit,
payment on accepted delivery, two-week grace and a five percent undelivered-value
penalty) and the government fiscal ceiling.

## Model

- Every operative defense producer, in each region where it has defense capacity, is
  offered a 52-week contract for its recipe capacity at the reference price. The offer is
  not reserved for seeded firms: a newly built, purchased or player-controlled defense
  plant wins the same contract after it becomes operative, and a completed contract is
  renewed while the producer remains operative.
- The government pays a ten percent deposit on the 52-week contract value from treasury.
  It records a prepaid asset; the supplier records cash and unearned revenue. This is the
  only cash the supplier receives before delivery.
- Settlement runs in phase 1 against opening stock. Accepted delivery earns the deposit
  first, then pays the cash balance from the weekly procurement budget and treasury. A
  contract never accepts more value than the budget plus the deposit already held, so an
  unfunded delivery is not booked as a sale. Deferred quantity is preserved in
  `government.pending_procurement`.
- A supplier shortfall of any part of the weekly quantity increments a grace counter. After
  two weeks the supplier pays five percent of the undelivered value to the government. A
  government funding shortfall is deferred, not penalised.
- At contract end, unearned deposit is refunded to the government. Accepted deliveries are
  recorded in the supplier's weekly unit sales, so defense revenue and trailing units are
  visible in the normal company reports.
- Defense is no longer a government spot book; the healthcare spot book and its budget are
  unchanged.

## Evidence

Tests in `crates/cw-sim/src/economy/procurement.rs`:

- `defense_procurement_is_funded_by_a_ten_percent_deposit` checks the contract quantity,
  the deposit, and the matching government prepaid asset and supplier unearned liability.
- `accepted_delivery_earns_the_deposit_and_pays_the_supplier` runs real weeks and checks
  the deposit falls only when output is accepted.
- `a_shortfall_beyond_two_weeks_costs_five_percent` removes stock and checks the exact
  penalty and both ledgers.
- `an_unfunded_week_is_deferred_not_penalised` sets the budget to zero and checks that no
  stock is removed, no cash moves, the shortfall counter stays clear and the unmet quantity
  is preserved.
- `completing_a_contract_refunds_the_unearned_deposit` checks completion and refund.
- `every_operative_defense_producer_can_win_a_contract` retires the seeded contracts and
  checks that the government reopens demand for the producer.
- `defense_procurement_is_capped_by_the_government_budget` in
  `crates/cw-sim/tests/industries.rs` checks that a funded budget buys only the lots it can
  pay for, leaves the rest unsold, and has a cash counterparty.

The 260-week seed-3 baseline is back to four rival failures, the guided firm in control and
score 61. The 100-seed conservative five-year distribution is score 56/62/62, dissolved
median five (range 3-10), controlled median one. Canonical fixture hash is
`2538fa8ba2174503b1ad9931ec5c7ebea9f46c9cacc4fcdede4c2d67c8cc38fe`.

## Known limits

- Milestones are represented by weekly accepted deliveries and the two-week grace, not by
  separate quarterly milestone objects. No extra milestone bonus or payment tranche is
  invented.
- The three fiscal budgets are still seeded from baseline tax receipts and are not
  recalculated from a trailing thirteen-week mean. Labour income tax is not yet collected
  weekly, so there is no honest receipt series to smooth; this is recorded as an open
  fiscal gap rather than faked.
- There is no phone screen listing government contracts yet. A player defense firm receives
  the contract automatically and sees the cash, unearned revenue and defense sales in the
  existing statements.
