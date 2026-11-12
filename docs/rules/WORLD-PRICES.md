# World prices and their weather (2.0)

World prices are the outside world's price for oil, gas, coal, metals, grain and the rest, in
price families. The paths, the three modes and the cycle coupling are in MARKETS.md (World
commodity prices). This page is the 2.0 addition: texture on top of a path, and markets that
follow the world. Engine: `economy/world_prices.rs`. Numbers: `content/initial-balance.json`
(block `texture`) and `content/price-paths.json` (the authored paths).

## Texture
In a historical game with variance, each family listed under `texture.families` drifts around its
authored path week to week. Per family: `swing_bps`, one standard deviation of the stray in basis
points of the path (oil 1,300, natural gas 1,600, and so on); `half_life_weeks`, how long a
disturbance takes to fade by half; and the named causes for a push up or down.
- A week's shock is drawn from the game's seed, the family and the week alone. A disturbance sums
  the past weekly shocks, each smoothed twice by the family's fade, so a shock builds over weeks
  and fades over months: the world drifts and turns, it does not jitter. Nothing is saved; the
  same seed plays the same weather.
- The seed also includes the scenario's start year, so one seed in two scenarios does not play the
  same weather.
- Shocks from before the game began count as nothing, so day one sits exactly on the path.
- A disturbance past `texture.named_bps` (1,500) is named. The price reason on the Goods screen
  prints a cause from the family's list ("A cold winter lifts natural gas").
- Dated history still shows through: the scripted timeline (EVENTS.md) moves the path itself, so
  the 1986 oil collapse in Texas is a collapse in every game.
- Variance is the default mode for every scenario and for custom games. Historical mode keeps the
  smooth path; random mode is unchanged. A family with no texture entry keeps its smooth path.

## Home prices follow the world
A traded good is priced around the outside world's middle quote, halfway between the gateway's
import and export prices, so home prices follow world prices and inflation the way the gateways
do (MARKETS.md, Regions and statewide services). Ports hold finite room: when the corridor leans
hard on imports the last shipments cost more and home prices rise toward them; when exports
flood the ports the last shipments sell cheap. Every firm exporting through one port in a week
is paid the same average price. Room grows and shrinks slowly with use (`room_growth_bps`,
`room_shrink_bps`).

## Demand follows the economy
`demand.cycle_elasticity` (content) sets how far each product's orders swing with the business
cycle: steel, building materials, vehicles, machines and advertising swing hard, food, care and
utilities barely move, and goods bought to invest also follow how easy credit is. The market
screen says why ("recession: orders for steel are down 14 percent").
