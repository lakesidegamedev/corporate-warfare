# Changelog

All notable changes to Corporate Warfare are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow the policy in
[docs/RELEASING.md](docs/RELEASING.md): patch for minor updates and fixes, minor for big
features, major for wide feature releases or upgrades.

Every pull request that changes the game, the app, content or tooling adds its entry under
**Unreleased** in the same pull request. A release moves those entries under a new version
heading.

## [Unreleased]

### Changed

- Buildings, city blocks, the countryside and every vehicle are true 3D voxel models, drawn from the
  camera's angle each frame: turn the map and you see their sides, vehicles turn as they follow the
  road, and the sun lights a fixed side of each building as you go round it.
- The Freight mode is redrawn: each route is a clean arc between its two hubs over a darkened map,
  with one coloured stripe per company that carries goods on it (yours in gold), wider where more
  freight moves and dashed for water, and a legend naming the companies. Clouds step aside while it
  is open.
- Freight routes no longer show outside the Freight mode. Elsewhere trucks drive on the real
  interstates and trains on the real railways, which are now drawn when zoomed in; a route with no
  road or track simply shows no vehicles.

### Added

- Market depth for every good: tap a good on the Markets screen to see how much is made, sold,
  imported and exported each week, the stock held and how many weeks of sales it covers, your
  share of sales over time, the top sellers and buyers with their shares, and in plain words
  why the price is moving this week (buyers short of stock, unsold stock on the shelves,
  reliance on imports, or spare capacity for services).
- Region dossiers: tap a region for its workforce and how many are without a job, wages by
  skill, land price, what households spend their money on, who competes there in each sector
  with their shares, and your divisions there with their profit, with charts over time.
- Division operations: each division shows how hard its plants run, what it made against what
  it sold, and what one lot costs (inputs, labour, upkeep, land, depreciation and freight)
  against its price, with the margin per lot and charts of output, use and cost per lot.
- Freight stats on the company shipping panel: lots moved, cost per lot, who carried them
  (your own carriers, other companies or outside carriers when lanes are full), and your own
  carriers' capacity used and idle, week by week.
- A rivals board: every company side by side (revenue, profit, cash, debt, share price,
  divisions and best market share), sorted by any column. Pick one to chart it against you by
  quarter and see the goods you both sell, head to head.
- Economy history in the economy panel: the corridor share index, the bank rate and inflation,
  the price level, companies operating and gone, raids and household spending, with date ranges.
- Five years of weekly history behind these displays. Saves from earlier versions load and start
  the history from the next week.

- Deeper company financials: every company page and the Stats page have tabs for an overview,
  the income statement, the balance sheet, the cash flow statement, divisions and ratios. Pick
  weekly, quarterly or yearly periods and compare the last several side by side. Ratios cover
  gross and net margin, return on equity and on assets, debt to equity, the current ratio,
  interest cover and revenue growth.
- A cash flow statement for every period: opening cash, operating, investing, financing and tax,
  and closing cash, worked out from the company's own books so it always adds up.
- Division results by quarter: revenue, costs, profit, plants and market share for each division.
- New charts across the app (company pages, Stats, share and goods details, the economy panel): line,
  bar or area, date ranges from four weeks to everything kept, weekly, quarterly or yearly points,
  several series at once with a legend (your revenue against the two largest rivals in your sector
  whose figures you can see, a share against the corridor index, a good against its reference
  price), the value under the pointer or finger, and dates such as 2001 Q3 along the bottom. A
  quarter or year still under way is marked "to date".
- Longer history: ten years of weekly share and goods prices, a year of weekly statements and
  quarterly statements for the whole career. Saves from earlier versions still load and start
  their longer history from the year of prices they already hold.
- Every company on the stock board shows its statements, floated or not, and area charts draw the
  largest series at the back with every line on top, so a balance sheet shows assets, liabilities
  and equity together.

- Seven more map modes: your share, competition, demand, workforce skill, infrastructure (roads,
  power and broadband), land price and population, each with a legend showing what its colours
  mean.
- More map settings: countryside, rivals' buildings, interstates and seasons on or off, and labels
  (off, cities, or cities and regions).
- Interstates are drawn as 8-bit roads with a dark edge, grey asphalt and, up close, a dashed gold
  centre line, over the regions' shading and under traffic and buildings, from the scenario's
  framing inwards.
- Zoomed in, the countryside fills in as a voxel diorama on the real ground: forests (pines in the
  north), farm fields and barns, rocks on dry land, and houses round every city that spread out in
  a boom and pull in during a recession. Trees turn in the autumn and everything wears snow in
  winter.
- Voxel trucks, trains on visible tracks, ships on the lakes and cargo planes carry the freight up
  close; the lanes you use run in your gold livery. Trucks follow the real interstates and trains
  the real railways (Natural Earth, public domain) between the region hubs, not straight lines.
- The map tells the story: a plant still going up shows as a building site with scaffold and a
  crane, a division adding or retooling plants has a crane beside it, a mothballed one goes dark,
  and one you have just taken over flies a gold flag for six weeks. Rivals' sites and closures show
  the same way. In a boom cranes rise beside the cities; in a slowdown or recession their lights dim.
- The real Earth under the map: NASA's Blue Marble imagery (public domain) on the globe, the menu and
  the scenario, sharp close up. The seasons follow the game's calendar: a snow line that creeps south
  in winter, autumn colour, and browner winters in the mid-latitudes, half a year apart in the south.
  Over it your regions lie flat and see-through. Switch it off under Settings, Satellite Earth, for the
  painted map with raised regions. `tools/earth_tex.py` bakes the images.
- Zoomed in, your divisions stand on the map as front-on, cel-shaded voxel buildings, square to
  the map like classic 8-bit games, one model per industry (a farm
  with barn and silos, a mine headframe, a refinery, a chemical works, a factory, a building site,
  a port, a shop, a hospital, an office tower, a studio, a mast, apartments, a base, a bank) in
  three sizes that grow with the division's plants. Cities on the map are voxel city blocks too,
  a town, a city or a metropolis by population, a step smaller so your buildings stand out.
  At the scenario's framing they are small badges instead, and further out nothing marks them.
  Zoomed in, rivals' plants stand there too, smaller and greyed, so you can see who you face where.
  `tools/voxel_sprites.py` bakes them.
- Zoomed in on Android, Windows and Linux, the map's traffic becomes little voxel vehicles doing
  the business: trucks on road lanes, trains of a locomotive and wagons on a thin track on rail
  lanes, cargo ships on the lakes (never over land), cars on the highways and cargo planes with a
  short faint contrail overhead. Each faces the way it is moving, even on a turned map. Busier
  lanes carry more of them, and lanes where you ship or hold capacity run in your gold livery.
  Zoomed out the traffic stays as dots. `tools/voxel_vehicles.py` bakes the sprites.

- Turn the map: twist with two fingers, or drag with the right mouse button on desktop. Buildings
  and labels stay upright, and an N button turns it back to north. Dragging now reaches almost to
  the poles.

### Fixed

- No more pulsing: the rings round buildings, the weekly ripple, the breathing selection outline
  and the menu's pulsing scenario glow are gone; rooftop lights are steady.
- Traffic, planes, clouds and the sun stop when the game is paused and run faster at higher
  speeds, instead of moving on regardless.
- Freight and cars keep a steady pace when you pan or zoom the map; they no longer jump or speed
  up as the camera moves.
- Cars and freight run under buildings, and buildings and labels stand above the clouds, instead
  of traffic being drawn over everything.
- On iPhone and iPad the pause banner no longer jumps or resizes, answering a decision card no
  longer tears its sheet down mid-close, and Continue resumes time at the speed it was running.
- On iPhone and iPad the app no longer stalls at the top speed: the game runs off the interface
  thread and the screen refreshes at most a few times a second at high speed. Every tab now has
  the pause and speed buttons, not only the map, and so does every tab on Android phones.

### Added

- Android adaptive launcher icon with a themed (monochrome) layer, so Android 8 and later no longer
  shows the icon in a white circle.
- Google Play listing assets: the 512 pixel icon, a 1024 by 500 feature graphic built from the
  game's own menu and globe, and draft listing copy for review (`store/play/`).
- Store declaration drafts: one table of what the app sends, mapped onto App Store App Privacy and
  Google Play Data safety and app content answers (`store/declarations.md`); the privacy policy
  draft now covers the desktop and direct-download update check and where each platform keeps the
  analytics switch.
- App Store screenshots at the sizes Connect accepts, rendered from a real career: iPhone 6.9-inch (1320 x 2868) and 13-inch iPad (2064 x 2752), flattened to RGB because a screenshot with an alpha channel is rejected. The Apple smoke workflow collects them as the `store-screenshots` artifact and the test asserts the PNG carries no alpha.

- The map is now 3D. As you zoom in it tilts away into the distance, and the scenario's regions
  stand up as lit tiles whose height shows the layer you are looking at, rising and falling as you
  switch layers. The globe is lit by the sun, with an atmosphere, a glint on the ocean, land
  coloured by climate, and a night side that fades in smoothly with cities glowing in the dark.
  Your divisions stand as buildings with lit windows and their plant counts, big cities get
  skylines with blinking rooftop lights, freight moves along the lanes, cars run the highways up
  close, planes cross between the big cities trailing contrails, shallow water glows along the
  coasts, the selected region glows, and a new week sends a ring out from every plant. A 3D button
  flattens the map again.
- Real clouds: the map and the menu globe carry a real satellite cloud map of the whole Earth,
  with its fronts, storms and tropical clusters, drifting west on the trade winds and east on the
  westerlies. Each scenario has its own sharp close-up clouds, and clouds clear as you zoom in.
  Clouds can be off, thin, light or heavy (heavy clouds cast shadows), set in Settings or in one
  tap with the new cloud button on the map. Cloud imagery:
  Solar System Scope (CC BY 4.0), based on NASA Blue Marble.
- A sharper 8-bit look: stepped corners, hard bevelled edges and hard shadows on panels and
  buttons, and a pixel font (Silkscreen, SIL Open Font License) for headings, labels, buttons and
  tabs. Body text and figures stay in the regular font so they are easy to read. No glass or blur.
  On iPhone, iPad and Mac the app no longer uses iOS 26's Liquid Glass: system bars, buttons and
  sheets are solid and gold-accented to match.
- iPad Split View and Slide Over are checked at a third, a half and two thirds of the screen in
  both orientations; the week controls stay pinned on screen at every width, where at half width
  Next week used to scroll out of reach. Snapshots are kept in `evidence/ipad-multitasking/`.
- The main menu's globe now turns in sunlight with its night side lit by cities, gold trade
  routes arc between the world's great cities, satellites circle it, the sun glares from the
  side, and a band of light crosses the logo.
- Graphics settings: Low, Medium, High and Ultra presets, and separate choices for map detail,
  3D tilt, raised regions, lighting, day and night, city lights, traffic, buildings,
  frame rate (30, 60 or unlimited), animations and an on-screen frame counter. Low looks and costs
  the same as before. Moving effects only redraw what moves, at the chosen frame rate, and stop
  when the map is off screen.
- Google Play can take a release: the release workflow builds a signed Android App Bundle, verifies its signature, manifest and arm64 library, and attaches it to the release. The APK stays for direct downloads.
- Notes for Review for the first App Store submission: the offline game, the anonymous market snapshot and its switch, and the absence of purchases.

### Fixed

- Android back now returns one level at a time, closing settings, overlays, the new-game steps,
  the load and scenario editor screens, and stepping back through companies opened from a company
  page, instead of leaving the app. From another tab it returns to the map first.
- On Android 15 and later, the menu, settings, new-game, load, scenario editor and tablet screens
  keep clear of the status and navigation bars.
- A build installed from Google Play no longer offers the GitHub self-updater, because Play requires updates to come through Play. The direct-download APK still updates itself.

### Changed

- The requirement ledger and release evidence now record every uploaded TestFlight build from 5 to 15 in `evidence/releases/testflight-builds-5-15.md`.

## [1.7.1] - 2026-10-28

Fixes and depth from the first week of Lone Star: no instant raids on new players, no stuck pauses, plant crews and research that no longer bleed a new company, every market trading with smooth prices, company pages with logos and a face for every CEO, division detail, readable freight, raid buttons and visible rival personalities, a CFO who says why you are losing money, faster weeks, scenario dates, and every screen on iPhone, iPad and Android. Saves from 1.7.0 do not load in 1.7.1.

### Added

- Phones and iPad audited against every 1.7.1 desktop screen (stuck-pause banner and pause settings, company pages, division detail, freight sections and lane panel, CFO advisor, Rivals card, raid panel and rival tags, scenario dates, stock target label, tutorial research step). Two gaps closed: on your own company's page, tapping a division opens its detail on iPhone, iPad and Android; and on iPhone and iPad a region board now lists "Your divisions here", each opening its detail, as Android and desktop already did.

- Company pages. Tap any company in the stock list, the stock dialog, a rival's dossier, a region board or a news item to open its page. Listed companies show the share price chart, valuation, a financial summary, divisions and plants by region, holders, subsidiaries and joint ventures, recent moves, grudges and friends, news and Buy and Sell. Private companies show only what is public: name, logo, chief executive, sector, regions, size band and public moves. Your own companies show everything.
- Every company has a logo, drawn in 8-bit pixels from its name from a fixed set of plain shapes (stars, arrows, wheat, drops, gears, towers, waves, monograms), the same on every platform. A tool refuses any symbol that looks like a banned shape.
- Every company has a chief executive with a face. Firms outside the twelve-person cast get a generated, fictional chief with a portrait, name, temperament and background, shown in the stock dialog.

- Division detail. Tapping a division on the map, in the region board's "Your divisions here",
  in the CEO Office or on the company page opens a detail view with its weekly financials
  (revenue, each cost line, operating profit, margin, a revenue trend, capital invested and
  book value), operating strategy (what the recipe does, pricing posture, transfer price,
  shipping and stock target), inputs (weekly need, source, price, stock and weeks of cover),
  outputs including by-products (made, sold, stored, price and buyers), plants (status,
  utilisation, age, staffing) and labour (workers by tier, wage against the region, payroll,
  union state, shortages). Pricing, transfer price, stock target, shipping and wage offers can
  be changed from it. Results are estimated from the plants' last week and your closed sales at
  today's asking prices. The report is read only and does not change world state or hashes.

- Raid from where you see a rival. Every rival row (region board, map region sheet, dossier and
  stock page) now carries a Raid button that opens the engine's quote: success chance, plants at
  stake, price, the rival's defence, any repeat-raid surcharge and a budget slider. When a raid is
  not open the panel says why in one line and, when a plant of yours in that market would fix it,
  offers a Build button. Compose gets the raid panel for the first time.
- Rivals show their personalities. Rival rows carry the chief executive's portrait and name, a
  temperament tag and grudge or ally marks. The inbox lists rival moves and names the chief and
  temperament behind each move and raid. A compact Rivals card in the CEO Office lists the groups
  with a grudge against you, your allies and who you can raid now with the quoted chance.

- Fair opening: rivals can no longer raid a new company or a newly opened division for its
  first 26 weeks. In Texas Oil Patch 1981 (and the other scenarios) the player's divisions were
  raided in the first weeks, before any marketing or defence could exist, and the game paused.
  Later raids are unchanged.
- The CFO. When a company has lost money over the last four weeks, the CEO Office now shows up
  to three reasons, biggest first, in plain English with what each costs a week and one button
  that fixes it: pause research, stop marketing, stop training, stop quality spending, price a
  good above what it costs to make, or let the plants plan production again. A company that is
  making money sees nothing.

### Changed

- Freight you can read at a glance (1.7.1). Every division and the CEO Office now have a
  Shipping section in plain words: what the freight bill is each week and per load, how much of
  it went to your own carriers, other firms' carriers and outside carriers (with the premium the
  outside carriers cost you), the three lanes you use most and whether they are congested, your
  route policy in a sentence such as "Cheapest route: mostly truck, mainly Columbus to Chicago",
  and one suggestion with a quoted payback when it matters: build carrier plants on a lane where
  outside carriers take a lot of your money, or reserve loads on a busy lane. The route policy
  picker explains each choice in a line and shows what this division's last week of freight would
  have cost under it, before you pick. On the map, the Freight layer has a legend, lanes you own
  or ship on carry a gold edge, and tapping a lane opens a simple panel with its room, price,
  owners, your share and a reservation with its quote (price, weekly cost, expected saving).
  Freight alerts now say what is happening on the lane in figures and what to do about it. The
  screens call a lot of goods a load. A build suggestion has a Review this build button that
  opens the build dialog already filled in with the region, plant type and number of plants.
  Summaries are worked out by the engine; they add no mechanics. The Shipping figures are now
  kept in the save file, so they are still there after you load a game instead of staying empty
  until the next week closes (older saves load as before and fill in at the next week).
- Phone parity with the desktop app (1.7.1). The phone layouts now carry the desktop panels that
  were missing. On Android phones: the map shows decisions waiting, a pause and engine warnings,
  its layer chips scroll sideways instead of running off the screen, and a region's sheet scrolls
  and lists the rival leaders there (tap one for its dossier); the Inbox lists recent raids as
  Rival moves; the Portfolio lists your shareholdings with stake, value and the quarter's move;
  the CEO Office shows four-weekly results and marks a retooling division; Stats shows each
  holder's share of the votes. On iPhone and iPad: the Portfolio lists your shareholdings, the
  CEO Office shows four-weekly results, Stats shows each holder's share of the votes, and the
  Structure panel gains past activist campaigns, carve-out, activist win and internal-pricing tax
  totals, and sector names with a reputation bar.

- Every pause now says why and how to continue. A banner at the time controls on every screen,
  in the Compose and Apple apps, names the pause in plain words with one button: Answer opens
  the exact card, Continue resumes for everything else.
- A setting for which events stop the clock. By default time stops only for decisions that need
  an answer (cards, board votes and deal deadlines close at hand, the five-year score). Turn on
  Stop for warnings too to also stop for payment grace running out, a first strike or a rejected
  union counter.

### Fixed

- Market snapshots no longer skip when time runs fast. A snapshot was only sent when the week landed exactly on a multiple of five, so a run that stepped over one sent nothing. iPhone, iPad, Android, Windows and Linux now send when an advance crosses a five-week boundary.

- Every market trades and prices move smoothly. Goods that sat at a round placeholder price with
  nothing made or sold (maintenance, research and, in some scenarios, fertilizer, ore, rare
  earths, timber, vehicles, pharmaceuticals, building materials and natural gas liquids) now have
  a producer in every scenario, a new Plant Maintenance Services strategy supplies maintenance, and
  the Markets board no longer lists goods nothing makes or uses (training, water). Prices were
  swinging up and down every week or two because rivals changed prices only every fourth week on
  two rules that fought each other; every firm now sets its price every week from its stock cover
  and sales, a sixth of the way at a time, so prices follow supply and demand and settle. A
  corridor price no longer jumps when a small seller drops out for a week. Services no longer
  produce a third more than they sell, and no longer sit throttled below demand. A plant run down
  to its floor keeps one worker of each tier instead of idling for good, and rivals no longer
  retool the only plant making a good or reset every plant to 85 percent each review, which made
  their output swing against the weekly plan. Coastal Generation now opens in Los Angeles, where it has
  customers. This changes the world state of every scenario, so saves and replays from 1.7.0
  do not carry over.
- A division's stock target slider read "Company default" beside "26 weeks of inputs", which was
  only the slider's maximum. It now shows the real company default, such as "Company default: 4
  weeks", and notes that perishable inputs keep at most 2. The tutorial's research step told new
  players to fund research with a budget that lost them money. It now asks for a modest budget and
  shows one sized to your company: its cost a week, the weeks to the next level and what that
  level buys. Display only, no change to world state or hashes.

- Market snapshots now arrive from Android, Windows and Linux. Until now only the iPhone build sent
  its anonymous market-balance snapshot; the Compose app had no sender. It now posts one every five
  game weeks in the background, with a random install id and nothing else about you, and fails
  silently. A new Analytics switch in Settings turns it off. The iPhone build already sends the
  current snapshot format, since the snapshot comes straight from the engine.
- A new plant no longer pays a full crew for the whole build. Crews are hired in the last four
  weeks before a plant opens, so it opens staffed without half a year of idle wages. A hardware
  plant in Silicon Valley cost about $11,000 a week for 24 weeks before making anything.
- Research no longer burns cash at the technology frontier. A company that already has the best
  level it is allowed to buy stops spending its research budget until the frontier moves.
- Research is cheaper to start: the first levels cost $40,000, $160,000 and $360,000 instead of
  $100,000, $400,000 and $900,000.
- Faster weeks and a more responsive interface. A game week now takes about 20 to 40 percent
  less time in every scenario (Ruhr and Rhine 18.8 to 11.6 ms, Silicon Valley 12.8 to 9.4 ms,
  Texas 13.0 to 10.7 ms, Great Lakes 9.4 to 7.3 ms on the test machine) with identical results:
  freight route shapes are kept from week to week until the network changes, and rival
  expansion scoring reads each market once. Pixel icons, sector badges, portraits and the logo
  are drawn from a bitmap built once instead of hundreds of rectangles a frame, and the map
  keeps its projected shapes until the view moves, on Android, Windows, Linux and Apple.

- Every scenario now keeps its own calendar. The status bar, elections, chair terms, seasons,
  price paths, news, saves and chart axes read from the scenario start year, so Texas 1981 no
  longer shows "2000 W05" on both the Android and iPhone clients.
- Seed firms open at a fair share price. Their first quarter of earnings is unobserved, and the
  opening valuation assumed far more than the firms really earn, so shares such as Gulf Broadcast
  Network sat at several times fair value and slid for weeks. At week 5 the median price over fair
  value is now about 1.0 in every scenario (was up to 1.6), and firms priced over twice fair value
  fell from 17 to 20 of about 50 down to 2 to 8. Saved worlds from 1.7.0 are unaffected.
- Fictional firm and cast names no longer echo real companies or people (for example
  Anadarko Gas Partners, Lone Star Rail Freight, Bluebonnet Grocers, Golden State Power,
  Texoma Power and Light and Joel Rosenthal).

- Market snapshots now arrive from Android, Windows and Linux. Until now only the iPhone build sent
  its anonymous market-balance snapshot; the Compose app had no sender. It now posts one every five
  game weeks in the background, with a random install id and nothing else about you, and fails
  silently. A new Analytics switch in Settings turns it off. The iPhone build already sends the
  current snapshot format, since the snapshot comes straight from the engine.

- No more stuck pauses. The clock could stay stopped with no way to continue: a card that was
  answered or no longer applied (a union that settled, a strike that ended, a covenant that was
  cured, a review that cleared, a bid that closed) kept the pause, a pause could name a card the
  Inbox did not show, and a rival's card could stop the time bar. Cards that no longer describe the
  world now retire on their own, a pause that can no longer be answered clears itself, rivals'
  cards stay out of your Inbox and alerts, and the time bar only stops when the engine says so.
  Answering a card always clears its pause.

## [1.7.0] - 2026-10-27

Lone Star: supply chains you can own and contest, a shipping network, deeper corporate play, rivals with names and memories, and three new playable scenarios: Texas Oil Patch 1981, Silicon Valley 1995 and Ruhr and Rhine 1991 with currencies. Saves from 1.6 do not load in 1.7.0.

### Added

- Rival personalities (1.7 preview 5). Each rival now plays the way its chief executive's
  temperament says, across building, raids, bids, freight, storage, speculation, bonds, listings
  and joint ventures. Integrators buy up their suppliers and build their own stores, raiders
  raid, take lanes and spend more on marketing, financiers bid, float and borrow, speculators
  stockpile ahead of swings, builders build and the cautious sit on a bigger cash cushion and
  rarely start fights. Rivals without a named chief play their archetype's habit. A rival you
  raid, bid for or squeeze out of a freight lane holds a grudge and comes after you, and
  grudges fade over about a year. Joint venture partners and the two sides of a friendly deal
  become allies: allies are never raided or bid for and are the first asked to share a project.
  A rival's dossier now lists its grudges, its friends and the moves it has made lately.
  Difficulty changes only how sharp and how aggressive rivals are. Rivals with a speculator or
  integrator temperament now build their own warehouses, tank farms, silos and cold stores
  when the fees, spoilage or an expected price swing pay for one. The market snapshot gains
  per-temperament move counts and grudges and alliances formed.

- Currencies and exchange rates (1.7 preview 7), a mechanic any scenario can use. A scenario may
  have one country or several, each with its own money and central bank. Every firm keeps its
  books in the money of its home region, and each week the rates move with the countries'
  relative inflation, their central bank rates and a random shock. A currency can be pegged to
  another inside a band, like the 1991 exchange rate mechanism, and a strained peg can break:
  the band widens, as in 1992 and 1993 when the scenario follows its historical path. Sales
  paid in another currency are collected two weeks later at the rate of the day, and the
  difference is booked as a foreign exchange gain or loss. Gateway prices and wages follow each
  country's money. Contracts, loans, deposits, insurance, joint ventures and mergers stay inside
  one currency, a firm builds only in regions that use its money, and only firms keeping books in
  the lead currency hold stakes in other firms, which the exchange quotes in that currency. You
  see net worth and cash in a reporting currency you choose, company figures carry their own
  currency where it differs, and the economy card gains a foreign exchange section. Scenarios
  with one money are unchanged: their saves and world hashes are identical.
- Ruhr and Rhine, 1991 is playable (1.7 preview 7), the first scenario with more than one
  currency. Eighteen regions across Germany (the Ruhr, Cologne and Bonn, Hesse, the Palatinate,
  the Saar, Hamburg, Lower Saxony, Stuttgart, Munich and the new eastern states), the Netherlands
  (Rotterdam, Amsterdam, Limburg), Belgium (Antwerp, Liege), Luxembourg and France (Lorraine,
  Alsace), with the Deutsche Mark, guilder, Belgian and Luxembourg franc and French franc, the
  last three pegged to the mark. Rhine barge, rail, road, the Rotterdam and Antwerp pipelines to
  the Ruhr and North Sea gateways connect them. Fifty-five rival firms cover coal, steel,
  chemicals, autos, engineering, utilities, banks and logistics, and world prices follow coal and
  steel in decline, the reunification boom and the 1992 to 1993 recession. All regional numbers
  are game estimates built on NUTS areas, not statistics. You start with a steel mill in the
  Ruhr. The map draws on the globe and the corridor map on Android, desktop and Apple.
- A character cast (1.7 preview 5). Twelve fictional people, each with a pixel portrait, an age,
  a home town, a background and a temperament, now fill the named posts in the game. The central
  bank chair is always one of them and the chair's leaning toward high or low rates comes from
  who they are. The two party leaders, three state governors and the rival chief executives are
  drawn from the same twelve. Each scenario prefers its own people: Texas Oil Patch, 1981 starts
  with the Texas and Gulf crowd. Rival firms have a named chief executive with a temperament, and
  a rival that loses money for six four-week periods in a row can replace its chief executive.
  The new chief is chosen for a different approach, the old one sits out for two years, and the
  change is reported in the news. Open a rival's stock page, or a region board, to read its
  dossier: portrait, name, temperament, background and recent moves. The economy card shows the
  chair, the leaders and the governors with portraits, and news items about a person show theirs.
  A chief executive's temperament now steers how the rival uses bond borrowing, transfer pricing,
  listings, joint ventures and activist defence, and an activist campaign that forces out the
  chief executive replaces the named person too. Market snapshots gain a count of chief executive
  turnovers. Great Lakes worlds play differently
  from before because posts are now drawn from the cast; saves from earlier previews load with
  unnamed rival management.
- Silicon Valley, 1995 is playable (1.7 preview 7). Sixteen economic regions built from whole
  California counties (San Jose and Silicon Valley, San Francisco, Oakland and the East Bay, San
  Mateo, Marin and the North Bay, Sacramento, Stockton and Modesto, Fresno, Bakersfield, Los
  Angeles, Orange County, the Inland Empire, San Diego, Monterey, Santa Barbara and Ventura, and
  Redding), sized from the July 1995 Census estimates. Fifty-six rival firms cover chips and
  hardware, software, telecoms, defence and aerospace, studios, farms, banks and venture-style
  financiers, retail and construction, and the Bakersfield pipeline to Los Angeles and Oakland
  exist on day one. Five ports (Oakland, Los Angeles, San Diego, Stockton, Santa Barbara) share a
  Pacific sea system with gateway legs to Asia and the Pacific and the Panama route. Wages are a
  little lower than 2000, oil and gas are cheaper and chips dearer, and world prices follow a
  1995 to 2005 path with variance by default: the memory chip slide, the 2000 spike and the
  2001 to 2002 tech bust, with oil and grain moving modestly. You start with a chip and hardware
  maker in San Jose. The California map draws on the globe and the corridor map on Android,
  desktop and Apple; the Great Lakes and Texas map layers are byte for byte unchanged. Price
  families can now belong to one scenario only (the chips and electronics family), so older
  scenarios play exactly as before.
- Texas Oil Patch, 1981 is playable (1.7 preview 6). It has its own map and its own start:
  twenty-two economic regions built from whole counties across Texas, Oklahoma and Louisiana
  (Houston, Beaumont and Port Arthur, Dallas and Fort Worth, San Antonio, Austin, the Permian
  Basin, Corpus Christi, the Panhandle, El Paso, East Texas, the Rio Grande Valley, Lubbock,
  Abilene, Oklahoma City, Tulsa, Cushing, western Oklahoma, New Orleans, Baton Rouge, Lake
  Charles, Shreveport and Lafayette), sized from the 1980 census. Forty-five rival firms cover
  the oil chain (independent drillers, refiners, petrochemicals, savings and loans lending against
  rigs, farms and cattle, builders, carriers) and the pipelines from the Permian and Cushing to
  the Gulf Coast and across Louisiana already exist on day one. Six Gulf ports are linked by sea
  and by gateway legs to the Atlantic, the Caribbean and the Mississippi. Wages are lower and oil,
  gas, fuel, power and farm output are dearer than in 2000, and world prices follow the Texas
  1981 path with variance by default. You start with oil wells in the Permian. The Texas map
  draws on the globe and the corridor map on Android, desktop and Apple. Every scenario now
  picks its own regions, lanes, firms and starting balance from `content/scenarios/<id>/`;
  Great Lakes 2000 plays exactly as before (its saved games and replays are unchanged).
- Government and systemic events (1.7 preview 4). The party in power now also sets five
  market laws. The Workers' Party brings common carrier rules (owned pipelines, rail and ships
  sell spare capacity at a capped rate and may not lock a lane up with contracts), review of
  deals that buy a supplier or a customer once their market share reaches 35 percent, a
  strategic reserve (the government buys oil, ore and other raw goods in busts and sells in
  spikes) and nationalisation of failing strategic firms. The Commerce Party brings production
  quotas in extraction states (output held to 80 percent of capacity while a price is soft),
  bailout loans for failing strategic firms and privatisation of state firms once they earn.
  State firms price at cost plus five percent. Pipeline spills, port strikes, refinery fires,
  lane outages and harvest failures now hit the network with real probabilities, durations and
  effects on lanes, plants and stock. Rivals build pipelines on congested liquid and gas lanes
  and react to quotas, common carrier rules and the reserve. The freight congestion card now
  fires from lane fill data. The economy card lists laws in force, the reserve, quotas, state
  firms and recent disruptions; alerts appear only when a law or incident touches your
  companies. State rescues are rare: only large, operating-profitable strategic firms get one,
  and the reserve also sells when prices rise 10 percent over what it paid or in a supply shock.
  Telemetry snapshots (schema 4) carry law changes, reserve, rescue and incident
  counts.
- Corporate structure and integration (1.7 preview 4). Companies in one group now supply each other
  first: the engine keeps standing contracts between divisions that burn what another makes, ahead
  of any open-market sale. Each supplying division has one transfer price setting, from its cost
  through the market price (the default) to a markup. Pricing under market in a part-owned company
  takes value from its minority shareholders and can draw a challenge; profit moves between states'
  tax bills; a group supplier that keeps losing money is marked down by the market.
- Each division now has a reputation, built by quality spending and marketing and lost slowly,
  that earns a price premium and loyal customers in every market. Raids cost reputation as well as
  plants.
- Corporate bonds are priced off the central bank's ten-year yield plus a credit spread from
  leverage, coverage and size, with no grade gate. Buy and sell them on a bond list at the market
  price.
- Carve out a division into a subsidiary, list part of a subsidiary on the exchange keeping
  control, or spin it off to the parent's shareholders. Two groups can co-own a division as a
  joint venture with stated stakes, split its profit, and buy out or leave after 26 weeks.
- Activist investors press companies that fall far from their peak, miss board goals twice, stop a
  dividend or squeeze minority holders. Concede, negotiate or fight; if you do nothing the board
  answers after four weeks. Alerts appear only when a decision is needed.
- Rival firms borrow in the bond market, keep transfer prices honest toward minority holders (or
  tilt them toward the lower-taxed state), float subsidiaries when short of cash and share growth
  projects in joint ventures. A temperament table is the hook for the character AI.
- Market telemetry schema 4 reports groups, internal pricing, reputation, bonds, ventures,
  listings, spin-offs, activists and construction load. A Structure section in Markets on every
  platform and Command Center actions for every new verb.

- Owned logistics (1.7 preview 3). Freight is now lane capacity. Trucking, rail, pipelines and
  lake and sea shipping carry goods between regions; each mode has its own price, speed, smallest
  load, terminal needs and list of goods it may carry (pipelines liquids and gas only, cold goods
  by truck or rail). Carrier divisions give your group finite weekly capacity on the lanes they
  serve, and you ship on your own carriers first and at cost. Other shippers pay a price that
  rises smoothly as the lane fills; outside carriers take the overflow at a premium so goods
  never stop. Rail terminals and ports are buildable and open their region to every firm. Each
  division has a standing shipping policy (cheapest, fastest, own carriers first, most reliable,
  with an optional preferred mode or carrier) and the engine routes every shipment over up to
  three legs. Lane contracts reserve capacity for 4 to 52 weeks at a locked rate, so you can lock
  rivals off a lane you depend on. Great Lakes ports get lake shipping, and the data supports
  gateway sea legs for later scenarios. Alerts appear only for lanes you ship on: full for four
  weeks, a carrier rate hike, a rival reserving a lane. Rivals choose policies, build carriers
  where lanes overflow and sign lane contracts. Map freight layer with congestion colours, a lane
  panel, a shipping policy picker per division and Reserve lane in the Command Center, on
  Compose and SwiftUI. Snapshot schema 3 adds lane use, rates and the outside carrier share.

- Storage (1.7 preview 2). Goods now take up space. Every plant has a working yard, six weeks of
  what it uses and makes; anything beyond that sits in the region's outside storage for a weekly
  fee per lot, so goods never vanish for lack of room but a pile costs money. Four new logistics
  strategies you can build, with capex, a crew and a build time like any plant: Warehouse, Tank
  Farm (liquids and gas), Grain Silo (grain and bulk) and Cold Store (food, livestock and
  pharmaceuticals). Other firms' overflow fees are paid to storage owners with spare room.
  Perishables spoil at a fifth of the rate inside your own cold store or silo. Stock beyond the yard
  also pays a small holding cost. Each division has a stock target, one slider for weeks of
  inputs to keep, and grain is bought ahead in the harvest quarter by default, so the harvest
  glut no longer forces a fire sale. Buy to store lets you (and rivals) buy a good on the
  exchange into storage and release it later; the Markets good card has a Store section, and
  the Command Center has Stock target, Buy to store and Sell from store. Rivals run a simple
  version: they buy cheap, steadying world-price goods in a downturn into space they already
  have, and sell when prices are high or the cycle booms. The region board and each division
  show storage used against capacity. Alerts appear only when storage is nearly full, outside
  fees are a real cost, or spoilage is losing real money. Market snapshots now report storage
  use, outside fees, holding costs, spoilage and held goods.
- World commodity price paths, in every scenario. Oil, natural gas, coal, ore, steel, grain,
  fertilizer, chemicals, timber, rare earths and building materials now have an outside-world
  price that moves every week and sets what gateways charge to import and pay to export. The
  new-game screen offers three paths on every platform: Historical (the real shape, on the real
  dates), Historical with variance (the same shape with the timing and depth of each swing
  shifted by your game's seed), and Random (neutral prices that drift around where they began;
  busts come only from the business cycle). Each scenario sets a default. Great Lakes 2000 has
  its 2000 to 2008 oil and metals climb and the 2008 collapse; Texas 1981 has oil falling from
  its peak to the 1986 collapse. A collapse in world prices feeds the business cycle as a demand
  shock, and the cycle tilts prices a little in return, without counting the same swing twice.
  The Markets economy card lists each price family against day one and its change over the last
  quarter. The editor's price path field now takes effect. Price data is in
  `content/price-paths.json`, keyed by product family, so new products join a family with one line.
- Deeper product chains (1.7 preview 2). Plants can now make several products at once in a fixed
  split, and nothing a plant makes is left without a price or a buyer. Oil is a real chain:
  wells pump crude oil, and a refinery turns it into fuel, gasoline, jet fuel and petrochemical
  feedstock. Gas processing strips natural gas liquids from natural gas. Iron is a chain too: a
  blast furnace makes pig iron (its slag is sold as building materials), a basic oxygen mill
  makes steel from pig iron and scrap, and an electric arc furnace melts scrap alone; scrap comes
  out of steel works, component stamping and vehicle assembly. Mixed farms grow grain and raise
  livestock, and food processing turns both into food. Nine new goods: crude oil, gasoline, jet
  fuel, petrochemical feedstock, natural gas liquids, pig iron, steel scrap, grain and livestock.
  Seven new plant strategies, each with realistic circa 1980 to 2000 yields, and the build
  screens list a plant's by-products.
- Rivals plan and build the new chains with the same rules as you: a plant is valued on
  everything it makes, and a refinery counts as a good site when the corridor imports its fuel,
  gasoline, jet fuel or feedstock.
- Food, groceries, grain and livestock now spoil a few percent of unsold stock each week, and
  plants buy no more than two weeks of a perishable input.
- Scenario editor, on every platform. Take any built-in scenario and change its numbers:
  starting capital, difficulty, the opening economy, which rival firms start and how big and
  cash-rich they are, wage and demand levels in each region, and the laws in force in week one
  (corporate tax, wage floor, merger review, unions) and the world price path (historical,
  historical with variance, or random).
  Save, save as, delete, and start a game straight from the editor. Saved scenarios appear in
  the new-game picker beside the built-in ones, and every built-in
  scenario card has a Customize button. The editor is on the main menu everywhere.
- Custom scenarios are stored as versioned JSON files in the game's data folder, checked on
  save with plain-language messages and sane limits (at least three rivals must start).
  A save made from a custom scenario keeps its rivals and regional levels when it is loaded.
- A custom 8-bit icon set used everywhere. Navigation, the sidebar and status bar, menus, save,
  load, settings, search, quit, play, pause and the three speeds, next week, map zoom and globe
  buttons, every Command Center category, alerts, buy and sell, movement arrows, close and back
  are now hand-drawn pixel icons, the same pixels on Android, Windows, Linux, iPhone, iPad and
  Mac. They replace the stock Material and SF Symbols icons. New tooling checks that every icon
  the apps use exists and that every grid is 16 by 16.
- iPhone, iPad and Mac reach the 1.6 feature set:
  - A Command Center with every action in the game, searchable by category, each with a form
    built from the engine's list (pickers, sliders with an exact amount box, switches), a live
    preview and Submit. It opens from the CEO Office on iPhone and from the sidebar or the search
    box on iPad and Mac.
  - A Research screen with the technology ladder, progress, what the next level buys and research
    and marketing budgets on sliders.
  - An economy card on Markets: a fold-out panel on iPhone, a full column on iPad and Mac.
  - A new-game flow with scenario cards and art, coming-soon scenarios, difficulty, starting
    economy and starting capital, and a founding screen for starting with no company.
  - A main menu with a turning globe where each scenario's region glows and is named, and the
    sector icons flying in under the logo.
  - A tutorial of eight steps that tick off as you do them, each with a Show me button, docked on
    iPad and Mac and a slim bar on iPhone. Settings can restart it.
  - A load game list with Load and a confirmed Delete on each save.
  - On iPad and Mac, a desktop layout: a sidebar for every screen, a status bar with cash, profit,
    cash outlook, share price, net worth, the economy and the time controls, and multi-pane screens
    (map outliner, CEO Office beside results, economy beside stocks and commodities). Command with
    1 to 8 switches screens, Command K opens the Command Center, Command N plays a week and
    Command P plays or pauses.

### Changed

- Raids no longer have a thirteen-week cooldown: repeat raids on the same rival cost forty percent
  of the budget more for each one still on the record. A raider can take a whole division, but a
  bigger stake is harder to win; the defender no longer keeps a guaranteed plant.
- The forty-plant order cap is gone. Past a firm's construction capacity (forty plants plus a tenth
  of what it runs) building costs more and takes longer, smoothly.
- Bonds are no longer limited to grade A and B firms or priced by a coupon you type; the field
  now sets the most you will pay, and zero takes the market price.

- Plastics now use petrochemical feedstock and natural gas liquids, and the chemical plant uses
  feedstock beside fuel. Oil wells make crude oil, not fuel; fuel now comes from refineries.
- Automatic prices: a good sells out and rises only while the corridor is still importing it,
  and eases back toward the reference price when local supply already covers demand, instead of
  sitting at the ceiling. The ceiling is 95 percent of the cheapest import, so local goods win
  once normal freight is added.
- Gas, timber, pharmaceuticals, rare earths and building materials start with local producers,
  and so do the new chains: several rival firms open with an extra plant in another region.
  Starting saves and the cross-platform test hash change with the new content; saves from earlier
  versions do not load.
- The data format moved to schema 2 (by-products and spoilage). Schema 1 files still read.

- Sprawl overhead no longer jumps at 15 divisions. It grows smoothly with division count, and
  owning logistics divisions takes part of it off on top of logistics strength.
- Freight capacity per block, the freight reference price and the two seeded carriers were
  recalibrated so a carrier earns its keep from lane use. Crossroads Carriers and Oaken Freight
  Lines now survive nearly every seed instead of failing in most.

### Fixed

- Build forms, the Command Center and the market list quoted Great Lakes reference prices in every
  scenario, so a Texas or California career saw the wrong margins and prices. The catalogues now
  come from the career's own scenario on Android, desktop and Apple, and reload when a different
  career starts.
- Region boards showed no population in scenarios sized from a census other than 2000; they now
  show the scenario's own count, and no longer label it 2000.
- Rival oil firms now build refineries when the corridor imports fuel. The build test counted
  gasoline and jet fuel, which the corridor never imports, as missing room, so no rival ever
  built one while fuel imports ran at six times local output. Only the main product decides
  whether there is room; the by-products still count toward whether the plant pays. Over three
  years fuel imports fall from 86 to 72 percent of supply, and to about 61 percent at five years.
- Perishables spoil far less in your own cold store or silo: the loss falls by 80 percent for
  the part of the stock that fits in the space you own, up from half. The large food stock in
  the corridor is mostly the grocers' two weeks of input cover, which rots at the full rate
  without a cold store.

- A company's share value no longer jumps when another company buys into it. The valuation
  added the buyer's whole equity to the company being bought, so a rival taking half of your
  company multiplied its fair value per share about six times with cash and income unchanged,
  and selling shares into that spike produced false profits. The buyer's value now gains its
  stake in the company it holds, which is how look-through value was meant to work. In a
  96-seed growth-policy check, runs ending far above the normal range from this spike fell
  from 22 of 96 to 1 of 96 (that one is a genuine profit boom).

- Freight and advertising were made but never sold, so their plants were cut back to the
  minimum. Freight bills, advertising purchases and service inputs (software, telecom, research,
  maintenance, freight) are now paid to the firms that hold that service this week, in
  proportion to the stock they hold, and only the remainder goes to the outside sector.
- Fuel the corridor made while it imported fuel: asking the full import price put the delivered
  price above imports. Local sellers now stay under the import price.
- Seed supply contracts delivered from a firm's headquarters region even when the plant sat
  elsewhere, which could strip a plant's own inputs; they now deliver from the plant's region.
- Stock of food sat unsold in storage at twelve thousand lots; spoilage and shorter perishable
  cover bring it down.

## [1.6.1] - 2026-10-27

Android and desktop fixes from the first tester reports.

### Fixed

- The CEO Office crashed the moment it opened on phones when you had not founded a company yet:
  the founding screen tried to scroll inside the Office's own scrolling list.
- A large empty band above the map's top bar on phones: the map drew over the tutorial bar
  above it. The map now stays inside its own area and the phone tutorial bar is one slim row.
- The map's top bar showed a blank line instead of "No company yet" before you found one.
- The Office tab label wrapped onto two lines on phones; it now reads Office.
- Region cards on the founding screen sit two to a row on phones instead of one.

## [1.6.0] - 2026-10-27

A real economy and a real game shell: business cycles, a central bank with rotating chairs,
governments and laws; a globe main menu, scenario picker, difficulty and starting economy; a
Command Center for every action; research and economy screens; a desktop layout; fine-grained
sliders; load game; a tutorial; and in-game updates. Saves from 1.5.0 do not load in 1.6.0.

### Added

- A main menu on every platform. The game opens on it instead of jumping into the last save:
  Continue, New game with a scenario and a mode (guided, sandbox or five-year score), Settings
  and, on desktop, Quit.
- Exit to the main menu from a running career. The in-game menu (the map HUD on iPhone and Mac,
  the Menu tab on Android and desktop) saves, opens Settings or saves and returns to the menu.
- A full Settings page, also from the main menu: graphics (map detail low, medium or high,
  animations, and interface size on Android and desktop) and, on desktop, window mode
  (windowed, maximized, full screen) and resolution.

- A desktop layout for wide windows (Windows, Linux and large tablets) instead of the stretched
  phone screens: a sidebar for every screen, a status bar with cash, profit, cash outlook, share
  price, net worth and the time controls always in view, and dense multi-pane screens. The map
  sits between an outliner (alerts, your divisions, rival moves) and the selected region's board;
  the CEO Office shows results, a pricing table and divisions side by side; Markets shows the full
  stock and commodity tables together; Stats runs in two columns; Inbox lists decisions beside the
  open one; Portfolio tables your companies and your shareholdings.

- A Command Center on Android and desktop: every action in the game in one searchable list,
  grouped by category, each with a form built for it (pickers, sliders with exact amounts,
  switches), a live preview of what it would do and Submit. Ctrl+K opens it from anywhere on
  desktop.
- A Research screen: the technology ladder to level 20, progress and cost to the next level,
  what it buys, the industry frontier, and research and marketing budgets on sliders.
- An economy card on Markets (a fold-out panel on phones): the cycle phase, growth, inflation
  against target, the central bank, prime and ten-year rates with a history chart, the central
  bank chair and recent decisions, the government, its approval, the next election, laws in
  force and announced, and the news.
- Founding a company when you start with none: name, headquarters with wages and demand, first
  strategy by sector with capex, build time and margin, and capital on a slider.
- A guided tutorial of eight steps that tick off as you do them, each with a Show me button;
  docked on desktop, a slim bar on phones, replayable from Settings.
- Desktop: a grouped sidebar with icons and a company switcher, a status bar with the economy
  phase, the central bank rate and inflation, action search, keyboard shortcuts (Space plays or
  pauses, N next week, 1, 2 and 3 set the speed, F1 to F7 switch screens), and hover states.
- A new main menu on Android and desktop: a slowly turning globe with stars where each
  scenario's region glows and is named as it turns into view, the sector icons flying in from
  around the globe to settle under the logo, and Continue, New game, Load game, Settings and Quit.
- A new-game screen: scenario cards with art (Great Lakes 2000 to play; Texas Oil Patch 1981,
  Silicon Valley 1995 and Ruhr and Rhine 1991 coming soon), then how you start, difficulty, the
  economy on day one and starting capital.
- Load game: every save with its company, game week and date, newest first. Save now keeps its
  own save beside the autosave instead of overwriting it.
- Sliders instead of fixed choices, on every platform: a custom track with tick marks, a live
  readout of what the value means and nudge buttons for exact steps. Production target runs 0 to
  100 percent in 1 percent steps with the lots a week it gives; pricing against the market runs
  -50 to +50 percent and over cost 0 to 100 percent, each showing the price and margin per lot;
  raid size runs 0.5 to 4 to 1 in tenths; training and quality budgets slide up to a tenth of
  weekly revenue beside the typed amount; a merger remedy divests 1 to 50 percent in half steps.

- A launcher on desktop: a branded splash window with progress while the engine and world map
  load, then the game window centred at the chosen size.
- Updates from inside the game. At launch (switchable in Settings) the game reads the public
  release list; when a newer version is out the main menu shows it with its notes. On Windows
  "Update now" downloads the installer with a progress bar, installs it and reopens the game;
  Linux, Mac and Android open the download.
- Tester previews: preview builds show as 1.6.0-preview.N, follow pre-releases in the updater so
  each preview offers the next one and then the final release, and install over each other.
- A Preview release workflow that builds tester previews for Windows, Linux and Android and
  publishes them as GitHub pre-releases; the full release workflow now skips pre-releases.
- The Windows installer adds a Start menu entry and a desktop shortcut and lets you choose the
  install folder.

- A real business cycle. Output runs above or below its trend with momentum; good times and
  cheap money build up credit, and the more credit the likelier a bust, which leaves borrowers
  paying debt down for months. Inflation rises when the economy runs hot. Household demand,
  loan rates and the nominal drift of wages and import prices all follow the cycle. Booms and
  recessions now come from the economy instead of random cards.
- A central bank that meets every six weeks and moves its rate in quarter points toward its
  inflation target. Its chair has a stance from dovish to hawkish that tilts every decision;
  at the end of a four-year term the government reappoints the chair or names a new one. Each
  meeting records its decision and reason. A prime lending rate and a ten-year bond yield
  follow from the policy rate.
- Government and laws. Two parties, an election every four years from November 2000 decided by
  the economy, and a programme that takes effect a quarter after a change of government:
  corporate tax, a low-skill wage floor, the share at which a merger is reviewed, and union law.
- Difficulty: Easy ($8M, rivals never attack your markets), Standard ($5M) and Hard ($3M, every
  rival fights for share), a starting economy (as in 2000, calm, boom or downturn) and a custom
  starting capital.
- Three future scenarios shown on the scenario picker: Texas Oil Patch 1981, Silicon Valley
  1995 and Ruhr and Rhine 1991, with art for every scenario.
- An action catalogue in the engine describing every player action once, so every platform can
  list and offer all of them.

### Changed

- Pricing against the market now allows -50 to +50 percent (was -20 to +20). Prices still stay
  between half and twice the reference.
- Desktop opens at 1600 x 900 instead of a phone-shaped window, and the map is the first tab on
  Android and desktop.

## [1.5.0] - 2026-10-27

The whole world on a globe, scenarios, a Stats screen, tougher rivals and a new look. Saves from
1.4.0 do not load in 1.5.0: the engine now keeps five years of company history.

### Added

- A new 8-bit Corporate Warfare logo: a gold C beside a W drawn as a stock line that runs green
  to red and breaks out into an arrow. It is the app icon on iPhone, iPad, Mac, Android, Windows
  and Linux and leads the redesigned start screen, with a row of the new 8-bit sector icons.
- 8-bit icons for all fifteen sectors, shared by every platform.
- A speed control: pause, 1x, 2x and 4x (one tick is one week; 1x is a week a second).
- A Stats screen for every company, from the CEO Office: results by week, four-week period or
  year with revenue and profit charts, the income statement beside the period before, the balance
  sheet over time, market value, shares issued, free float, price to earnings, the share price
  and who owns it. The engine now keeps five years of each company's results.
- Scenarios on the start screen. A new career starts from a scenario card that shows the
  country, starting year, number of regions and the situation; Great Lakes, 2000 is the first.
- The whole world on the map. Zoomed out it is a globe you can spin; zooming in flattens it
  smoothly onto the scenario, where counties, roads and rivers fade in. Double-tap, or the
  globe button, flies between the two. Zoom buttons on every platform and the mouse wheel on
  desktop.
- Outside the scenario the world is greyed out but still mapped: every country, state and
  province border and the main highways, in grey, with the scenario in colour.

### Changed

- Buttons no longer flash while weeks process: a running week does not mark the app busy.
- The seed field is gone from the new-career screen; every career gets a random seed.
- Test runs no longer send anonymous market data.
- Rival companies fight harder. Cost leaders, consolidators and vultures undercut prices, move
  into markets you hold, review their plans every two weeks and can bid for your companies.
  A rival waits at least 26 weeks between raids on the same target.
- Corporate Warfare is presented as an economic sim, not a Great Lakes game. The start screen,
  store text and market labels no longer advertise the region; the Great Lakes is named only as
  the first scenario.
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

[Unreleased]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.7.1...HEAD
[1.7.1]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.7.0...v1.7.1
[1.7.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.6.1...v1.7.0
[1.6.1]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.6.0...v1.6.1
[1.6.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.3.1...v1.4.0
[1.3.1]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/lakesidegamedev/corporate-warfare/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/lakesidegamedev/corporate-warfare/releases/tag/v0.1.0
