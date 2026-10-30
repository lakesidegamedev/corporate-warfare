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
shortfall and contractual penalty. A contract ships from where the seller makes the good: when
the seller holds none of it in the contract's origin state and runs no plant making it there
(the plant was sold, closed or carved out), the origin moves to the state of its largest
operating plant for the good, or failing that the state where it holds the most beyond its own
input cover, and the buyer pays freight from there. Never silently buy an unlimited replacement off-map.
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
shortage can be filled and a producing region's surplus has a buyer. Export room grows with
use the way infrastructure does: after a week whose exports filled a gateway's capacity, the
capacity rises by one percent (at least one lot) for every later week, player and rival sellers
alike. A week that leaves room unused adds nothing, and the outside account's paced cash still
limits what the room pays for. Gateways record the
lots they import and export each week and keep a smoothed import average; that average is
the "room to build" shown on a state's board and used by rival expansion. No local
back-to-back transaction can profit from the bid/ask spread.

The outside economy settles through one finite account: imports
pay into it, and exports and background public works are paid from it. It opens with the larger of
$500 million and 450 weeks (about nine years) of the scenario's opening trade surplus. The surplus
is what the opening plants make of each traded good beyond what the opening plants and the
households use, valued at the gateway bid (85 percent of reference), so a scenario built on
exporting heavy industry (Ruhr and Rhine, where almost half of what the plants make leaves through
a gateway) has an outside world that can pay for it, and Great Lakes keeps exactly $500 million. Each week it may
draw at most one percent of its opening cash, net of that week's import receipts, so a
corridor running a sustained trade surplus sees exports taper toward balance rather than
stop at once. It buys only what it can pay for in full; a seller never hands over goods
for part of their price. Nothing is minted. The account is a trader, not a vault: each week it
sells on a thirteenth of the stock on its books (what it bought from the corridor) to the rest of
the world, at cost, so cash plus stock never changes and no profit is booked. Without the resale
the goods sat on its books forever while its cash only fell, and every export-heavy scenario ran the
account dry within a few years (Texas $855 million to $360 million in four, Great Lakes $463 million
to $201 million in five); with it the account is flat to rising over thirty years in all four
scenarios and exports are never limited by its cash. Re-export
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
offer of the buyer's region. A firm never buys from itself, but its own stock beyond input cover
in another state feeds its own plants first: the goods move at their carrying cost and the firm
pays only the freight, so an integrated steelworks takes its furnace's pig iron instead of
exporting it at the gateway bid and importing it at the ask. Speculative buying into held stock
never moves own stock. Each pairing is priced as delivered, the seller's asking price plus
freight over the lane. Pairings fill cheapest delivered first; buyers tied on one source at one
price share it in proportion to what each still wants and can pay for; imports are the last
resort at equal price. The seller receives its asking price and the buyer pays the freight.

Prices and production follow sell-through, for every firm, every week (1.7.1, revised in 1.8
preview 3). A firm reads the weeks of cover it holds of a good (free stock over its average weekly
sales, over the weeks it has on the books up to 13) against the cover it wants, two weeks (one for
a perishable). Where plants buy the good, the stock that matters is the whole corridor's: buyers
keep weeks of input cover in their own yards and top it up every week, so a seller's shelf is
nearly empty after trade even when every buyer is full. So only buyers left short lift the price,
6 percent of reference for every week of their weekly use they lack after this week's trade
(each firm and state counted once against the sum of its plants' cover targets), and only unsold
stock beyond the seller's target cuts it, 6 percent for every week over. A seller that sells what
it makes to full buyers asks reference. Households count as buyers too: they keep no stock, so the
lots they asked for in a week that no seller had on the shelf (unmet household demand; demand
their budget could not pay for is not unmet) count as being short the seller's whole target cover
against their weekly asks. A good that no plant or household at home buys, that the corridor
exported last week, is never read as scarce: its shelf is empty because the surplus left at the
export bid. Any other good no plant or household buys reads the seller's own cover against its
target both ways: a week too little wants 6 percent more. The gap is held to five weeks either
way (30 percent). The price then moves a sixth of the way there, at most 2 percent in a week, within
half to twice reference and never above 95 percent of the cheapest import, so local goods beat
imports once normal freight is added. Where the corridor's gateways import at least a quarter more
than its plants made (smoothed imports at least 125 percent of last week's home output), the seller
wants that 95 percent import ceiling whatever its own shelf says: buyers pay the import price for
the rest anyway. Nothing sold with stock on the shelf wants 30 percent under; nothing sold and
nothing held gives no signal and the price eases a 26th of the way back to reference a week. There
is no dead band and no fixed step, so a price moves continuously with supply and demand and settles
instead of swinging. A service cannot be stored, so spare capacity is its unsold stock: it reads
the share of its installed capacity (operating plants at full run) that sold, asks reference at 85
percent, 0.6 percent of reference more or less for every point above or below, up to 15 percent
over reference (no import quote) and down to 20 percent under. A service's reference is its
catalogue price carried to today's price index (the index background wages follow), so its band,
floor and ceiling rise with its payroll; goods keep their catalogue reference and follow the price
level through import and export prices. Household demand and public works read services against
the same indexed reference. Unsold service output expires each
week. Stock a firm keeps as input cover for its own plants does not count as unsold. Rivals price
this way in the weekly tick, not through their slower strategic review.

Production is planned plant by plant: each plant's target moves 15 points a week toward what its
good sold, floor 30 percent, so one product's glut never idles a firm's other plants. A goods
plant with more than four weeks of unsold stock cuts, one under a week and a half raises; a
service plant moves toward what the firm sold of it plus a tenth, between 10 percent and full, so
it neither wastes a third of its output nor stays throttled below demand; a carrier cuts when its
capacity ran under 85 percent full by lot-km, unless its own lanes sent 40 percent or more of their
lots to outside carriers, and raises from 90 percent (see Carrier planning); a plant whose good has not sold yet
holds. A plant run down to its floor keeps at least one worker of every tier its recipe uses.
Rivals always plan this way; the player's companies do too until the player sets a price or target
by hand, and either can be handed back to automatic. A hand-set target holds goods plants at that
rate, but service and carrier plants, whose unsold output expires, keep following this plan under
it as a ceiling, unless the player locks the target (`SetProductionTarget` with `lock`, absent and
false in older commands). The CFO names any service or carrier division of a hand-planned company
that made at least one and a half times what it sold last week, with the weekly cost of the excess
and a button that hands production back to the plan. Contract deliveries, power dispatched to the
grid and construction services all count as sales.

The corridor price on the Markets board is the average asking price of the firms that make a good,
each weighted by the lots it sells (plus one), including firms in restructuring, or the cheapest
import quote when nobody makes it. A small seller joining, leaving or being restructured moves it
only a little. The board lists a good only when some strategy makes it and a plant in the world
makes or uses it, or it has traded or sits in stock: a good nothing uses is not shown at a
placeholder price. Storage space, a plant's own capacity, is never listed.

Every scenario opens with a producer for every good its plants use (a dataset test enforces it).
Maintenance, used by seven plant types, is made by the Plant Maintenance Services strategy
(construction industry) and research by IT Services; the roster of each scenario opens with
those and the other plants its chains lacked. A rival does not retool the only plant making a
good, since that would leave the good with no producer. A profitable rival no longer resets all
its plants to 85 percent every review; the weekly plan already raises a plant that sells out.

The player can instead give any good a pricing posture, after A House
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

A family index is relative to the general price level, not a nominal price: the engine inflates
every gateway price with the period's inflation on top of it, so 10,000 holds a good's real
price and 8,000 is a fall of a fifth against everything else (wages included). Authoring a path
therefore needs no allowance for the period's inflation. Farm goods carry the lesson: wages
follow inflation, and 77 to 86 percent of Silicon Valley and Texas farm output sells at the
gateway bid, so a path that sank to 55 to 75 percent of day one took farm margins from about 12
percent to nothing in five years. The grain and fertilizer paths of those two scenarios now fall
no lower than 88 percent, and a test holds the real grain bid within 15 percent of day one over
five years in both.

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
A refinery or gas plant (industry `chemical_industries`) can shift its cut each week: each output
scores its owner's ask against reference (the region's export bid, if lower, for a good no plant
or household at home buys and the corridor exports), less 4 basis points per hundredth of a week
of unsold stock beyond its glut cover (at most 20 percent). An output scoring under the
yield-weighted average gives up 2 basis points of its recipe yield per point under, at most a
quarter; the lots given go to the outputs scoring above the average in proportion to their lots
times how far above, each at most half again its recipe yield (all shifts scale down together
to keep that bound). Total lots never exceed the recipe. Fixed-proportion by-products (scrap,
livestock, natural gas from wells, building materials from furnaces) do not shift.
Output, price, valuation and planning all read the whole list: pricing and the weekly plan treat
every by-product as a priced good of the firm, the exchange sells it from stock like any other
good, gateways carry it (a good with no local buyer still has the outside buyer at reference
less 15 percent, and a region's gateway always has export room for at least what its operating
plants make of the good as a by-product, so a co-product of a plant built after the gateway was
sized never piles up unsold), and AI build and retool value a plant by all its products, counting a plant as
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

**Who is losing money.** The snapshot's corridor line splits operating companies into three
kinds and counts how many of each lost money over their last thirteen weeks: established (an
operating plant and founded more than 52 weeks ago), entrants (an operating plant, younger) and
shells (no operating plant: still building, mothballed or with every plant sold or raided away).
The all-company losing share mixes young plants still ramping and idle shells with the trading
companies; the established share is the one that should stay flat over a career.

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
also move with road condition. Every rate (owners', own operating cost, outside, lane contracts)
is the base rate carried to the price index at the start of the week, so freight rises with the
wages and costs carriers pay.

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

**Reading freight (1.7.1).** For each firm and destination region the engine keeps the week's freight
bill (split into own carriers and contracts, other firms' carriers and outside carriers), the loads,
the lanes used and the goods shipped. It is a read-out of the week just closed and is not saved with
the world. A division's Shipping summary shows the bill, the premium paid to outside carriers (their
price less what the same loads cost on own carriers), the three costliest lanes and the policy in
words. It prices each policy and preferred mode by routing last week's shipments again at today's
lane prices without booking anything. The suggestion is either a build (carrier plants for the
outside loads on the costliest outside lane, payback is their cost over the weekly premium saved,
shown only within five years) or a reservation of the loads the group moved on a busy lane that
nets a saving at last week's volume. A reservation quote is the fee `reserve_lane` would charge.

**Alerts** are sent only for lanes the player ships on: a lane full for four weeks, a carrier rate
hike of a fifth over four weeks, a rival reserving the lane.

**Carrier planning.** A carrier reads last week's fill by lot-km (lot-km moved over capacity times
the 400 km reference haul) and the lanes it had capacity on. Under 85 percent full it cuts its run
rate 15 points, from 90 percent it raises 15 points, and between them it holds. When its lanes sent
40 percent or more of their lots (own, contract and outside) to outside carriers it does not cut,
since that freight is wanted, but it still raises from 90 percent. Overflow alone does not raise:
in a 16-seed, five-year trial that spread capacity onto lanes the overflow was not on, carrier fill
fell from 69 to 49 percent and the outside share rose. With this rule fill was 74 percent against
69 and the outside share 47 percent against 49 (final year, 16 seeds).

**Rivals** set policies (own carriers first when they own any), build a carrier on the lane with
the most outside overflow when they earn and have cash (a terminal first where the lane has none),
and aggressive rivals reserve part of a busy lane they own a fifth of and ship on. The overflow a
rival builds for is net of carrier plants of that mode, at either end of the lane, that are being
built or opened in the last thirteen weeks, because every rival reads the same hot lane in the same
quarter. A carrier plant (not a terminal) that has run half a year, moved under a quarter of its
capacity in lot-km in the week just closed, and belongs to a firm that lost money over its last
thirteen weeks is mothballed, so the firm stops paying a crew and upkeep for freight that is not
there; it is reopened when a lane at its end runs with at least half its capacity overflowing to
outside carriers and the restart leaves the cash reserve intact. The player has the same
Mothball and Reopen commands.

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
the reserve sells instead when the price is over 120% of the average, or over 110% of what the reserve paid for the good, or above the average while a supply shock is on that good (a quota biting, a harvest failure, a shut plant that makes it). It offers up to 150 lots a week per region from its stock, ten percent under the going price, to buyers in any region (the buyer pays freight). Its release is an emergency sale and takes orders before other sellers. Its purchases and sales are real cash on the government ledger. Rival speculators sell their own store when the reserve sells,
and buy beside it when the reserve buys.

Systemic events, state rescue and vertical review: see EVENTS.md and CORPORATE.md.

## Elections, state laws and public tenders (1.9)

All of this lives in `economy::civics` (save trailer `POLI`, see INTERFACE.md) and runs the same
in every scenario; scenarios only supply the states, the regions and the cast.

**Races.** Every state has a governor race and the country has a leader race, each between two
people, one for each party, drawn from the cast (free governor or leader capable people) or, when
none is free, a generated face. A candidate states two or three positions: corporate tax (a state
adjustment of 100 to 300 points, or the national programme's rate), right to work, environmental
rules, and for the leader race common carrier and merger review. State terms are four years; state
`i` votes in week `45 + 52 * (i mod 4)` and every 208 weeks after, the leader race in the existing
election week. Gifts and the campaign are open for the last 26 weeks.

**Poll.** The business candidate's share, 0 to 10,000. State: the sitting governor's target is
`5000 + mood + gift swing`, clamped 1,500 to 8,500, and the poll closes an eighth of the gap each
week. Mood is `(600 - unemployment)/2 + wage change/2 - price change/2` in basis points, clamped
to plus or minus 1,500, where wage and price change are measured against references that follow
the present over 52 weeks. National: the poll is the governing party's hold chance,
`approval + 300 + gift swing`, the same chance the election always used, so with no gifts the
national result is unchanged. A candidate's gift bonus is `600 * F / (F + $200,000)` basis points
for total funds `F`, so it never reaches 6 points.

**Results.** State: the business candidate wins with chance equal to the poll (clamped 5 to 95
percent), drawn once from a stream of the week and state. The winner becomes governor and their
positions become the state's laws 13 weeks later (the same delay as national laws), keeping any
law they did not speak to. The leader race result still sets the national programme through
`politics::programme` after 13 weeks.

**State laws.** Right to work weakens union law by one step for firms headquartered in the state
(union drift in `unions.rs`). The corporate tax adjustment is added to the national rate in both
the quarterly accrual and the dividend/tax estimate. Environmental rules change the build cost
and lead time of heavy industry (extraction, energy, chemicals, manufacturing): lax 95 percent
cost and 90 percent time, standard 100, strict 108 percent cost and 115 percent time. The gift cap
(default $250,000 per firm per race; $1,000,000 nationally) is a state law too.

**Campaign gifts.** `GiveToCandidate`: cash leaves the firm as an expense and lands in the outside
advocacy sector (like a lobbying fee), so no money is made or lost. Refused outside the campaign
window, below $1,000, over the cap, or without cash beyond reserves. The next week the paper
discloses it and the other side holds it against the giver (reputation down by a quarter of the
gift's weight, weight being 1 bp per $2,000 up to 100). After the vote donors to the winner gain
half the weight and donors to the loser lose a quarter. Reputation feeds lobbying odds
(`policy::success_bps`). Rivals give once a month by temperament (cautious never, financier 20,
builder and integrator 40, speculator 60, raider 90 percent of the cap) to the side whose positions
help them: heavy plants, carrier ownership (national race) and large payrolls lean business.

**Tenders.** Every 13 weeks, if the treasury's weekly procurement budget is positive and open and
unfinished tenders commit less than 40 weeks of it, the state opens a tender for a road upgrade
(6 weeks of the budget, 8 to 26 weeks), power line (8 weeks, 10 to 30) or port expansion (10
weeks, 13 to 39) in the region of a state taken in turn with the least of that kind of works.
Bids close after 4 weeks. A bid is a price (at most the budget) and a delivery time; the lowest
`price + weeks * budget / 100` wins, skipping bidders that are not operating, foreign currency, hold
three works already, or lack a quarter of the cost in cash. The winner's cost is 70 percent of the
budget scaled by the region's land index, plus up to 12 percent for hurrying. Each week it pays
cost/promised weeks as construction bought from contractors in the region (the existing construction
service path), adds steel demand in the region, and the treasury pays the price in quarters as the
cost is spent, never more than half the week's procurement budget or the treasury's cash. A finish
after the promised time forfeits a tenth of the price per week late, at most half. A failed
contractor lapses the tender. Finished works add 10 percent lane capacity on trucks to and from the
region (cap 40), 15 percent on sea lanes and sea terminal access (cap 45), or 5 points of power grid
score (cap 15, never above 100 percent). Defence procurement and the strategic reserve are
untouched and draw on the same treasury.
