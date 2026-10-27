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

A gateway quotes finite buy and sell offers at the outside world's price +/-15 percent plus
freight. The outside price is the reference index scaled by the product's world price level
(see World commodity prices below) and by inflation. Each state's weekly gateway capacity per product = the larger of the corridor's
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
percent when stock exceeds four weeks of sales (two weeks for a perishable) or nothing sold, and
raised 2 percent when stock is under one week and the corridor is still importing the good, within
half to twice reference and never above 95 percent of the cheapest import, so local goods beat
imports once normal freight is added (services: reference plus 15 percent). With no imports a
price above reference eases back 1 percent a week instead of staying at the ceiling. Stock a firm keeps as input cover for its own plants does not count
as unsold. Production is planned plant by plant: each plant's target moves 15 points a week
toward what its good sold, floor 30 percent, so one product's glut never idles a firm's other
plants; a service is judged by sold against capacity, and a plant whose good has not sold yet
holds. Rivals always plan this way;
the player's companies do too until the player sets a price or target by hand, and either can be
handed back to automatic. Contract deliveries, power dispatched to the grid and construction
services all count as sales. The player can instead give any good a pricing posture, after A House
Divided: a step from 20 percent under to 20 percent over the market price (the median ask of
the other firms that make it, or reference when none do), or a markup of 10 to 100 percent over
the firm's own cost per lot (inputs at reference, its wage offers for this season's crew,
maintenance, land and depreciation, over full output). A posture prices the good at once and
every week after, within half and twice reference. A contract delivery is limited to what the buyer can pay for in
full, goods and freight together, in whole lots; the rest stays with the seller.

Services that reach customers across a state
(software, telecom, advertising, research, maintenance, care, training, water and construction)
clear statewide: an offer in a region may draw on the seller's stock in any region of the same
state, and settlement removes it from where it sits. Background public works buy eighteen lots
of construction a week per state, split across the state's regions by demand. A region's
gateway carries one corridor volume per state split by the region's demand share, never below
what its own plants trade. Corporate tax losses carry forward against later quarters' profit.

## World commodity prices (1.7.0)

Every scenario feeds the outside world's prices into gateway import and export prices. Products
belong to price families (oil, natural gas, coal, ore, steel, grain, fertilizer, chemicals,
timber, rare earths, building materials); the products table in `content/price-paths.json`
maps a product id to its family, and ids missing from the current content are ignored, so a
new product joins a family with one line. Products with no family keep their opening gateway
prices apart from inflation.

Each family has an index, 10,000 basis points on day one, recomputed every week before the
business cycle runs. Both of a gateway's prices are scaled by the ratio of this week's index to
last week's, the same way inflation scales them, so the +/-15 percent spread and the finite
capacity are unchanged. An index never leaves 20 to 800 percent of its day-one level.

The new-game screen offers three modes; the scenario sets the default (Great Lakes 2000 is
historical, Texas 1981 is historical with variance). A custom scenario carries its own choice.

- Historical: the scenario's anchor points, `[quarter, index]` with 100 meaning day one, linearly
  interpolated by week (13 weeks to a quarter) and held flat after the last anchor. The same
  every game.
- Historical with variance: the same anchors with the whole path shifted in time by a shared
  offset of up to 20 weeks plus up to 10 weeks per family, each swing's depth scaled to 60 to
  140 percent of history, and an autocorrelated weekly noise of about one percent, all drawn
  from the game's seed on a stream of its own (per family, so adding a family does not change
  another's draws). The path is rescaled so day one is exactly 10,000.
- Random: a neutral mean-reverting walk, `x += (10,000 - x) / 32 + shock`, shock uniform to
  plus or minus 2.5 to 4.5 percent per family, held between 50 and 200 percent. No drift and no
  scripted busts; busts come from the business cycle.

A family with no anchors in a historical game, or a scenario missing from the file, runs random.

Interaction with the business cycle, without counting a shock twice:

1. In the two historical modes a weekly change in the weighted world index (family `weight`s
   in the file) adds 8 percent of itself to the cycle's momentum, limited to -15..+10 a week.
   A commodity collapse is a world demand shock. Random mode feeds nothing.
2. A shadow copy of the cycle's momentum and gap dynamics tracks how much output gap those
   pushes explain.
3. The cycle tilts every family by 30 percent of the output gap that is neither explained by
   the shadow nor present on day one, times the family's `cyclical_pct`. A 5 percent
   unexplained downturn lowers oil about 1.5 percent and grain about 0.4 percent.

State (anchors, drawn parameters, noise, shadow gap and the trailing 13 weeks of index) is saved
with the world. The economy card shows each family's level against day one and its change over
the last quarter.

## Denominator and zero-state rules

A zero workforce tier with positive recipe need yields zero output. A recipe with zero
need in a tier ignores that tier. Demand/reference prices are strictly positive except
financial-contract types excluded from the goods solver. Zero total attractiveness or
zero total requested quantity clears no trade. Zero prior sales uses pinned reference
price, never division by zero. Missing historical observation is a flagged estimate,
not zero. Monetary allocation sums settled components once, then splits the resulting
cents, avoiding independent rounded postings whose debits/credits disagree.

## Product chains and multi-output plants (1.7)

A recipe has one primary output and may list `by_products`, each a fixed number of lots per
block per week at full capacity. Every product of a plant scales together with the plant's run
rate, the farm season and supply events, so a refinery never makes gasoline without its diesel.
Output, price, valuation and planning all read the whole list: pricing and the weekly plan treat
every by-product as a priced good of the firm, the exchange sells it from stock like any other
good, gateways carry it (a good with no local buyer still has the outside buyer at reference
less 15 percent), and AI build and retool value a plant by all its products, counting a plant as
having room when imports cover its primary output (a by-product nobody imports is still sold, so it
never hides the room: a refinery is built when the corridor imports fuel). Nothing a
plant makes is left without a price or a buyer.

The converted inputs' carrying cost is shared among the products by their value at reference
prices (joint-cost rule); the primary output takes the rounding remainder, so lot carrying cost
and the Inventory account stay equal. `unit_cost` for a cost-plus posture uses the same value
share. A plant whose deposit runs out stops its by-products with the primary.

Chains added in preview 2 (yields and prices are authored game constants, circa 1980 to 2000
proportions, set by `tools/content/add_chains.py`):

- Oil: wells make crude oil (and a little associated gas); a refinery turns crude into fuel
  (the road and industrial fuel product, here diesel and heating fuel, so every existing fuel
  consumer is unchanged), gasoline, jet fuel and petrochemical feedstock. Plastics and chemicals
  use feedstock; gas processing strips natural gas liquids from natural gas, and plastics use
  liquids too. Jet fuel feeds aerospace, gasoline feeds delivery fleets; the rest is exported.
- Iron: a blast furnace makes pig iron from ore and coal with slag sold as building materials;
  a basic oxygen mill makes steel from pig iron and scrap; an electric arc furnace melts scrap
  alone. Scrap is a by-product of steel works, component stamping and vehicle assembly. The old
  ore-to-steel strategy remains.
- Food: mixed farms grow grain and raise livestock together; food processing turns both into
  food. Perishable goods (food, groceries, grain, livestock) spoil a percentage of unsold stock
  each week, written off at carrying cost, and plants buy at most two weeks of a perishable
  input.

Services are paid to the firms that hold them. A freight bill, an advertising purchase or a
service input (software, telecom, research, maintenance, freight) pays the corridor's own
producers first, in proportion to the service stock each holds this week, relieves that stock
and records the sale; only the remainder goes to the outside sector. Service makers run before
the plants that use their services within a week, so a service input finds its producer's stock.

Seed firms may open with extra plants (`extra_plants` in `content/firms.json`) so that gas,
timber, pharmaceuticals, rare earths, building materials and the new chain goods have a local
producer at the start. A firm runs one strategy per sector in a region, so a firm's plants of
one sector sit in different regions; the dataset check refuses a roster that breaks this. Seed
supply contracts deliver from the region of the plant that makes the good, not the firm's
headquarters.

## Storage, stock targets and speculation (1.7)

Stock is still kept per firm, product and region. Space is a second, separate limit with four
classes: dry (warehouse), tank (liquids and gas), silo (grain and bulk) and cold (food, livestock,
pharmaceuticals). Each good declares its class, a weekly holding cost (basis points of the
reference price per lot) and the existing weekly spoilage rate. Services and contracts take no
space.

Capacity of a firm in a region and class is the sum of two parts, both in lots:

- A working yard: every operating plant brings 6 weeks of each input and output of that class,
  times its blocks. A plant that keeps normal cover never fills its yard.
- Owned storage: an operating storage facility adds `weekly_output_lots` times its blocks of its
  class (the recipe's `storage_class` marks it; it is a logistics recipe that makes the service
  `storage_space` and no goods, with the usual capex, crew, power and build time).

At each week's close, before spoilage, for every firm and cell:

1. Stock above capacity sits in outside storage and pays a fee per lot, by class: dry 80, tank
   120, silo 60, cold 200 cents a week. The fee goes to other firms with spare owned space in the
   same cell, pro rata to that space, as revenue; the rest goes to the outside sector. Goods are
   never refused or destroyed for lack of room.
2. Stock above the yard (inside owned storage or outside) pays a holding cost: the share of each
   lot's reference value given by the good's `holding_bps_weekly`. It goes to the outside sector.
3. A firm pays what its cash covers; it never goes negative from storage.
4. Spoilage (see Product chains) uses the good's weekly rate, cut by 80 percent (a cold store removes most of the loss) for the share of a class's
   stock that fits in owned storage in that region. Lost stock is written off at carrying cost as
   an operating expense, and its milli-lots are recorded on the firm's storage book.

Storage moves money and never creates or destroys stock; the only stock destroyed is the recorded
spoilage.

Stock targets. Each plant carries a stock target (0 to 26 weeks of inputs; 0 means the firm's
default cover, 4 weeks, and 2 for perishables) and a seasonal build, on by default. In the harvest
quarter (quarter 3) a plant that uses a seasonal farm good with spoilage under 2 percent a week
(grain) keeps 8 more weeks of it, buying ahead of the lean quarters. In that same quarter a grower
tolerates 6 more weeks of unsold stock of such a good before cutting price and output, so the
harvest is stored rather than dumped. An explicit target replaces the perishable limit.

Holding goods. A firm may hold goods back from sale and export. Held goods are reserved like
input cover: not offered on the exchange, not exported, not counted as unsold stock for pricing.
They never exceed stock on hand (production or contracts using them reduces the hold).

Buy to store. A buy order names a good, a region, up to 5,000 lots and a price limit of 50 to 200
percent of reference. Each week, after plants have bought their inputs from the exchange, an open
order buys up to 400 lots from the cheapest delivered sellers (firms or the region's gateway) whose
delivered price is within the limit, paying freight like any buyer. What arrives is added to held
stock. Orders last 13 weeks, a firm may keep 6, and a second order for the same cell tops up the
first. Sell from store releases held goods (all, or a number of lots) for the normal channels at
the firm's asking price and cancels the cell's order when releasing all.

Rival rule. Every strategic review a rival may, for a good its plants make or use that is
storable, not perishable and in a world price family: buy to store when the family index is at
least 10 percent under day one, no more than 2 percent down on the quarter, and the cycle is in
recession or recovery, using at most 6 percent of cash (up to 300 lots) and only into space it
already has, with cash above twice its reserve and profit behind it; release when the index is 5
percent over day one, or falling over 5 percent on the quarter while above day one, or the cycle
is in a boom. Rivals never build storage; temperament hooks come later.

Alerts and telemetry. The player is alerted only when stock is 95 percent or more of capacity in
a cell holding at least 500 lots, when four weeks of outside fees and holding cost reach 3 percent
of four weeks of revenue (and $2,000), or when four weeks of spoilage reaches 2 percent of revenue
(and $2,000). Market snapshots (schema 2) carry corridor and player storage lines: use, capacity,
owned capacity, outside lots, fees, holding cost, spoilage and held goods.

## Owned logistics, lanes and routing (1.7)

The generic `freight` product is lane capacity. Data lives in `content/logistics.json` and the
carrier recipes; the rules are the same in every scenario and scenarios only supply data
(starting terminals, ports, sea systems, gateway links).

**Modes.** Truck, rail, pipeline and sea are data: a price per lot-kilometre, a speed, a smallest
shipment, whether both ends need a terminal or port, whether the lane needs built infrastructure,
whether outside carriers backstop it, and which storage classes it may carry. Pipelines carry
tank goods (liquids and gas) only. Cold goods go by truck or rail only. Ship carries dry, tank
and silo goods. Services are never shipped. Rail needs a rail terminal at both ends of a leg, sea
needs a port at both ends on one sea system, and a pipeline lane exists only where a firm has
built capacity on it. Public terminals and ports come from the scenario; any firm can build more
(Rail Terminal and Port recipes), and a built one opens its region to every firm.

**Carriers and lanes.** A carrier is a logistics division whose recipe carries a mode (Trucking,
Full-Service, Freight Rail, Pipeline, Lake and Sea Shipping, Rail Terminal, Port). The lots it
made last week, counted over a 400 km reference haul, are its capacity this week, so a short lane
holds more lots than a long one. The capacity is spread over the lanes it serves: lanes with an
end within the mode's reach of its region, weighted by how much freight those lanes asked for in
that mode, or lanes the owner pinned. A lane is two regions in one mode. Capacity, use and
contracts are saved with the world.

**Price.** A lane's owners charge from the empty-lane rate (100 percent of the base rate) up to
the outside price (125 percent) as the lane fills, along a smooth square curve, so a full lane
meets the outside price with no cliff. A group ships on its own carriers at operating cost
(65 percent) and first. Lots beyond capacity go to outside carriers at 125 percent, so goods
never stop; pipelines have none, so a shipment either fits or takes another mode. Truck prices
also move with road condition.

**Charging.** A shipment is charged once. The shipper's cash pays exactly what carriers and the
outside sector receive; a shipper short of cash pays what it has and every receipt shrinks
together. Carriers are paid in cash and relieved of the freight stock they made for the purpose.

**Routing.** Each shipment is routed over one to three legs through hubs. A division's standing
policy picks: cheapest, fastest, own carriers first, or most reliable (fewest outside lots, then
emptiest lanes), with an optional preferred mode or carrier that ranks routes meeting it ahead.
The default is cheapest. The policy governs the freight a division pays for, goods arriving in
its region. A change of mode at a hub adds a week.

**Lane contracts.** A firm reserves lots on a lane for 4 to 52 weeks at the lane's spot rate less a
10 percent commitment discount, fixed for the term, up to 80 percent of the lane and never against
its own group's carriers. The fee is paid to the lane's carriers every week at the opening whether
or not the lots are used; a holder that cannot pay loses the contract. Reserved lots are not for
sale to anyone else, so a busy lane pushes rivals onto dearer routes.

**Alerts** are sent only for lanes the player ships on: a lane full for four weeks, a carrier rate
hike of a fifth over four weeks, a rival reserving the lane.

**Carrier planning.** A carrier follows how full its lanes ran: under 40 percent of capacity cuts
its run rate, over 85 percent raises it.

**Rivals** set policies (own carriers first when they own any), build a carrier on the lane with
the most outside overflow when they earn and have cash (a terminal first where the lane has none),
and aggressive rivals reserve part of a busy lane they own a fifth of and ship on.

**Sprawl** no longer has a threshold. Overhead is 0.18 percent of revenue times the square of the
division count over 100 (10 divisions 0.18 percent, 20 0.72, 30 1.6), doubled with a secondary
type. Logistics strength takes up to half off and each owned logistics division takes six percent
off, up to forty.

## Government laws on markets and logistics (1.7)

Four new laws sit beside tax, wage floor, merger review and unions. The party in power sets
them in its programme. A new government announces its programme at the election and it takes
effect 13 weeks later; a government returned to office re-announces its market programme if
it differs from what is in force. The starting government's programme is in force from week
one. Laws that are per state (quotas) are a bit per state in code order.

| Law | Commerce Party | Workers' Party |
|---|---|---|
| Common carrier | off | on |
| Production quotas | on in states that specialise in extraction, 80% of capacity | off |
| Vertical merger review | off | from 35% share |
| Strategic reserve | off | on |
| Rescue of failing strategic firms | bailout, then privatise | nationalise |

Common carrier. A lane's price already rises with fill from the empty-lane rate to the outside
premium. Under the law the price owners may charge other shippers is capped at the empty-lane
rate plus 40% of the gap to the premium (110% of the base rate with the shipped content, since the empty lane is 100% and the premium 125%), the
same cap applies to contract rates, and lane contracts together may hold at most 40% of a
lane's capacity instead of 80%. Rivals do not try to lock lanes up. The rate cap lowers the
return on a new carrier, which rival build tests read.

Production quotas (prorationing). While an extraction good's corridor price is under 92% of
its 52-week average (after 26 weeks of history), plants making it in a quota state may run
at only the quota share of capacity. The cap is a regional supply modifier recomputed every
week and lifts the week the price recovers or the law is repealed. Rivals do not build new
extraction capacity in a capped region. Applies to any extraction good in any scenario.

Strategic reserve. For extraction goods that do not spoil, the government buys when the
corridor price is under 85% of the 52-week average: up to 300 lots a week per good, at the
corridor price, from firms' free stock (stock beyond their own plants' input cover), paying
from government cash (at most 1/40 of it a week across goods), up to 4,000 lots per good. When
the price is over 120% of the average it offers up to 150 lots a week per region from its
stock in that region, priced 3% under the going price, through the same exchange as everyone
else, so it competes with speculators and with importers. Its purchases and sales are real
cash on the government ledger. Rival speculators sell their own store when the reserve sells,
and buy beside it when the reserve buys.

Systemic events, state rescue and vertical review: see EVENTS.md and CORPORATE.md.
