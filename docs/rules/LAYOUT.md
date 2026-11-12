# The 2.0 layout

The desktop app opens in the 2.0 layout (`Tycoon.kt`, `GridView.kt`, `DataGrid.kt`). The previous
layout (the 1.x sidebar and pages, `Desktop.kt`) stays for this release behind **Classic layout**
in Settings; the environment variable `CW_UI=1.0` forces it for a session and `CW_UI=2.0` forces
the new one. Phones keep the tab layout of `App.kt`. Rules for what the screens show live in the
other rule pages; this page is only how the 2.0 layout is organised.

## Frame
- **Top strip**: cash, profit, cash in four weeks, share price, league place by market value, net
  worth, the economy, the date and the time controls. Each figure opens the page behind it. It
  also holds the one timeframe (4 weeks, 13 weeks, a year, five years or everything) that drives
  every change, percent, trend and bar, and the pinned focus (a sector, a region or your own).
- **Left**: pages grouped by what you run. Company (Overview, Finance, Research, People, Plants),
  Markets (Goods, Openings, Regions, Freight and lanes, Auctions and listings), Rivals (League
  table, Raids, Activity, Deals and stakes), World (Economy, Politics and laws, Timeline, The
  paper, Letters) and You (Portfolio, Career, Hall of Careers). The map docks beside any page with
  Ctrl M. Ctrl K opens the actions palette, which also finds places, goods, companies, divisions
  and saved views.
- **Right rail**: three tabs. Needs you (decisions, company alerts, unread letters, raids on you),
  Watch (each starred good or company with its price and change over the timeframe, and its one
  alert) and Feed (raids and rival moves from the last 13 weeks, the ones that involve your company
  first). It folds to three marks with counts.

## Tables
Every table has one control bar in the same order: View, Scope, Timeframe, Search, Filter,
Columns and a menu for the rest.
- Views are saved column, sort, filter and group sets. Each screen ships ready views; any change
  marks a view as changed, with Save, Save as and Reset. Saved views live in the settings, not
  the save, so they carry to the next career; the number keys 1 to 9 switch between them.
- Scope narrows to your own rows, a sector or a region, with a total of what is shown.
- Filters show as chips: click a cell for only this or not this, pick a column for a range or the
  top or bottom few, or type terms such as `sector:chemicals price>10 mine`.
- A column can show a figure, its change, its change in percent, a trend, a bar or its rank. Rows
  can be grouped with subtotals. A change the game keeps no history for is not offered, and a
  timeframe longer than the history shows a dash.
- Clicking a figure with a history opens its breakdown; Ctrl-click or a long press puts it in a
  side pane, where a pinned pane stays so two can be compared.
- Ctrl-click or hold rows to mark them and chart them together. A table copies itself for a
  spreadsheet.
- On a phone the same table is two-line rows with the same sort and filter.

## Screens that are 2.0-only
- **Openings**: where a new plant would pay (CAPACITY.md), by sector, region or fastest payback,
  with Build here.
- **Auctions and listings**: plants on offer with bid, accept, withdraw and list a site
  (AUCTIONS.md).
- **Raids**: campaigns against you, by you and between rivals, with Defend (RAIDS.md).
- **Career**: ambition, medal, rank, what the next medal needs, the board's warning and the year
  by year record (CORPORATE.md, Careers).

## Content
The layout carries no game numbers. Saved views, the timeframe, stars and the classic layout
choice live in the app's settings file, not in the save. The medal wording and career lengths on
the new-game screen come from `content/careers.json`.
