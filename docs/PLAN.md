# Corporate Warfare — plan

Status: draft for review · 2026-10-25 · owner: unassigned
Precedes repo creation. Target: iOS first slice, then macOS/desktop.
Supersedes: `corpgame` / `corporate-warfare-sandbox` (old Next.js + Mongo web game, being scrapped).

## Decisions locked

| decision | choice |
|---|---|
| Simulation engine | Rust core + SwiftUI shell (UniFFI) |
| Spatial model | **BEA economic areas, ~180 nationally**; no county layer, no sub-region sites |
| Facility location | the **region** is the build location |
| First slice | **cross-state corridor**, ~13 regions, all 15 industries, 5 years |
| Time | weekly ticks, instant local compute, pause/play |
| Opponent | AI rivals on identical rules + a light event/narrative layer |
| Data | real US public data, baked into the app at build time |

---

## 0. The pitch

A single-player corporate empire sim for the United States, modelled in ~180 real
economic regions, where you run one company and the rivals run by the same rules. Time
advances in weeks, and a week resolves instantly — locally, on the device, with no server
and no waiting.

> **One sentence:** *A House Divided's corporation system, rebuilt as a native,
> region-level, instant-turn company sim where the fight is over markets, labour, and
> control rather than seats.*

The whole reason this project exists is the turn. AHD's median turn is **112.6 s**
(p95 191.5 s), and `corporationTurn` alone is **23.2 s median** across 6,162 sectors and
~2,420 Mongo round trips. This game's entire architecture is an answer to that number.
If a week takes longer than a blink, the design has failed.

---

## 1. Design pillars

1. **Instant weeks.** A tick is a pure local function over compact state. Budget:
   p50 ≤ 4 ms, p99 ≤ 12 ms on an A15-class phone. No network in the loop, ever.
2. **Region is the unit of place.** ~180 real economic areas, each a labour market, a
   consumer market and a logistics node. A region is a place, not a dot on a map.
3. **The company is the game.** Deep P&L, capex, working capital, debt, brand, R&D,
   headcount. Corporate warfare means price, talent, supply, and control — not armies.
4. **Rivals obey the same rules.** No cheating AI economies. If a rival outgrows you,
   it is because it made better decisions inside the same model.
5. **Every screen answers "what do I do next?"** Depth in the model, not in the menus.

---

## 2. What we take from AHD, and what we leave

The corporation stack in `AHDGame` is ~151 non-test modules under
`src/lib/corporations/` plus ~60 turn phases under `src/lib/turn/corporation/`.
That is a decade of balance decisions and edge cases. We are **re-deriving** it in Rust
against a regional model, not transliterating it — but we read it first.

### Take (re-derive)

| AHD source | What it teaches |
|---|---|
| `corporations/sectorCalculations.ts`, `sectorProfitBasis.ts`, `sectorRevenueBasis.ts` | the sector P&L identity — what revenue, margin and cost actually mean |
| `corporations/sectorGrowthCost.ts`, `plantCosts.ts`, `plantLedger.ts`, `capexTxLog.ts` | capex, capacity growth, construction queues, salvage |
| `corporations/corpMarketShare.ts`, `marketShare.ts`, `sectorValuation.ts` | share, valuation, and how capacity converts to revenue |
| `corporations/sectorInventory.ts`, `turn/corporation/producer*` | production stages, input consumption, inventory carry |
| `corporations/applyPriceMultipliers.ts`, `marketQuote.ts`, `orderFlowEngine.ts` | pricing, quotes, order flow, wash-trade guards |
| `turn/corporation/demandThrottle.ts`, `brandLoyaltyTurn.ts`, `brandQualityTurn.ts` | demand elasticity, brand as a slow-moving asset |
| `turn/corporation/rdInnovation.ts` | tech level as a compounding, capital-hungry advantage |
| `corporations/corporateOwnership.ts`, `ipoIssuance.ts`, `hostileTakeoverNotifications.ts`, `mergerReview/*` | cap table, IPO, tender offers, antitrust as a real constraint |
| `corporations/financialFogOfWar.ts`, `honestBalanceSheets.ts` | how much rivals should let you see — a good single-player fog rule |
| `corporations/operatorPolicy.ts`, `autopilot.ts`, `caretakerCeo.ts` | how to give NPC firms a cheap, legible policy brain |
| `corporations/legalStructure.ts`, `foundingCosts.ts`, `names.ts` | company formation, entity types, naming |

### Drop or defer

- **All of politics.** No bills, NPPs, elections, parties, ministers. AHD's political layer
  is most of its turn cost and none of this game's fantasy.
- **Index funds, sovereign bonds, multi-country ops, currencies, IMF, banks.** The
  slice needs corporate credit and equity, not a capital-markets simulator.
- **Real estate speculation, entertainment/media lanes, defense procurement.** Content
  breadth, not core loop. Revisit after the slice validates.
- **Counties.** Not just for cost — see §3.

---

## 3. Why regions, not counties

Counties were the obvious port, and they are the wrong unit for this game:

- **Facilities are the thing you place.** A player builds two plants in a metro, not in a
  county line. 3,142 counties put 3,100 rows of state between the player and every
  decision while adding no legible choice.
- **A county is not an economy.** Labour, commuting, suppliers and customers cross county
  lines constantly; a county-level labour pool is a fiction that produces false scarcity
  and weird wage arbitrage.
- **Regions make the interesting trade-off explicit.** With ~180 regions, *the region is
  the labour market*: every firm in Cleveland hires from the same pool. Your expansion
  raises your rival's wage bill. That is corporate warfare, and it is invisible at county
  granularity.
- **It deletes a layer instead of compressing one.** No market-cell mapping, no county
  adjacency graph, no aggregation heuristics. Demand clears per region. Freight runs
  region→region.

We still *use* county data — it is aggregated up into the regions in the build pipeline
(§6). The model is regions; the data is as granular as the sources allow.

---

## 4. Why AHD turns are slow, and the rules that make that impossible here

Measured from AHD production (`docs/plans/2026-09-18-turn-performance-plan.md`):

| symptom | number |
|---|---|
| whole turn, median / p95 | 112.6 s / 191.5 s |
| `corporationTurn` median | 23.2 s |
| Mongo commands per turn | ~21,000–22,000 |
| BSON decoded per turn | 436 MB → 168 MB after projections |
| per-phase hard ceiling | 240 s |

The binding constraints are **round trips × latency** and **BSON decode**, not language
speed. A rewrite alone would not fix AHD. Ours avoids the shape entirely:

**R1 — One tick is one pure function.**
`fn step(&mut World, &Commands) -> TickReport`. No I/O, no `await`, no database handle,
no clock, no filesystem. Anything needing a side effect returns a request the host
drains *after* the tick.

**R2 — State of arrays, not arrays of objects.**
`struct RegionTable { pop: Vec<u32>, wages: Vec<u32>, ... }` indexed by dense ids.
No `HashMap`, `Box<dyn Trait>`, or per-entity allocation in the hot path. Entity ids are
`u32` indices; deletion is swap-remove with a generation check.

**R3 — Only touch what changes, and only when it changes.**
- 180 regions is small enough to refresh in full every week — no stagger, no
  approximation, no "the map is 8 weeks stale" bug class. (Staggering returns only as an
  option if the national map ever outgrows the budget.)
- Facilities that are idle, fully contracted and stable take a cheap path.
- Rival AI runs a **budgeted slice**: N firms per week, round-robin, so AI cost is
  constant regardless of firm count.
- Market clearing runs per **product category × region**, never per facility.

**R4 — Simulation is not coupled to the frame.**
The sim owns a thread. UI renders from an immutable snapshot. A slow tick can never stall
SwiftUI, and 60 fps is never contingent on the economy.

**R5 — Budgets are tests, not aspirations.**
A headless benchmark runs every tick type on CI and fails the build if p99 regresses
more than 10%. A per-phase budget table is asserted in debug builds.

**R6 — Saves and replays are single blobs.**
`postcard`/`bincode` snapshot, versioned, written off-thread, ~2 MB ceiling. Plus a command
log so `seed + commands = identical state hash` on iOS, macOS and Linux CLI.

---

## 5. Simulation model

### 5.1 Entities

| entity | storage | notes |
|---|---|---|
| **Region** (~180; slice ~13) | SoA table | population, labour by skill tier, unemployment, wage index, land cost, energy cost, freight index, demand vector, supplier depth, local tax/incentive, urban/rural class |
| **Firm** | SoA | cash, debt, equity, brand, reputation, credit rating, tech, AI archetype or player |
| **Facility** | SoA | firm, region, industry, capacity, utilisation, workforce, tech, age, inputs, output stock |
| **Contract** | compact array | supply, labour, lease, debt, offtake; term, price, volume |
| **Share lot** | compact cap table | ~5–40 holders per firm, not a market microstructure book |
| **Event** | queue | scripted + stochastic, resolved weekly, one at a time |
| **Freight lane** | sparse table | region-pair cost/transit, derived from distance, mode, volume |

Money is **i64 cents**. Fractional-feeling quantities (brand, utilisation, goodwill) use
fixed-point too, so iOS and Linux produce identical state hashes. No `f64` in state.

### 5.2 Region attributes that actually matter

Each region carries the things a company would care about, and they are all derived from
real data, then normalised for play:

- **Labour**: population, participation, skill tiers (low / mid / high), unemployment,
  prevailing wage by tier, union density, in-migration rate.
- **Costs**: industrial land, electricity, gas, water, construction cost index.
- **Logistics**: freight index, distance to the slice's external gateways, port/rail access.
- **Demand**: household count, income, and a spend vector across ~25 product categories.
- **Supply depth**: which industries have local suppliers, so vertical integration and
  supplier failure mean different things in Pittsburgh than in Indianapolis.
- **Policy**: property tax, incentives available, permitting speed — the lever a region
  pulls to attract your plant, and a source of event cards.

### 5.3 Industries (15, from AHD's `CORPORATION_TYPES`)

`agriculture · extraction · energy · chemical_industries · manufacturing · construction ·
logistics · retail · healthcare · financial · technology · media · telecommunications ·
real_estate · defense`

Each is a recipe book: inputs (commodities/services), labour skill mix, capital intensity,
energy intensity, logistics intensity, build time, tech curve. Vehicles and entertainment
are later sub-lanes, not slice features.

### 5.4 Tick pipeline (ordered, single pass, 260 weeks = 5 years)

| # | phase | cadence | budget (p50) |
|---|---|---|---|
| 1 | apply validated player commands | every tick | <0.1 ms |
| 2 | macro: rates, commodity indices, national demand | weekly, cheap | 0.2 ms |
| 3 | regional labour + wage + unemployment update | weekly, all regions | 1.0 ms |
| 4 | facility production + input draw | active facilities only | 1.5 ms |
| 5 | freight and contract settlement | weekly | 0.5 ms |
| 6 | market clearing per category × region | weekly | 1.5 ms |
| 7 | firm finance: P&L, cash, interest, tax | weekly | 0.5 ms |
| 8 | capital: construction, R&D, M&A offers | weekly | 0.5 ms |
| 9 | rival AI decisions | N firms/week, round-robin | 1.0 ms |
| 10 | valuation, share price, credit rating | weekly | 0.3 ms |
| 11 | events / narrative beats | weekly | 0.3 ms |
| 12 | snapshot + metrics ring buffer | weekly | 0.3 ms |

**Total p50 ≈ 8 ms, p99 target 12 ms.** The regional model leaves real headroom: if a
phase misbehaves we can afford to make the labour market considerably richer before
touching the architecture.

### 5.5 The core loop the player feels

1. Pick a region and an industry; found or acquire a firm.
2. Build capacity, hire, contract inputs, set price and brand spend.
3. Sell into regional demand; watch margin, share and cash.
4. Expand into a second region, build a distribution network, or buy a rival.
5. Rivals respond — undercut you, poach your staff (raising *your* wages), bid for your shares.
6. Five years later: valuation, market position, and whether you still control the company.

### 5.6 Rivals and the narrative layer

- **24 seeded corridor firms + 6 national entrants** at slice start, each with an archetype
  (cost leader, brand builder, consolidator, technologist, vulture) expressed as weights
  over the same decision set the player has.
- **Light narrative**: ~12 event cards in the slice — a plant closure, a regional incentive
  offer, a poaching raid, a supplier failure, a hostile stake, a recall, a rate shock.
  Events are data, not code paths; they queue, resolve in one week, and leave a trace.
- **Fog**: rivals' internals are partly hidden (AHD's `financialFogOfWar` idea) so due
  diligence is a decision, not a lookup.

---

## 6. Time and pacing

- **1 tick = 1 week.** 52/year, 260 for the 5-year slice.
- Player can pause, run at 1×, or fast-forward; because a tick is ~8 ms, fast-forward is
  just ticking more often, not skipping simulation.
- Decision latency is the pacing lever: with 260 weeks, a build that takes 26 weeks is a
  strategic commitment, and a bad quarter is recoverable.
- Target: **a full 5-year run in 45–75 minutes**, with meaningful decisions in most weeks
  and no dead weeks (playtest analytics: share of ticks with ≥1 actionable item ≥ 40%).

---

## 7. Data: real US regions, baked in

**Source of truth is a build-time pipeline, not a runtime fetch.** Output is a compact
binary shipped in the app; the app never phones home for data.

The trick that makes this cheap: **BEA economic areas are defined as sets of counties.**
We aggregate county-level public data up into regions, so the region table is real,
and the granularity of the sources is not our limit.

| dataset | use | source |
|---|---|---|
| BEA economic areas (component-county map) | the region universe and boundaries | BEA / ERS |
| County population, age, income | labour pool, demand base | Census / ACS 5-yr |
| GDP by county, industry, and metro | baseline output, industry mix, demand | BEA CAGDP2 / CAGDP9 |
| Employment and wages by county × industry | labour cost, skill mix | BLS QCEW |
| Establishments by county × NAICS | competition density, supplier depth | Census CBP |
| Rural–urban continuum, typology | logistics, wage, demand modifiers | USDA ERS |
| Electricity and fuel prices | energy cost | EIA |
| Freight/road network proxies | logistics cost | BTS / FHWA |

- **Licensing:** US federal data is public domain; ship attribution in About and keep a
  `DATA-SOURCES.md` with vintage per dataset.
- **Size:** ~180 regions × <1 KB → well under 200 KB national; the corridor slice is a few KB.
- **Gaps:** missing region×industry cells fall back to the national average and are flagged
  in the dataset, never silently zeroed.
- **Pipeline:** `tools/data-gen` → `crates/cw-data/src/us.bin`, checked in with a version and
  a hash. Regeneration is a documented command, not a manual step.
- **Boundaries for the map:** simplify BEA polygons to ~200 vertices per region, or ship a
  hex cartogram. At 13 regions on a phone, a labelled cartogram beats a real map.

---

## 8. Tech stack and repo shape

```
corporate-warfare/
  crates/
    cw-core/      state, ids, money, determinism, RNG (PCG/SplitMix64, seedable)
    cw-data/      baked region/industry/product datasets + schema
    cw-sim/       tick pipeline, all game rules (the only crate with game logic)
    cw-ai/        rival policy, budgeted scheduler
    cw-ffi/       UniFFI surface: new_game, apply, tick, snapshot, save, load
    cw-cli/       headless runner: sim, bench, sweep, replay
  apple/
    CorporateWarfare.xcodeproj        SwiftUI app (iOS 17+, macOS later)
    Sources/                          screens, snapshot adapters, design system
  tools/
    data-gen/     region dataset pipeline
    balance/      sweep reports
  docs/
    PLAN.md · DATA-SOURCES.md · SIM-CONTRACT.md · SLICE.md
```

- **Rust 2024 edition**, `no_std`-friendly core, `criterion` benches for ticks.
- **Binding:** UniFFI → staticlib → XCFramework (`aarch64-apple-ios`, `aarch64-apple-ios-sim`,
  `aarch64-apple-darwin`). Swift holds an opaque `SimHandle`; the sim runs on its own serial
  queue; UI reads `Snapshot` structs (SoA slices, no per-frame world copy).
- **UI:** SwiftUI, no third-party state framework. One design system module. Tabs:
  Company · Market · Map · Rivals · Finance.
- **Desktop later:** macOS reuses the same SwiftUI app (cheap). Windows/Linux is a Tauri v2
  shell over the same FFI with a TypeScript UI — a later decision, not slice scope.
- **No server.** No accounts. Optional iCloud sync of save blobs, off by default.

---

## 9. First slice — the Great Lakes corridor, ~13 regions, all 15 industries, 5 years

**The corridor:** Cleveland · Akron · Canton · Youngstown · Pittsburgh · Columbus · Dayton ·
Cincinnati · Toledo · Detroit · Fort Wayne · Indianapolis · Lima/Findlay, plus Appalachian
extraction country (Wheeling/Charleston). Roughly 13–14 BEA areas across OH, PA, MI, IN, WV
and northern KY.

Why this corridor: it contains every industry in the model and a real gradient between
them — heavy manufacturing and chemicals in Cleveland/Detroit, legacy steel in
Pittsburgh/Youngstown, insurance and finance in Cincinnati, logistics in Columbus/Toledo,
tech and healthcare in Columbus and Indianapolis, agriculture in western Ohio and Indiana,
extraction in Appalachia. It is a whole economy, cross-state, at a scale a phone can show.

**Alternatives if the owner prefers:** the I-75 auto corridor (Detroit–Toledo–Dayton–
Cincinnati–Lexington–Knoxville–Atlanta) for a single-industry identity, or the Southeast
Piedmont (Charlotte–Raleigh–Nashville–Atlanta) for a growth-market identity.

External gateways (Chicago, Louisville, the East Coast) exist as **off-map demand and
supply sources** priced by freight lane — not playable regions.

### In the slice

- New game → choose region, industry, legal structure, starting capital (seed scenarios).
- Build/expand/retool/mothball facilities; hire and set wages; contract inputs; set prices;
  brand and R&D spend; borrow; acquire a distressed rival; defend against a stake.
- 15 industries, ~25 product/commodity categories, regional demand, inter-region freight,
  off-map gateways.
- 30 rival firms, budgeted AI, four archetypes.
- 12 event cards; onboarding that teaches one decision at a time.
- Instant pause/play, autosave, save slots, deterministic replay.

### Explicitly out of the slice

Bonds and credit markets beyond a bank term loan · index funds · politics and legislation ·
international operations · currencies · unions · media/entertainment lane · vehicle lane ·
real-estate speculation · antitrust merger review beyond a simple concentration check ·
multiplayer · cloud accounts · the full ~180-region national map (M6, after the slice proves
the tick) · sub-region sites and parcels.

### Definition of done for the slice

1. A stranger can start a company, reach year 5, and explain why they won or lost.
2. Tick p99 ≤ 12 ms and ≥ 55 fps sustained on an iPhone 13 or newer, measured on device.
3. `cw-cli replay` reproduces the exact final state hash from a recorded command log on
   iOS, macOS and Linux.
4. Save/load round-trips to an identical state hash; save ≤ 2 MB.
5. Ten golden 5-year runs produce sane distributions (no runaway monopolies, no death
   spirals, ≥ 35% of seeded rivals still solvent at year 5, player win rate 40–60%).
6. TestFlight build installable by internal testers.

---

## 10. Milestones

| # | milestone | focus | exit evidence |
|---|---|---|---|
| M0 | **Skeleton + harness** | repo, workspace, CI, determinism harness, tick budget harness, corridor dataset pipeline | `cw-cli sim --corridor --years 5` prints a P&L; benchmark gate runs in CI |
| M1 | **Core economy** | regions, firms, facilities, recipes, production, demand, freight, finance | headless 5-year run is internally consistent (cash reconciles, no negative inventories) |
| M2 | **Markets, rivals, valuation** | regional markets, share, pricing, budgeted AI, share price, M&A | 30 rivals behave plausibly across 100 seeded runs |
| M3 | **FFI + SwiftUI shell** | new game, company, market, map, finance, pause/play, save | playable 5 years on simulator, then device |
| M4 | **Narrative + onboarding + balance** | event cards, tutorial, sweep-driven tuning | 3 external playtesters finish a run |
| M5 | **Slice release** | TestFlight internal, device perf, crash-free | slice definition of done, all six criteria |
| M6 | **National expansion** | all ~180 regions, out-of-corridor operations | tick p99 still ≤ 12 ms nationally |
| M7 | **Desktop** | macOS build; then Windows/Linux decision | same core, same save, same replay hash |

Shape: M0–M2 are the risk (the model and its cost). M3 is the longest single build. M4 is
where it becomes fun and is the most likely to slip. Do not start M6 before M5 ships.

---

## 11. Performance budget and gates

| gate | target | enforced |
|---|---|---|
| tick p50 / p99 (corridor) | 4 ms / 12 ms | CI bench, blocks merge on >10% regression |
| tick p99 (national, M6) | 12 ms | CI bench with ~180 regions |
| memory, steady state | < 80 MB | Instruments, device gate |
| save size / write time | ≤ 2 MB / < 150 ms off-thread | test |
| cold start to interactive | ≤ 1.5 s | device gate |
| UI frame time during tick | ≤ 16.7 ms | device gate |
| allocations per tick | 0 in steady state | counting allocator in debug |

The counting allocator turns "please write data-oriented code" into a failing test.

---

## 12. Determinism, saves, and testing

- **Seeded RNG streams per subsystem** (regions, events, AI) so adding a draw in one system
  cannot shift another. Draw order is part of the contract.
- **Iteration order is explicit** — every loop runs over an index range or sorted id slice,
  never a hash map.
- **Command log**: `(tick, command, args)`; replay is the primary integration test.
- **Golden runs**: 10 seeds × 260 ticks, state-hash pinned; a rule change produces a
  readable diff of final metrics, not just a hash mismatch.
- **Sweep harness** (`cw-cli sweep`): thousands of headless runs over parameter grids.
  This is the real balance tool — not manual play.
- **Test pyramid**: unit (rules, arithmetic), property (money conservation, inventory
  non-negativity, cap table sums to 100%), integration (replay), device (perf).

---

## 13. Risks

| risk | why it bites | mitigation |
|---|---|---|
| **Balance is the real cost, not code** | 260 ticks of compounding make small errors explode | sweep harness in M0; golden distributions; tune parameters, not special cases |
| **Region aggregates hide local reality** | a "region" can be a city plus 300 miles of farmland | weight demand/costs by population and employment; keep urban/rural mix in the row, not a single scalar |
| **BEA definitions drift across vintages** | boundary revisions change the map under saved games | pin one vintage in the dataset; saves store region ids + dataset version |
| **Cross-region logistics can become a routing game** | players build spreadsheets of lanes | cost is a lane price, not a network solve; one distribution hub per region max |
| **Rust↔Swift boundary churn** | every rule change wants a new struct | versioned snapshot schema; FFI surface frozen at 8 calls |
| **Scope creep toward AHD parity** | "we already have it in AHD" invites porting politics | freeze the port list in §2; anything not listed is a new decision |
| **Naming/repo collision with old `corpgame`** | the old repo is also called Corporate Warfare | archive old repo, new repo `corporate-warfare`; see §14 |

---

## 14. Open decisions for the owner

1. **Corridor:** Great Lakes (recommended) vs I-75 auto corridor vs Southeast Piedmont.
   This is the slice's identity, so pick before M1.
2. **Repo:** create `lakesidegamedev/corporate-warfare` (private) and archive
   `lakesidegamedev/corpgame`; move `/root/misc/corpgame` and
   `/root/misc/corporate-warfare-sandbox` to
   `/root/misc/archive/2026-10-25-corporate-warfare-v1/`. Confirm before I touch the old repos.
3. **Starting position:** found a company with seed capital, or inherit a struggling firm
   with debt and a board (better tutorial, harder first hour)?
4. **Monetisation:** premium one-off (fits a deep single-player sim) vs free + expansion
   regions. Changes onboarding and content pacing, so decide before M4.
5. **End condition:** 5-year score, or open-ended with a valuation/control target? The slice
   assumes 5-year scored; open-ended is a later mode.
6. **Kanban:** register this as a project on the ops kanban and add M0–M5 as items.

---

## 15. What I would do first, concretely

1. Create the repo skeleton and the **tick budget harness before any game logic** — the
   performance promise is the product, so it gets a test on day one.
2. Build the **corridor region dataset** and print it: 13 regions, industry mix, wages,
   demand, freight lanes. If it does not look like the Great Lakes, fix that before writing rules.
3. Write the **P&L identity** for one industry end to end (agriculture, simplest recipe) and
   get `cw-cli sim` producing a believable 5-year run.
4. Only then add the second industry, then rivals, then UI.
