# Operating economy rules v0.1

Author-selected implementation baseline, 2026-10-25. These are game-design constants,
not measured historical facts. Implement them before tuning. Parameter changes require
scenario diffs and a decision entry; do not invent a different solver during coding.
This file resolves allocation/timing ambiguity in the planning contracts.

## Arithmetic and cadence

Game year is exactly 52 weeks, quarter 13. One tick closes one week. Store money in
cents, physical quantities in milli-lots and rates in basis points. Tables in CONTENT
use whole lots for readability. Use i128 intermediate multiplication; check conversion
to i64. Prices are cents per whole lot. Cost of milli-lots = round_even(price * qty / 1000).
For every split, compute exact integer quotas, floor each, then assign residual units in
descending fractional-remainder order, ties by stable entity ID. Never use hash order.

Rounding carries for repeated interest/payroll accrue in sub-cent integer numerators;
post complete cents and persist residuals. Avoid an interest-free loan from weekly rounding.
A command batch runs at the next opening boundary, not at arbitrary UI wall-clock time.

## Opening-week order

0. Apply pending accepted commands in sequence; validate against prior closing revision.
1. Expire offers/contracts, deliver due shipments, complete funded construction, settle
   due governance and legal rulings. All can produce urgent pause items.
2. Reserve statutory obligations and scheduled debt service. Plan staffing and production.
3. Allocate regional labour, pay/expense payroll and employ workers for this week.
4. Dispatch utility generators from opening fuel stock, then allocate current-week
   electricity/water services to all industrial consumers simultaneously using the spot
   allocator and reserved budgets. Utilities are an explicit pre-production service
   stage; they do not require their own current-week electrical output as an input.
   Unserved consumers reduce planned output before consuming other inputs. Then produce
   from opening usable physical input stock plus allocated utility service, after staffing constraints. Newly produced
   output can sell this week. Goods purchased this week are never usable for production
   until a later week, including local supplies. This breaks same-week production cycles.
5. Clear reserved fixed contracts, then spot markets, then final demand. Match all buyers
   within each stage simultaneously. Do not run an entire firm before the next firm.
6. Settle local spot deliveries and create inter-region shipments. Pay freight services.
7. Accrue/settle expenses, depreciation, interest, tax and claims. Apply default rules.
8. Advance funded investment/R&D; update brand, credit, values, events and closing reports.
9. AI observes permitted closing data and queues next-week commands.

Fixed contract delivery failure first tries documented supplier stock, then records
shortfall and contractual penalty. Never silently buy an unlimited replacement off-map.
Contract precedence is a deliberate priority paid for by commitment, not free extra supply.

## Labour clearing

Each region has three independent worker pools. Named firms and background employers
submit desired workers by tier plus wage cents/week. Existing workers retained unless
firm lays off or a worker moves. Weekly 2 percent of staffed workers enter voluntary
job search; at least one only when deterministic accumulator reaches a whole worker.
Unemployed workers and searchers allocate across offers by wage above reservation wage.
Weight = max(0, wage - reservation + reservation/10) * open_positions. Offers below
80 percent of prevailing wage get zero new applicants. Use capped proportional allocation,
then largest remainder; no worker fills two jobs. Unplaced searchers retain their old
job if it still exists. Net hires cannot exceed actual applicants.

Prevailing wage moves 1/16 toward the employment-weighted mean offered wage weekly,
clamped to +/-2 percent per week outside declared events. Reservation wage is 70 percent
of prevailing. Skill training upgrades at most 0.1 percent of tier workforce weekly,
paid from explicit government or firm budgets; do not create workers by upgrading them.
Payroll is actual workers * agreed wage; contracts prevent retroactive wage cuts.

A facility's labour limit is min_tier(actual_staff_tier / recipe_staff_tier), capped at
100 percent. No substitution across skill tiers in v0.1. Production limit is the minimum
of labour, physical capacity and every required input. Unused staff still earns wages.
Background employment is a finite calibrated table with the same reservation constraints.

## Spot allocation algorithm

Books are per destination region, product and clearing stage. Seller offers include
origin, available stock, unit price, quality and known delivery schedule. A buyer has
requested quantity, reserved cash and maximum delivered price. Imports are finite offers.
Exclude unaffordable/unreachable offers and stock reserved by earlier contracts.

For physical intermediate goods, each buyer allocates its desired quantity across
eligible offers using delivered-price/quality weights. Household demand is one budgeted
buyer per region/product. Its brand preference also applies. Services can only clear
where their industry permits delivery; healthcare and retail are local by default.

Attractiveness BPS = clamp(round_even(quality_bps * brand_bps * reference_price /
(delivered_price * 10000)), 1000, 100000). Industrial buyers set brand_bps = 10000.
Quality is 5000..20000, brand factor 7500..15000. Reference price is positive pinned data.
No zero/negative prices; the minimum price is one cent/lot.

First construct buyer-to-seller bids: weighted desired quantities, capped by each buyer's
cash. Reserve the sum of per-fill costs rounded UP to cents, so final half-even postings
cannot exceed reservation. If full reserved cost exceeds cash, scale every quantity by cash/cost,
floor, then distribute at most one residual milli-lot per candidate in remainder order
only where affordable. This leaves tiny deterministic budget dust, never overspends.

At each seller, if total bids exceed stock, allocate available stock pro rata among its
buyers with largest remainders. Buyers receive less if others compete; do NOT re-spend
returned cash in the same clearing stage in v0.1. Release unused reservations afterward.
This one-pass rationing is deliberate: bounded work, no firm-order advantage, no solver
convergence risk. UI explains unfilled demand and alternative supplier recommendations.
No extra units are granted based on market-share targets. Match sales and purchase
postings atomically; quantities and delivered-price components reconcile exactly.

Example A: one seller has 100 lots, two equally priced buyers request 80 and 40. Fill
66.667 and 33.333 lots by remainder. Reverse firm table order: identical result.
Example B: two equal sellers, each price $10 and supply 100; household requests 100 with
$1,000. Each sells 50, revenue $500; neither obtains revenue for its unsold 50.
Example C: same sellers but only $600 budget. Allocate 30 each; unused demand remains 40.
Example D: no affordable offers yields no trade, no negative stock and all cash unspent.

## Final demand, gateways and inventories

Each region/product has baseline weekly lots and household expenditure weight from the
calibration pipeline. Household spend budget = 90 percent of disposable wage income plus
explicit pension/transfer income; remaining 10 percent becomes savings. Initial household
savings = 13 weeks of baseline income. At most 1 percent of savings can supplement a
week's budget after a wage shortfall. Savings cannot go negative. Government transfers
have explicit fiscal counterparty balance, not an unexplained firm revenue source.

Product budget uses fixed expenditure weights summing 10000. Requested quantity =
baseline_lots * clamp(reference_price / last_week_sales_price, 0.5, 2), with integer BPS
ratio. If no prior sale, use reference price. Existing budgets still limit purchases.
Unspent category budgets return to household savings, not another automatic purchase loop.
R&D/brand can redistribute seller demand; neither creates household cash.

A gateway quotes finite buy and sell offers at reference index +/-15 percent plus
freight. Each state's weekly gateway capacity per product = the larger of the corridor's
weekly input demand and weekly output for that product at seed, floor 100 lots, so a
shortage can be filled and a producing region's surplus has a buyer. Gateways record the
lots they import and export each week and keep a smoothed import average; that average is
the "room to build" shown on a state's board and used by rival expansion. No local
back-to-back transaction can profit from the bid/ask spread.

The outside economy settles through one finite account opened with $500 million: imports
pay into it, and exports and background public works are paid from it. Each week it may
draw at most one percent of its opening cash, net of that week's import receipts, so a
corridor running a sustained trade surplus sees exports taper toward balance rather than
stop at once. It buys only what it can pay for in full; a seller never hands over goods
for part of their price. Nothing is minted. Re-export
still pays freight and transit, and cannot use a shipment before arrival.

Goods use moving-average cost inventory, not free market-value write-ups. Each receipt
adds actual delivered cost. Issue cost = rounded proportional carrying cost with a
persisted residual; final issue takes all remaining carrying cost. Spoilage writes off
stock/carrying cost, never cash again. Financial statements label this accounting policy.

Freight distance comes from actual region centroids/validated lanes. Baseline road rate
$0.03 per lot per km, minimum $1 per inter-region shipment, lead time max(1, ceil(km/500))
weeks. Charge once; own logistics firm receipts equal the transport service postings.
If no named operator supplies capacity, finite background carrier offers apply. Retail
purchases final consumer goods; households buy retail delivery rather than both producer
and retailer for the same demand. Non-retail industries may sell direct only where the
product metadata allows it. No double counting of household demand.

## Regions and statewide services (1.2.0)

Retail, leases and electricity clear in each of the 31 economic regions. Goods sold between
firms clear on one corridor commodity exchange per product: every buyer's need in any region
meets every seller's stock in any region beyond that seller's own input cover, plus the import
offer of the buyer's region. Each pairing is priced as delivered, the seller's asking price plus
freight over the lane. Pairings fill cheapest delivered first; buyers tied on one source at one
price share it in proportion to what each still wants and can pay for; imports are the last
resort at equal price. The seller receives its asking price and the buyer pays the freight.

Prices and production follow sell-through. Each week an automatically priced good is cut 5
percent when stock exceeds four weeks of sales or nothing sold, and raised 2 percent when stock
is under one week, within half to twice reference and never above the cheapest import (services:
reference plus 15 percent). The production target moves 15 points a week toward what sold,
floor 30 percent; a service is judged by sold against capacity. Rivals always plan this way;
the player's companies do too until the player sets a price or target by hand, and either can be
handed back to automatic. Contract deliveries, power dispatched to the grid and construction
services all count as sales. A contract delivery is limited to what the buyer can pay for in
full, goods and freight together, in whole lots; the rest stays with the seller. Services that reach customers across a state
(software, telecom, advertising, research, maintenance, care, training, water and construction)
clear statewide: an offer in a region may draw on the seller's stock in any region of the same
state, and settlement removes it from where it sits. Background public works buy eighteen lots
of construction a week per state, split across the state's regions by demand. A region's
gateway carries one corridor volume per state split by the region's demand share, never below
what its own plants trade. Corporate tax losses carry forward against later quarters' profit.

## Denominator and zero-state rules

A zero workforce tier with positive recipe need yields zero output. A recipe with zero
need in a tier ignores that tier. Demand/reference prices are strictly positive except
financial-contract types excluded from the goods solver. Zero total attractiveness or
zero total requested quantity clears no trade. Zero prior sales uses pinned reference
price, never division by zero. Missing historical observation is a flagged estimate,
not zero. Monetary allocation sums settled components once, then splits the resulting
cents, avoiding independent rounded postings whose debits/credits disagree.
