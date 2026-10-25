# Financial, labour, policy and rival rules v0.1

Initial tuning constants below are authored game rules, not observed 2000 values.
Use common ledger, permissions and versioned deterministic RNG. No special AI cash.

## Credit

Base annual policy rate starts at 600 basis points. Commercial spread by credit grade:
A 200, B 400, C 700, D 1200. Each week, grade uses trailing 13-week interest coverage
and debt/tangible assets: A requires coverage >=4 and leverage <=30%; B >=2 and <=50%;
C >=1 and <=70%; otherwise D. Missing positive earnings assigns C unless cash covers
13 weeks obligations, then B. Insolvent/past-due firm cannot obtain new unsecured credit.

Standard loan: up to 50 percent unpledged tangible asset carrying value, 104-week term,
weekly straight-line principal, fixed rate at origination, unpaid fractional principal
on final week. Secured loan debt-service coverage must be >=1.25 from a labelled 13-week
forecast. Personal assets cannot guarantee firm debt without explicit player consent.
Unsecured maximum is 13 weeks positive operating cash flow, and grade A/B only.

Bond: minimum $500,000 issuance, 260-week term, quarterly coupons, principal at maturity,
2 percent issue fee. Only A/B firms, equity >0 and debt/assets after issue <=60 percent.
Buyer cash must be available before issuance. Background institutional investor has finite
capital and at most 10 percent of its portfolio in one controlling group. Subsequent
weekly trading uses actual holder lots and finite float, with accrued-interest settlement.
No repayment by new debt unless a funded refinancing actually settles before maturity.

## Banking

Banks are funded intermediaries in v0.1, not fractional-reserve money creators. Cash is
outside settlement currency. A customer's bank deposit is a financial asset; the matching
bank deposit is a liability, never a second copy of outside cash. Deposit transfers cash
to bank and credits depositor claim. Withdrawal redeems claim for bank cash, constrained
by actual liquidity. Spending a deposit either transfers a same-bank claim or redeems
and pays through the settlement account; two sides always match. Display liquid funds
separately from freely available outside cash.

Bank opens with paid capital and can accept household/firm deposits. Annual deposit rate
starts at base-200 BPS, minimum zero. Required liquid cash >=20 percent of withdrawable
deposits; adjusted capital >=12 percent of risk-weighted loans. No lending if either
post-loan condition fails. Plain business loan risk weight 100 percent; government
security weight 20 percent. Lending transfers bank cash to borrower, creates loan asset
and matching borrower debt. Loan principal repayment reverses those balances; interest
is bank revenue. Customer deposit interest is expense and liability increase, with no
invented bank cash. A bank must fund withdrawal of accrued interest from actual receipts.

When loan enters default, provision 50 percent uncovered principal immediately, adjust
on recovery and remove written-off principal. Loans and bank deposits are excluded from
ordinary inventory/production recipes. Staffing and infrastructure are real overhead.
Deposit demand allocation considers offered rate and perceived solvency. A bank run
withdrawal request = 25 percent deposits in a region-shock week, pro rata settlement
from available cash; unpaid claims remain. Four-week unresolved liquidity failure enters
restructuring. No deposit insurance/bailout unless an explicitly funded scenario program.

Bank equity fair value = max(0, cash + loans_after_provisions + other_assets + stake_values
- deposit_liabilities - own_loans - bonds - other_liabilities). No additional EBITDA
enterprise multiple or second debt subtraction. This overrides ordinary-company value.

## Insurance

Initial line: property/business-interruption cover for declared facilities. Each policy
has region, insured asset, coverage limit, deductible, weekly premium and 52-week term.
Annual quoted premium = limit * base_risk_bps /10000 * 1.25. Base annual expected-loss
risk starts at 200 BPS, regional multiplier 0.75..1.5 from labelled calibration. Payments
weekly with accrual remainder. Half the annual premium is initially unearned reserve
only if prepaid; otherwise accrue earned premium for elapsed coverage, not an arbitrary
upfront asset. Claim reserves are distinct liabilities from unearned premiums.

Insurer accepts a policy only if capital >=10 percent total coverage and unrestricted
cash >=two weeks overhead +5 percent coverage. Risk also limits single-region coverage
to 40 percent total portfolio limit unless the insurer explicitly chooses concentrated
specialist mode with doubled capital requirement. All quoted constraints preview in UI.

Each week, idiosyncratic loss per insured facility occurs when deterministic annualized
hazard draw hits; weekly threshold = annual risk probability/52 with accumulator-safe RNG
threshold. Conditional loss severity uses 10/30/100 percent asset value with weights
70/25/5. Expected-loss calibration must divide probability by expected severity, rather
than treating a 2 percent loss ratio as a 2 percent full-destruction chance. For default
200 BPS loss ratio and expected severity 0.195, annual incident probability is
round_even(0.02/0.195) = 1026 BPS. Region catastrophes are separate scenarios/events;
remove their expected loss from idiosyncratic calibration if included in the baseline.

One correlated event draw per region/week applies to all affected policies, never a
separate independent draw pretending to be a catastrophe. Claim = min(limit, max(0,
physical insured loss - deductible)). Expense and reserve at incident, payout next week,
reserve reduction on payment. Actual damaged facility loses asset value/capacity once;
claim compensation does not reverse physical damage. Insurance reserves and firm debt
remain distinct in valuation. Reinsurance is deferred from v0.1, explicitly; underwriting
concentration remains a decision, not a missing model fix.

## Unions

Store workforce organisation BPS and bargaining state per firm/region. Organisation
starts at the region calibration (default 1500 BPS estimate if unavailable, visibly flagged).
Weekly growth +25 BPS when pay <90 percent prevailing or layoffs >10 percent last 13 weeks;
otherwise -5 BPS to a floor of 500. At >=5000, organisation ballot can form a bargaining
unit. Ballot uses stable worker-share support from wages/conditions, not rerolled on reload.

Union demand every 52 weeks: wage floor max(current wage*1.05, prevailing*1.02), plus
one-week severance for future layoffs. Firm can accept, counter at >=current wage, or
reject. Counter accepted if >=98 percent demand with two-week no-layoff pledge. Two-week
unresolved deadline starts strike: 70 percent affected workers unavailable, no wage for
striking workers, organisation +50 BPS/week, one news/reputation event. Settlement above
95 percent initial demand ends strike next opening; accepting a counter uses updated
support only once/week. Max strike duration 13 weeks, then binding median of demand and
last solvent offer, constrained not to require an impossible payment. Insolvency remains
possible. UI always states costs and choices. AI follows these same thresholds.

## Policy and antitrust lobbying

Three request types: permitting acceleration (build duration -25 percent for future
projects in region for 52 weeks), training grant (funded subsidy of 20 percent retraining
cost), and tax adjustment (-200 BPS regional corporate tax modifier for 52 weeks).
Request costs max($25,000, 0.1 percent group assets), paid to outside advocacy sector.
One active request/group/region, 26-week cooldown. Four-week decision. Success probability
= clamp(2000 + reputation_bps/4 - recent_policy_wins*500, 1000, 5000) BPS. One draw saved
at request creation. Funding a request cannot guarantee its result. Costs remain on failure.

Training grants need a finite government budget or are declined. Tax modifier cannot
make total tax negative. No option to buy instant antitrust approval. A failed lobbying
campaign with concurrent mass layoffs can reduce reputation 250 BPS, recorded publicly.
These are fictional transparent game institutions, not a full political simulator.

## AI policy cookbook

Every firm evaluates urgent debt/default, strike and offer deadlines weekly. Strategic
policy cohorts are stable firm_id modulo 4, so each firm receives one strategic decision
slot every four weeks regardless of firm count. Each slot examines at most eight regional
candidates, four product/contract alternatives and four actions. Process observations and
scores in stable-ID order. Work scales linearly with population; do not claim constant
cost independent of firm count. Persist cohort, last-action cooldown and pending proposals.

Common order: (1) preserve solvency/reserves; (2) fix production bottleneck; (3) price and
stock; (4) expand/R&D/finance/M&A. Reserve target = four weeks mandatory outflow plus due
principal next 13 weeks. If projected cash below target, cut discretionary spending, sell
unpledged inventory/stakes, seek legal funded credit, then restructure. Never gift cash.

Pricing: start at delivered unit cost * target markup, compare to regional reference;
if prior 4-week sell-through <50 percent, price -5 percent; if >90 percent and margin
positive, price +2 percent. Clamp price 50..200 percent reference, except no lower than
variable cost unless vulture liquidation. Inventory target four weeks forecast inputs,
order deficit with available cash; household/firm forecast uses actual last 4 weeks.

Build candidate NPV proxy = 52-week incremental contribution - capex - financing cost.
Only build if positive, forecast post-action cash >= reserve target, local labour feasible,
and planned capacity <=1.25*observed reachable unmet demand. No omniscient hidden figures.
Acquisition candidate uses public fair estimate with 20 percent uncertainty haircut unless
diligence reveals data. Require post-deal debt/assets <=60 percent and positive combined
operating contribution. NPC public holders tender using CORPORATE reservation factors.

Archetype weights on [cost saving, growth, quality/brand, R&D, acquisition]:
Cost leader [5,3,1,2,1]; Brand builder [1,3,5,2,1]; Consolidator [2,2,1,1,5];
Technologist [2,2,2,5,1]; Vulture [3,1,1,1,5] with distressed-price preference.
Score = weighted integer projected 52-week net cash benefit - risk_penalty. Sort descending,
then command kind/target stable ID, attempt at most four legal actions. If rejected,
log reason; do not mutate directly. Each archetype must win and lose in reference runs.

## Fully specified AI candidate calculations

Candidate region shortlist is the cheapest eight delivered-cost regions in last observed
public data, excluding those without legal access; ties by region ID. If fewer than eight,
use all. No hidden current rival cost sheets. Candidate price/order actions use actual
last4-week own volumes and costs. Build projected annual contribution = min(new_capacity,
last4week_mean_reachable_unmet_demand) * unit_contribution *52, with unchanged observed
prices/wages and forecast label. Risk penalty = capex/10 + max(0,due13week_debt -
post_action_free_cash). Negative unit contribution rejects before scoring. R&D savings
benefit = last13week_eligible_input_and_payroll_cost *4 *200/10000, minus threshold
funding cost and success probability adjustment7500/10000. Brand benefit forecast uses
last4week incremental sales per observed brand change; until enough data, budget at most
1 percent revenue and score neutral rather than invent sales elasticity. Acquisition
benefit uses target disclosed annual cash flow less interest/integration cost and bid
premium; undisclosed target earnings haircut20 percent. Archetype multiplies these same
cash benefit categories. For ties: safety action first, then lower target ID. At most one
new build and one new deal per firm's four-week strategic slot. Routine existing policies
continue each week without consuming an extra strategic decision.

Credit ratio zero cases: debt/assets is zero if debt is zero, otherwise unlimited when
tangible assets are zero. Interest coverage is unlimited only when interest due is zero
and EBITDA positive; zero when EBITDA<=0. Negative equity is always gradeD, regardless
of cash balance. Forecast debt service zero makes coverage test pass only if forecast
operating cash is nonnegative. All thresholds use cross-multiplied integer comparisons.

The thirteen-week bargaining fallback wage is midpoint(demand,last_offer), rounded to
cents, capped at the forecast affordable payroll per affected worker using last13-week
realised receipts minus non-labour mandatory outflows. Never lower existing contractual
wages by this cap. If capped affordable wage is below an existing contractual floor,
record failed settlement and enter normal payment-grace/restructuring instead of pretending
to fund it. It is possible for the company to fail; there is no free arbitrator bailout.
