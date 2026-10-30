# Financial, labour, policy and rival rules v0.1

Initial tuning constants below are authored game rules, not observed 2000 values.
Use common ledger, permissions and versioned deterministic RNG. No special AI cash.

## Credit

The policy rate, inflation, expected inflation, neutral real rate, target inflation, opening
credit spread and opening wage growth come from the scenario's `macro.json` (1.9): calibrated
game estimates of its period, so Texas 1981 opens near 10 percent inflation with the policy rate
at 16.5 percent, Ruhr and Rhine 1991 at 3.8 percent and 7 percent, Silicon Valley 1995 at 2.8
and 5.75, Great Lakes 2000 at 3.4 and 6.5. The mechanics are the same in every scenario. The
central bank meets every six weeks and moves a quarter point (half a point when far off) toward
a Taylor rule around that neutral rate and target; the rate ceiling is 20 percent. The ten-year
yield is derived: a quarter of the policy rate plus three quarters of (neutral real rate +
expected inflation), a 75 point term premium and an eighth of inflation above target; a test
holds each opening yield to the period's figure. Lenders add a tenth of inflation above target
to every loan quote. Opening wage growth runs above or below inflation by the scenario's gap and
fades into inflation evenly over 104 weeks; after that wages follow the price index. Opening
loans carry the opening policy rate plus the opening credit spread plus the grade B spread.
Commercial spread by credit grade:
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

## Character cast

`content/cast.json` lists twelve people; a person's index is their id in saves. Roles are what
a person can be asked to do: chief executive, central bank chair, governor, party leader.
Chair-capable people carry a stance from -100 (dovish) to +100 (hawkish) that replaces the old
random chair draw. At world start, from a stream seeded by the world seed, posts are filled in
order: chair, the two party leaders, three governors, then a chief executive for each rival in
handle order while more than two people remain free. Nobody holds two posts. People whose home
scenario matches the world's scenario score 10 higher; for governors a matching home state adds
20; for chief executives a temperament that suits the firm archetype adds 5 per step (best 2,
good 1). Ties draw from the stream.

At a chair term end the successor is the free chair-capable person, from the scenario's own
people when any are free, whose stance is nearest the government's drawn stance; with nobody free
the incumbent is reappointed. Governors are redrawn after each election.

Every fourth week a board reviews a rival whose chief executive has served at least 52 weeks. If
each of its last six four-week periods since the appointment lost money, there is a 25 percent
chance the board replaces the chief executive with a free person, preferring a different
temperament. The old chief is benched for 104 weeks. Merged and liquidating firms release their
chief. Each change is news with the new person's portrait, a dossier move and a telemetry count.
An activist demand for a new chief executive replaces the named person the same way. In the player's primary company the board takes the same step after two missed mandates in a row (CORPORATE.md, Executives). Rivals also hire a chief financial officer, chief operating officer and logistics chief through the same commands as the player, only when the effect pays for the pay. A firm's
structure weights (borrowing, transfer pricing, listing, partnering, resistance) are its
archetype's habits pulled three parts in four toward the chief executive's temperament.
Telemetry carries counts only, never names. Portraits are `apple/Resources/cast-portraits.json`,
generated by `tools/cast_portraits.py`: a 32 by 32 bust with three moods (neutral, pleased,
displeased) and a 16 by 16 reduction for every person, and a 64 by 64 office portrait for the named
chiefs. The session picks the mood per appearance (`cw_data::Mood`): letters from a deal, a beaten
raid or good news are pleased, from a lost raid, a walkout or a poached executive displeased, and
the rest neutral; a dossier follows the firm's latest raid; the paper's byline takes it from the
kind of story. The cast has forty named people in `content/cast.json`; saves name people by their
position, so the first twelve never move and new people are only appended. The roles `union_leader`,
`journalist` and `analyst` are filled by the scenario's own person (`cw_data::cast::scenario_person`)
and fall back to an office for a custom scenario. The roles `cfo`, `coo` and `logistics` mark named
people for hire (below).

## Rival personalities

A rival's style is its chief executive's temperament; a rival with no named chief plays the
temperament its archetype suits (cost leader and technologist builder, brand builder
integrator, consolidator financier, vulture raider). The style does five things.

1. Archetype weights on [cost saving, growth, brand, R&D, acquisition] are pulled three parts in
   four toward the temperament's own: integrator [2,4,2,2,3], raider [4,2,4,1,5], financier
   [2,2,1,2,5], speculator [2,2,1,3,2], builder [3,5,2,3,1], cautious [4,2,1,1,1]. Gates keyed
   to a weight (acquisition at 5, undercutting at 3, building at 2, entering a market on a
   smaller import gap at 5) move with them.
2. Every discretionary move (build, retool, raid, bid, supplier bid, bond, venture, listing,
   spin-off, carrier build, lane reservation, storage build, buying into store) has a class and
   an expected value in cents, taken from the generator's own projection. Utility is the value
   times the temperament's class weight in percent (100 neutral), times the stance toward the
   target, and for hostile classes times difficulty aggression (Easy 60, Standard 100, Hard
   140). A move below 100 percent is passed over on a stable draw seeded by the world seed,
   firm, week and class, so the same percentage of such moves go ahead and a reloaded save
   chooses identically. Survivors rank by utility, then command and target, and the best five
   are kept. Routine moves (prices, budgets, wages, shipping policy, selling stock, deadlines)
   bypass the selector. Every move still passes the common command validator.
3. Sharpness: Easy misjudges a move's value by up to 40 percent either way, Standard 15,
   Hard not at all, again by stable draw.
4. Money habits: a raid campaign (budget plus surcharge) may cost at most half of what the firm
   earned over its last thirteen weeks, and a firm with no profit does not raid, on top of the cash
   and reserve tests; the cautious hold a reserve target half again as large; raiders spend 70
   percent more on marketing; a raider accepts a raid short of paying for itself by up to
   `(weight - 100) / 4` percent of its budget; a speculator buys into store on a 5 percent dip
   in any phase but a boom.
5. Own storage: an integrator or speculator builds a store when three years (five for a
   speculator) of avoided overflow fees, four fifths of the cold-store spoilage it suffers and,
   for a speculator, a tenth of the value of stock it could afford in a class with a good under
   its opening price, beat the build and its financing at the policy rate.

Memory is kept per group, by its controlling firm, and the investor is a possible target but
never holds a grudge. A raid gives the defender a grudge of 3,000 plus 600 per block taken
(1,500 if it failed). A hostile bid gives 2,500. Reserving lane capacity once the lane is 60
percent full gives each other shipper there 600 plus a fifth of its share of the lane in basis
points. A joint venture gives its partners an alliance of 5,000, held at 4,000 while it runs; a
friendly share or asset purchase gives 2,500. A new grudge halves an alliance between the two
groups and a new alliance halves a grudge. Grudges fade 1 percent of their strength plus 1
each week, alliances 0.6 percent plus 1, and are dropped under 150. At 1,000 a memory is
shown in the dossier. Hostile moves are multiplied by `(100 + grudge / 40)` percent and by
`(10,000 - 2 * alliance) / 10,000`, and are vetoed against an ally at 5,000; a partnership is
vetoed against a grudge at 5,000 and favoured by `alliance / 50` percent. Raid, bid and
supplier-bid targets honour the same stance. Telemetry carries counts by temperament and class,
grudges and alliances formed, and moves made against a grudge. Never names.

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

Build candidate NPV proxy = 156-week incremental contribution at public prices - capex - three
years' financing at the policy rate.
Only build if positive, forecast post-action cash >= reserve target, local labour feasible,
and planned capacity <=1.25*observed reachable unmet demand. No omniscient hidden figures.
Unmet demand for goods is the whole corridor's smoothed imports, since goods trade on one
exchange; for other products it is the state's own. A goods plant's output is valued at what it nets at
the best importing gateway after freight from the site, so a distant site wins only when its
lower costs outweigh the freight. Growth-minded archetypes (growth weight 3)
build when imports cover four fifths of a plant; cautious ones (weight 2) only when imports cover
a full plant; vultures do not build. A rival stops building new plants at thirty plants.
Imports lag a new plant: the room a rival sees is the smoothed imports less the weekly output of
plants of that good still being built or opened in the last eight weeks, never below zero, so
rivals that see the same imports do not all build for them in one quarter.
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
