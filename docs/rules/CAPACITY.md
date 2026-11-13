# Capacity (2.0)

Capacity is the contest. The player and every rival build, close and pay out under the same rules,
with numbers in content (`content/initial-balance.json`, block `capacity`, `cw_data::CapacityRules`).
Defaults reproduce the rules before 2.0, and the block joins the dataset hash only when it differs
from them.

## Build price
A plant ordered now costs its catalogue capex times the build price (`sectors::build_price_bps`):
the weighted mean of the general price index and, for each build input (steel, building materials,
construction), last week's closing corridor price over its catalogue price. Content weights: price
level 40 percent, each input 20 percent, clamped to 50 to 300 percent. The price is frozen on the
facility's `capex_cents` at order time (an existing saved field, so no schema change). Expansion,
retooling and the rivals' build sums use the same price.

## Room to build (`economy::capacity`)
One signal per strategy and state, read by the AI and exposed by `Session::capacity_openings`:
- Traded goods: the corridor's smoothed imports, valued at the best import netback.
- Services and retail goods: imports into the regions a plant there serves, plus the household
  demand that found no seller there last week, plus what plants buying the service there went
  without (a power plant: over its whole grid), valued at the last closing price. Capacity the
  sellers there left idle is taken off: what their plants could have run last week with the staff
  and inputs they had, less what households and plants bought. A plant short of staff or of an
  input cannot serve anyone with its idle blocks, so they are not counted.
- Plants on order, or opened in the last eight weeks, are taken off the room.
- Inputs: a plant can only sell what it can buy the inputs for. For each corridor good a strategy
  buys, the room a new plant could still get is what the corridor exported last week, what home
  makers left idle, and the unused import room (all volume steps, beyond smoothed imports) of the
  gateways in the plant's own region (a plant imports only through its own region), less what
  plants already buying it lacked after last week's trade and what plants coming will take
  (`input_rooms`). The opening's room is no more than the scarcest input feeds, and a block's
  contribution runs at that share of full output while paying its whole staff, upkeep and land
  (`input_fill_bps`, `new_block_contribution`).
- A block's weekly contribution at public prices and regional wages, today's build cost and the
  payback in weeks.

## Household demand is spending power
Households ask for each product from a fixed baseline, moved by the price against reference and
the regional demand ratio, and never for more than their weekly budget for it (90 percent of last
week's income times the product's spending weight) buys at the price they last paid. So unmet
demand is money households would pay at today's price that found no seller. A category's budget
that finds no seller, or that sellers could not fill, goes back to the households' savings.

Household income is the whole scaled regional economy, but only the named firms sell to
households, so each household product's opening baseline is sized to the playable market
(content `household_cover_bps`, `household_unmet_bps`). Each market (a region for shops, a
state for services) where named plants sell asks for what those plants make for households, less
what plants use of it, over the cover share (85 percent), never more than its budget buys. Markets
with no named seller ask for an even slice of their budget, sized so the corridor as a whole opens
with 30 percent of what households ask for finding no seller. Where every market has a named
seller, those markets carry the whole 30 percent. A product no named plant makes keeps its whole
budget. What households no longer ask for stays in savings, as any unspent budget does. Measured
at week 4 of every scenario, every household product opens 24 to 39 percent unmet.

With supply and demand in the same range, the service band tops out at 130 percent of reference
(`service_max_bps` 13000) and a seller with no import to compete with at 150 percent
(`local_ceiling_bps` 15000).

## Rivals build and close
- Growth: after a quarter of results, profitable, staffed to nine tenths in the target region, with
  cash over twice the reserve plus the build cost. A rival orders one plant the size of the gap:
  as many whole blocks as the room holds, one to eight, fewer when cash past twice its reserve
  does not cover them or the order would need a board vote (capex over a quarter of assets).
  It does not expand an existing plant, because an expansion takes the whole plant out of service
  while the new blocks are built. Room needed: half a block for keen builders,
  1.2 blocks for the cautious, between for the rest. No cap on plants: past four plants each plant
  shortens the payback horizon by 8 weeks (156 down to at least 52).
- Entry: a builder (growth weight 3 or more) with idle cash may open a business outside its own
  sectors where the room holds three blocks, valued on three quarters of its contribution.
- Closing: a firm that lost money over the quarter mothballs its worst plant that has run 26 weeks
  and loses money at last week's output and this week's prices and wages. Never its last plant,
  never the corridor's only maker of a good. A plant closed this way stays shut 26 weeks.
- Raiding is left to temperaments with raid or bid weight 3 or more.

## Idle cash
A rival keeps twice its reserve, 26 weeks of mandatory outflow, the price of one more plant like
its largest and a quarter of its debt. Half of anything beyond that is paid out each quarter as a
dividend (`PayDividend`, the player's command). Valuation counts cash in full, so a firm that pays
out is worth less on the league; a firm grows by investing.
