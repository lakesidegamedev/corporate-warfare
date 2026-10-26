# Changelog

All notable changes to Corporate Warfare are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow the policy in
[docs/RELEASING.md](docs/RELEASING.md): patch for minor updates and fixes, minor for big
features, major for wide feature releases or upgrades.

Every pull request that changes the game, the app, content or tooling adds its entry under
**Unreleased** in the same pull request. A release moves those entries under a new version
heading.

## [Unreleased]

Planned as **1.1.0**: single-player A House Divided corporations on a real map of the Great
Lakes and Ohio Valley. Saves from 1.0.0 do not load in this release.

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

[Unreleased]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/lakesidegamedev/corporate-warfare/releases/tag/v0.1.0
