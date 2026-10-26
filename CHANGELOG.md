# Changelog

All notable changes to Corporate Warfare are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow the policy in
[docs/RELEASING.md](docs/RELEASING.md): patch for minor updates and fixes, minor for big
features, major for wide feature releases or upgrades.

Every pull request that changes the game, the app, content or tooling adds its entry under
**Unreleased** in the same pull request. A release moves those entries under a new version
heading.

## [Unreleased]

### Added

- A new 8-bit Corporate Warfare logo: a gold C beside a W drawn as a stock line that runs green
  to red and breaks out into an arrow. It is the app icon on iPhone, iPad, Mac, Android, Windows
  and Linux and leads the redesigned start screen, with a row of the new 8-bit sector icons.
- 8-bit icons for all fifteen sectors, shared by every platform.
- A speed control: pause, 1x, 2x and 4x (one tick is one week; 1x is a week a second).

### Changed

- Buttons no longer flash while weeks process: a running week does not mark the app busy.
- The seed field is gone from the new-career screen; every career gets a random seed.
- Test runs no longer send anonymous market data.
- A proper Settings page, opened from a gear on Portfolio, holds saving, analytics, saves and
  recovery, the walkthrough and the version; Portfolio is back to the player's money.
- On the map, Next week and the speed buttons fold into one time pill showing the current speed;
  tap it to open them. The "circles are your divisions" legend is gone.

### Fixed

- iPhone release builds failed at signing once the team reached Apple's cap on development
  certificates, because every CI run creates a new one on its fresh Mac. The release workflow
  now revokes the certificates earlier CI runs created through the API before it signs, and the
  iPhone archive is built without Mac Catalyst.

## [1.4.0] - 2026-10-26

### Added

- Android version, first build: the same Rust engine through uniffi's Kotlin bindings with a
  Compose interface shared with the coming Windows and Linux versions. A career can be started,
  continued from its autosave, played week by week or on auto-advance, and saved; the CEO Office
  shows the company, the quarter's profit breakdown, the pricing panel with postures and the
  divisions; Markets shows a region's board; Inbox answers decisions; Portfolio shows wealth, the
  cash outlook and the companies. The map is a region list for now. A new CI workflow builds
  the engine for Android, runs an engine smoke test through the Kotlin bindings and uploads a
  debug APK.
- The Android, Windows and Linux versions gain the drawn corridor map (pan and zoom, your
  divisions with plant counts, room to build and wage layers, every region's board on tap),
  building plants from a region with the full quote, and the Markets exchange: stocks with the
  corridor index and buy and sell orders, commodities with their movement, and region boards.
- Every GitHub release now carries a signed Android APK, the Windows installer, the Linux
  package and the Mac app, built by a new release workflow. Android, Windows and Linux take
  their version from the same workspace version as the iPhone app.
- Mac version: the iPhone app builds for the Mac through Mac Catalyst under the same App Store
  record, sandboxed, with the engine built for Apple silicon and Intel Macs.
- Windows and Linux versions, first build: the same Compose app in a desktop window, packaged as
  an MSI on Windows and a .deb on Linux by CI. Saves go to the usual per-user application data
  folder.
- Markets is now an exchange with three sections. Stocks lists every operating company with its
  share price, weekly move, market cap and a sparkline, under a market-cap weighted corridor
  index; sort by top movers, size or your holdings, and open a company for a price chart, P/E,
  fair value, free float, your position and a buy or sell order. Commodities lists every good's
  corridor price (the median ask of the companies that make it, or the import price where
  nobody does) with its weekly and quarterly move, a sparkline and supply against demand; open
  one for its chart, import and export quotes and, for your own goods, your pricing posture.
  Regions keeps each region's board.
- The engine keeps a year of weekly closing prices for every good and share.

### Fixed

- Company valuations used one quarter's EBITDA from week thirteen on instead of a year's, so
  every fair value fell to about a quarter of the rule (six times annual EBITDA) once a full
  quarter had closed. Opening share prices now reflect each company's expected plant earnings
  instead of the book floor alone, so shares no longer all climb in step at the weekly limit over
  the first quarter. Share prices show cents, and a P/E above 200 shows as n/m.

## [1.3.1] - 2026-10-26

### Added

- Pricing posture, from A House Divided. Every good a company makes can be priced
  automatically, at a step from 20 percent under to 20 percent over the market, or at a markup
  of 10 to 50 percent over the company's own cost per lot. A new Pricing panel in the CEO Office
  lists each good with its price, its profit per lot (green or red) and the share of last week's
  output that sold; tap one to change its posture or set an exact price. "Set your price" in
  Markets opens the same sheet.

### Changed

- Plants make money when run well. Twenty-one strategies earned under 20 percent of revenue at
  normal prices after wages, inputs, upkeep and depreciation (farms 6 to 9 percent, the truck
  carrier below zero, goods assembly 2 percent); their crews are trimmed so each now earns about
  17 to 24 percent. Output and market volumes are unchanged, and the released workers return to
  the regional labour pool. A guided career with no player action now ends its second year with
  about $1.4 million in company cash on average, up from about $1.0 million.
- Farms run seasonal crews: in the 80 percent yield quarters they keep 80 percent of the crew,
  so wages follow the harvest instead of staying fixed while output falls.
- Building a plant is paid in stages: the entry fee and construction service when work starts,
  the equipment when it is delivered. A company short of cash at delivery waits a week at a time
  instead of falling into arrears. The build quote shows what is due now and on delivery.

## [1.3.0] - 2026-10-26

### Added

- A corridor commodity exchange for goods. Every company's need for an input, in any region,
  meets every seller's stock in any region and the import offer of the buyer's own region,
  priced as delivered (asking price plus freight over the lane) and filled cheapest first, so
  domestic producers supply domestic plants and the outside market is the last resort. Buyers
  tied on one source share it in proportion. Exchange deliveries arrive within the week.
- Production planning from sell-through, for rivals and (automatically, until set by hand)
  the player's companies: more than four weeks of unsold stock cuts the target fifteen
  points down to thirty percent, under a week and a half of stock raises it back toward full;
  services follow sales against capacity. `SetAutoProduction` hands planning back.
- Anonymous market analytics: every five game weeks the app posts the engine's
  market-balance snapshot (per good: made, sold, imported, exported, held, median asking price
  against reference, household spend; corridor health; the player company's results) with a
  random install id to the Lakeside ops ingest. No names, account or device data. On by default
  with a switch under Portfolio, Settings; the privacy manifest declares Gameplay Content for
  Analytics, not linked, not tracking. The privacy audit allows network use only in the telemetry file and only to that endpoint, and rejects any other collected data type, SDK or device identifier.
- Automatic pricing for the player's companies: every good they make follows the same
  sell-through rule as rivals each week unless the player sets its price by hand; a price of
  zero hands it back.

### Changed

- Production is planned plant by plant. A firm's unsold stock in one good no longer cuts output
  at its other plants, and stock it keeps as inputs for its own plants is not counted as unsold.
- Interface overhaul from a design audit. Every main screen has a compact header with its action
  beside the title instead of a large title and empty space, and scrolled content ends at a hard
  edge under the tab bar instead of showing through it. CEO Office opens with a one-line identity,
  company cash and the quarter's profit grouped in one panel, and a breakdown of where the
  quarter's money went (revenue against inputs, wages, overheads, depreciation, interest and tax)
  with the biggest driver named. Markets shows one line per good with a signal (imported, tight,
  glut), sales and weeks of stock in words, filters for short supply, gluts and your own goods,
  and detail and pricing on tap. Inbox lists decisions first and only the news there is; a quiet
  week is one line. Portfolio leads with net worth and shows where company cash is heading.
  Saving and analytics moved to a Game panel. Step week and Play are now Next week and
  Auto-advance. The map's goal strip folded into the company panel, its legend is one line with
  detail on tap, and the HUD shows quarterly profit instead of last week's EBIT.
- The outside market quotes reference plus or minus 15 percent, up from 5, so domestic supply
  and demand set prices inside a real band.
- Freight costs $0.03 a lot per kilometre, down from $0.10, so goods can travel between
  regions at a fraction of their value.
- Pricing (rivals and automatic player pricing) sets one price per good from weekly
  sell-through against all stock held: more than four weeks of stock cuts 5 percent, selling
  out raises 2 percent, otherwise it holds, never above the cheapest import price.

### Fixed

- Mothballed plants can reopen. Mothballing now releases the whole workforce whether the player
  chooses it or an insurance catastrophe forces it; before, a catastrophe-mothballed plant kept
  paying full wages with no output and no way back, which bankrupted the corridor's only iron
  mine within months. Reopening costs 5 percent of the plant's capex and rehires through the
  labour market. Rivals reopen when the output pays and the corridor is importing it. Each plant
  row in the CEO office has Mothball and Reopen buttons.
- Supply contracts deliver only what the buyer can pay for in full, goods and freight together.
  Before, a buyer short of cash still received the whole week's goods and the unpaid part went
  onto its overdue bills while the seller was never paid for it. The rest now stays with the
  seller to sell on the exchange.
- Rival companies grow again. Their limit of six plants was never rescaled when 1.2.0 cut plants
  to a fifth of their size, so almost no rival could build after 1.2.0; the limit is now thirty.
  Cautious rivals also enter a market when the corridor imports at least a full plant's worth,
  and demand for goods is judged across the whole corridor, matching the commodity exchange. A
  rival now drills oil in the corridor, which had always imported all its fuel.
- Business purchases only cleared inside one region, so producers exported everything and
  buyers in other regions imported the same goods: the corridor barely traded with itself.
- Imports took a share of every order by attractiveness even when cheaper local stock sat
  unsold; goods now fill in merit order.
- Rival pricing compared a quarter's sales with current stock, so anything that sold at all
  ratcheted to its cap and anything that did not slid toward half price.
- Power dispatched to the grid and construction ordered for builds were not recorded as
  sales.
- Contract deliveries were not recorded as sales, so pricing, the market board and the AI saw
  contract sellers as selling nothing.

## [1.2.0] - 2026-10-26

Economic regions, sector divisions and corporate warfare. Saves from 1.1.0 do not load in this
release.

### Added

- 31 economic regions inside the eight states, built from whole counties joined to the
  nearest anchor metro in the same state, with Census 2000 populations and SAIPE 2000
  incomes. A region is the economic unit: its own labour pool, wages, household demand,
  land cost, goods markets and freight lanes. The state is the political layer: conditions,
  union law and taxes.
- State specializations: each state's two specialized sector types use 3 and 1.5 percent
  fewer inputs (for example West Virginia extraction and chemicals).
- Sector divisions, as in A House Divided: the first plant of a sector type in a region opens
  a division for a $100,000 entry fee with the founding plant at half price; later plants
  join it and run its strategy, and a retool moves the whole division.
- Market share and dominance per region and sector: expanding past half a market costs up to
  three times as much, softened when there are few rivals.
- Market raids: spend a marketing campaign against a rival's division in a market you both
  serve to take up to a fifth of its plants, bought at half book value. Odds follow the
  campaign's strength against the defender's; the defender keeps one plant; each market has
  a 13-week cooldown. Rivals raid too, including you.
- The map draws regions inside the states and shows only your divisions: one sector circle
  per owned sector type in a region, with a badge counting its plants.
- Region boards: the state's specializations, every sector's operators, leader and your
  share, raid options, room to build, conditions and plants. Market news in the Inbox.
- The build flow quotes the entry fee, founding discount, dominance surcharge and the input
  factor where the plant would sit.
- Board objectives: each quarter the board of your primary company sets one goal (a new
  region, a new state, an operating profit, a market share). Meeting it pays you a
  management bonus from the company, 2 percent of its quarter's operating profit and at
  least $25,000, moved from its cash. The goal shows over the map and in the CEO Office.
- An `expander` reference policy for sweeps: every twelve weeks it builds the plant whose
  three years of contribution at reference prices best repay its full cost, in any sector.
- The map pipeline is a committed script (`tools/map/build_map.sh`) and regions a committed
  generator (`tools/map/build_regions.py`).

### Changed

- The five-year score measures what you built over the opening position: growth of net
  worth (20 when flat, 100 at double) 45 percent, expansion (divisions and states beyond
  the first) 25 percent, market leadership 10 percent, resilience 10 percent and conduct 10
  percent. Control is still shown but no longer scored. Holding the starting company now
  scores about a third instead of matching active play.
- A plant is a fifth of its 1.1.0 size: output, inputs, crews, capex and land rent all
  divide by five, and seeded companies hold five times the plants, so the economy keeps its
  capacity. A farm plant costs $400,000 instead of $2,000,000 and opening a division with a
  founding plant about $300,000. Standard strategies keep their calibrated margins.
- Sprawl counts divisions, as in A House Divided's per-sector rule, instead of plant sites:
  adding plants to a division you already run never sprawls.
- Plants managed from outside the home region pay 2.5 percent more for inputs, outside the
  home state 5 percent.
- Services that reach customers statewide (software, telecom, advertising, research,
  maintenance, care, training, water, construction) clear across the state; goods, retail,
  leases and electricity clear in their region.
- Public works and outside gateway capacity are sized per state and split by region demand.
- Markets picks a region from a menu grouped by state; the CEO Office groups plants by
  division and retools whole divisions.

### Fixed

- Corporate tax was assessed on each rolling quarter's profit with no offset for losing
  quarters, so a seasonal business paid tax on its summers and was never credited for its
  winters. Losses now carry forward against later profit.
- `tools/design_check.py` held a copy of the farm worked example at the old plant size.
- The build preview always previewed the first region instead of the chosen one.
- Advertising split across media firms dropped the rounding remainder: the buyer paid more
  than the sellers received.
- A mine whose deposit was smaller than a week's output stranded a rounding remainder.

## [1.1.0] - 2026-10-26

Single-player A House Divided corporations on a real map of the Great Lakes and Ohio Valley.
Saves from 1.0.0 do not load in this release.

### Added

- Eight playable states (Illinois, Indiana, Kentucky, Michigan, New York, Ohio, Pennsylvania,
  West Virginia) with Census 2000 populations, rounded circa-2000 GDP, pay and union indices,
  and authored state conditions: power grid, roads, broadband, workforce skill, crime and cost
  of living. Freight distances come from real hub-to-hub distances.
- A real vector map bundled with the app (`apple/Resources/corridor-map.json`): state and
  county boundaries, the Great Lakes, rivers, interstates and cities, from public-domain US
  Census and Natural Earth data, with reproducible build scripts in `tools/map/`.
- A House Divided sector strategies: every sector type now offers three to five operating
  strategies for a 2000 start (for example Coal-fired, Gas Turbines, Nuclear and Renewables in
  energy, or Industrial, Precision and Sustainable farming), 53 in all, authored by
  `tools/content/build_sectors.py` so each strategy is a trade-off rather than a strictly
  better choice.
- Nine new goods so supply chains read like AHD's: coal, natural gas, timber, rare earth
  minerals, plastics, fertilizers, pharmaceuticals, building materials and vehicles.
- Sector focus: a primary and optional secondary sector type. On-focus plants use fewer
  inputs; off-focus plants pay a coordination overhead; changing focus has a four-week
  transition and an eight-week lock (`SetSectorFocus`).
- Logistics strength, built by a weekly logistics programme (`SetLogisticsBudget`), and a
  sprawl overhead beyond fifteen plants that logistics strength relieves.
- State conditions shape output (grid reliability, skill, broadband), freight cost (roads) and
  retail and property shrinkage (crime).
- Gateways record imports and exports, giving a public room-to-build signal.
- A map-first app: the real corridor map is the home screen, with pan and zoom, plant markers
  coloured by sector (your plants ringed in gold), and layers for your presence, room to build,
  wages and workforce skill. Tapping a state opens its board: conditions, the plants operating
  there and what it imports. On iPad the board docks beside the map.
- A House Divided style building: pick a sector type, compare its strategies (inputs, margin,
  capex, build time, staff, deposits, local imports) and confirm against the engine's preview.
- A CEO office: masthead, the week's key figures, sector focus with the change flow, logistics
  strength and programme, and every plant grouped by sector with utilisation, bottleneck and a
  retool flow.
- UI snapshots of the main screens on iPhone and iPad, rendered in CI for review.
- On a portrait phone the map frames the middle of the corridor at a readable size instead
  of fitting all eight states into a strip. City labels draw above plant markers with a
  halo, and plants fan out on the side away from the label. The map header keeps the
  company name on its own line when the figures do not fit beside it.
- Markets is a commodity board for the selected state: goods grouped by sector, last price
  against reference, weekly sales, local stock, gateway prices and import room.
- Corporation setup at founding: headquarters state, primary sector, a named founding strategy
  and an optional secondary sector, set at founding without the refocus transition.

- Release process: this changelog, the versioning and release procedure in
  `docs/RELEASING.md`, a pull request template, and a CI check that requires a changelog entry
  for code, content and tooling changes and keeps the workspace and iOS versions in step.

### Changed

- The app uses a dark cartographic look with a sector colour and symbol system in place of the
  light dashboard style; tabs are Map, CEO Office, Markets, Inbox and Portfolio.
- Playable regions are whole states instead of eleven authored economic areas; the thirty
  named rivals moved to states where their industries actually sit and run named strategies.
- Retooling switches a plant to another strategy of the same sector for a quarter of the new
  strategy's capex and a third of its build time (at least four weeks); a plant can no longer
  retool into a different sector.
- Every non-renewable extraction strategy now draws on a finite twenty-year deposit, as iron
  mining already did.
- Rival AI builds the best strategy of its sector in the state whose imports show room for it,
  judged over three years at public prices net of capex and financing, and retools plants stuck
  in glut toward strategies whose output is short.

### Fixed

- The outside economy settled through a $50 million account that a corridor export surplus
  emptied in about two years, after which gateways stopped buying and public works stopped
  paying, taking every exporter and construction firm with them. It now opens at $500
  million and draws at most one percent a week, so exports taper toward balanced trade
  instead of stopping. When short it buys only what it can pay for in full; before, a
  seller handed over a whole shipment for part of its price.
- Services, which have no import quote, stop at 15 percent over reference when rivals reprice.
- Only firms headquartered in a state could sell there: business, household and public-works
  sales and gateway exports all filtered on headquarters, so a plant built in another state
  could not sell its output. Sellers now only need stock in the state. Spot orders search
  every state where a producer holds the good.
- Business offers included a firm's own input stock, so rivals sold and then discounted the
  steel and plastics their plants needed. Offers now keep the plant's input cover back, as
  gateway exports already did.
- Liquidation wrote off the inventory account but left the physical stock in place.
- A shipment that a short-of-cash buyer paid only in part arrived carrying its full list
  value, more cost than the ledger recorded.
- Vehicle Assembly was priced for a 12 percent margin, below every other new good, and lost
  money on Michigan wages at gateway prices. Vehicles are now priced for 22 percent ($4,790).
- Rivals raised prices toward twice reference whenever the gateway bought their whole output.
  Prices now stop at the state's import price, above which local buyers import instead.
- Rerunning the sector content script duplicated the new goods.
- Every sale overwrote the week's unit sales for that good and state instead of adding to
  them, so a firm selling to households and then exporting recorded only the last fill.
- Rival pricing read the open sales week, which is empty between ticks, so rivals never
  repriced. It now reads the trailing thirteen weeks of closed sales.
- The market board counted only firms headquartered in a state, read the empty open week for
  sales, and showed reference price for every producer good. It now counts every firm's
  stock and sales where the goods sit, and quotes the lowest local ask, capped at the import
  price, for goods households do not buy.
- Negative compact amounts read "$-21K"; they now read "-$21K".
- "Washington,  D.C." lost its double space on the map.
- UI snapshots attach to the app's window scene; detached windows rendered blank, and the
  snapshot test now fails on a blank image.
- The map's zoom reset and board panel honour Reduce Motion.
- Choosing a state on Markets now reloads its prices instead of keeping the previous state's.
- Spot orders bought from the first out-of-state firm holding the good, which could strip a
  carrier of its own steel input; they now buy from the nearest producer.
- Favourable state conditions no longer push a plant beyond its rated capacity (they save
  inputs), and adverse ones cap the run rate before inputs and deposits, so a mine extracts
  exactly what it ships.
- Outside gateways were sized only to the corridor's input demand, so a coal field producing
  far more than local power plants burn could not reach the national market and failed in
  every career. Gateways now also absorb what the corridor's plants produce, at their
  discount.

## [1.0.0] - 2026-10-25

First complete engine release: every automated rules requirement closed with evidence, and a
second balance report over 860 careers.

### Added

- Takeover and antitrust fixtures with hand-computed escrow, fee, proration, squeeze-out and
  HHI arithmetic; lobbying, rival AI, event deck, score, liquidation, credit, governance and
  union fixtures.
- Real triggers for all 24 event cards, macro recessions, booms and fuel shocks from the macro
  stream, and tender cards addressed to the investor with their deal.
- Rival AI telemetry by archetype in the balance sweep, and diagnostic examples
  (`event_census`, `dissolution_census`, `firm_trace`).
- Passed board votes authorise large actions for thirteen weeks.

### Changed

- Antitrust review runs before any share or asset moves, and competition groups are judged at
  the ultimate controller.
- Rival strategic slots are scored and capped at four actions, use only public information,
  follow the rules' solvency order, and expand only into observed demand.
- Missed loan principal is carried as arrears owed to the same lender.

### Fixed

- Tender payouts could exceed escrow, and a player's tender created money.
- A shell subsidiary could bypass competition review; remedies lacked an authority check.
- Fourteen event cards could never open; card handlers deleted goods in transit, touched other
  firms' shipments, cancelled healthy contracts and capped capacity permanently.
- A player choice could veto a recession; macro regimes never fired.
- Rival takeovers never settled, a token player stake froze a rival's AI, and seasonal farms
  idled into insolvency (every farm failed within five years).
- Liquidation minted proceeds, never paid lenders and left dissolved firms with cash and live
  contracts.
- Missed principal was counted twice; board votes had no effect; retooled plants never returned
  to service.
- `AcceptLoanQuote` let a borrower name its own rate, term and size.
- Score control counted passive stakes and double-counted subsidiaries; conduct charged the
  player for rivals' breaches.

## [0.1.0] - 2026-10-25

Foundation, pull requests #8 to #69.

### Added

- Deterministic Rust engine: checked money and quantities, ledger, commands, session, replay
  and versioned saves; weekly economy with labour, production, utilities, markets, freight,
  contracts and gateways; ownership, boards, exchange, issuance, dividends, buybacks and IPOs;
  credit, bonds, banking and insurance; unions, lobbying, insolvency; research and brand.
- Rival AI, event deck, five-year score with continue, and the CLI (`doctor`, `run`,
  `diagnose`, `report`, `sweep`, `replay`).
- SwiftUI phone app over the engine with onboarding, save recovery and release workflows.

[Unreleased]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.4.0...HEAD
[1.4.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.3.1...v1.4.0
[1.3.1]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/lakesidegamedev/corporate-warfare/releases/tag/v0.1.0
