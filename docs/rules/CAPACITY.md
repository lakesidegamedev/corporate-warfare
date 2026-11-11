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
  demand that found no seller there last week, valued at the last closing price.
- Plants on order, or opened in the last eight weeks, are taken off the room.
- A block's weekly contribution at public prices and regional wages, today's build cost and the
  payback in weeks.

## Rivals build and close
- Growth: after a quarter of results, profitable, staffed to nine tenths in the target region, with
  cash over twice the reserve plus the build cost. Room needed: half a block for keen builders,
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
