# Phone visual specification v0.1

Build a calm, legible business instrument with visible regional identity. Do not copy
Capitalism Lab's desktop tables into a phone layout. Use native SwiftUI interaction and
navigation. This is the selected design, not a request for the implementation agent to
choose a style.

## Tokens

Light: background #F5F2EA, surface #FFFFFF, text #17202A, secondary #526070,
navy #14253A, positive #1E755B, negative #A13434, warning #805800.
Dark: background #0D1724, surface #172638, text #EDF0F4, secondary #AFBDCD,
positive #61BC99, negative #FF8A89, warning #E0B86D. Theme follows system by default.
Verify contrast after opacity/disabled styling; tokens alone do not certify every view.
Positive/negative always have direction/word labels as well as colour.

System typography with Dynamic Type: largeTitle serif for portfolio/company names,
headline for section names, body for explanatory text, caption for period/units.
Monospaced digits for comparable monetary/quantity values, never shrink-to-fit a whole
table. Abbreviate large values with accessible full-value label. Minimum body17pt at
standard size, controls at least44x44pt, common spacing8/16/24pt. Content readable in
320pt width, no essential horizontal scroll. Charts have text summaries and selectable
points; no decorative dashboard chart without an actual decision purpose.

## Navigation and layout

Five tabs: Portfolio (building.columns), Company (building.2), Regions (map), Markets
(chart.bar.xaxis), Inbox (tray). Use SF Symbols with visible text. Selected firm is a
persistent context chip below title, never a hidden filter. Company links to Finance,
Ownership, Facilities and Subsidiaries. Portfolio is investor context, distinct from
Company's legal-entity cash. A Finance action always shows whose balance it changes.

Dashboard order: title/context; critical decision banner if present; cash/forecast row;
profit/change explanation; most important bottleneck; upcoming obligation; recent trend.
One primary recommended next action with a reason, not an omniscient instruction. Values
show period. Tap a metric opens its ledger-backed explanation, not a bigger fake chart.

Region map uses actual simplified polygons if legible; else the selected fallback is a
fixed labelled cartogram derived from real centroid adjacency, with explicit legend.
Choosing cartogram never changes region membership or invents geographic connectivity.
Region detail prioritises wage/input/freight tradeoffs and two comparator cards before
secondary demographics. All map regions also appear in an accessible searchable list.

Action forms are sheets with visible actor, amount/quantity units, computed preview and
sticky confirm. Financial/control-changing actions require one clear confirmation,
routine recurring policy changes one submit. Destructive/investment preview includes
estimated effects and authority. Error remains inline, retains fields and provides
retry/remedy. No technical Rust/FFI/ID/version text in normal product screens.

## Motion and assets

Motion limited to short150-200ms value transitions and navigation. Reduce Motion removes
chart interpolation and zoom. Haptics only for committed material action/urgent alert,
respect settings. First release needs no soundtrack to meet completion; optional subtle
sound defaults off. App icon concept: simple industrial skyline within a rising portfolio
bar motif, navy/cream/copper, no small text or currency logo. Create original final vector
or bitmap asset and include all required Apple sizes through asset catalog. Never ship
the bootstrap debug view, generic framework icon or a watermarked placeholder.

Phone fixtures/screenshots required for every screen contract: empty/new career,
healthy operator, low cash, strike, hostile tender, insufficient authority, bank liquidity
warning, corrupted save recovery, year-five result and continued sandbox. Render with
real read models from deterministic scenarios. Data and actions must agree.
