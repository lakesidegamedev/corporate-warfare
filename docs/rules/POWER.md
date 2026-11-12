# Power on a grid (2.0)

Electricity is not carried between regions the way goods are. It is pooled over a grid. Engine:
`economy/production.rs` (`dispatch_electricity`, `grid_key`) and `economy/markets.rs`
(`grid_regions`). It runs in step 4 of the weekly order (MARKETS.md), before production.

## The grid
A grid is a currency area: every region that keeps its books in the same money (a region takes the currency of its country; `content/scenarios/<id>/currencies.json`, CURRENCIES.md). A scenario with one currency is one grid. Every generator on the grid feeds
every plant on it, whatever county either stands in. A plant in a region with no generator is
supplied like any other.

## Dispatch each week
1. Generators (operating plants whose recipe makes electricity) put their output into the pool.
   Their fuel comes from their opening stock; a generator does not need its own current-week
   power.
2. Every plant that needs electricity asks for what it planned to draw. A buyer can only reserve
   what it holds in free cash.
3. Gateway imports of power add to the pool, finite per week, at the gateway's import price.
4. When the pool is short, every plant that asked gets the same share of its plan, whoever owns it
   and however new it is. A plant short of power cuts its planned output before it consumes other
   inputs.
5. The grid's going price is the generators' asking prices weighted by what each made, with
   imported power at the import price, so a short grid pays more as its generators' asks rise.
6. Every buyer pays for what it was granted. Generators are paid the whole takings in proportion
   to what each delivered. The part the gateway supplied is paid abroad. No money is created or
   lost.

## What it means in play
- A power station is paid for every unit it delivers, so a station on a grid with spare power
  earns less and one on a short grid earns more.
- A short grid pushes the power price up, which raises costs for every plant on it and makes the
  Openings board show room for a generator (CAPACITY.md counts what plants buying the service went
  without, over the whole grid, as room).
- Services that businesses buy (power, maintenance, research, software) get dearer when their
  business customers go short, not only when households do.

## Content
No number here is authored: the grid follows from each scenario's countries and currencies
(`currencies.json`), the generators' asking prices and the gateways' import prices and room
(`content/scenarios/<id>/logistics.json`).
