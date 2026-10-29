# Currencies and exchange rates

A scenario may span several countries. This is a universal mechanic: a scenario with one
currency never activates it and plays, saves and hashes exactly as before.

## Data

`content/scenarios/<id>/currencies.json` lists countries, currencies and central bank
behaviour. Every region names its country. The first currency (the numeraire) belongs to the
anchor country: its central bank is the macro model of `macro_cycle.rs`. All numbers are
authored game estimates, not market history.

Ledger units are re-based so one unit of every currency buys the same on day one. The real
nominal ratio (`face_milli`) only quotes rates.

## Books

A firm keeps its books in the currency of its home region. Investor cash, the exchange's share
quotes, the state and the outside lenders are in the numeraire. The engine converts at the
current rate where money crosses a currency line: market trade, freight, the gateways, public
works and state purchases, tax, loans from outside lenders, share issues, dividends,
buybacks, IPOs, liquidation payouts and founding capital.

## Weekly rate model (`economy/fx.rs`)

1. Each country has inflation (the anchor's, plus an authored gap and a slow random walk) and
   a central bank rate (the anchor's, plus an authored gap, half its excess inflation and the
   strain of defending a peg).
2. The fundamental rate pulls back toward purchasing power parity, tilts with the real rate
   gap and takes a random shock.
3. A pegged currency is held inside a band around parity. Strain is how far the fundamental
   sits outside the inner half of the band. Strain, plus a scripted window on a historical
   price path, raises the weekly chance of a band crisis, which widens the band.
4. Gateway prices and background wages follow the currency and the inflation gap.
5. A sale paid in another currency books a receivable and is collected two weeks later at the
   rate then. The difference is booked to other income and recorded as an FX gain or loss.

## Restrictions

Contracts, intercompany loans, guarantees, deposits, bank loans, insurance, mergers and market
raids need both firms in one currency. A firm builds only in regions of its own currency (to
operate in another country, found a firm there). Only numeraire firms hold stakes in other
firms and issue bonds. The state rescues only numeraire firms. It buys defence and strategic reserves from firms in
every currency of the scenario, the same rule in every scenario: a defence contract is written
in the supplier's own money and the treasury, which keeps its books in the numeraire, pays each
delivery converted at that week's rate. A supplier outside the numeraire gets no advance (an
advance held for a year would carry the exchange rate with it) and is paid on delivery. One
treasury funds every country's purchases; separate treasuries per country are not modelled.

## Reporting currency

A display choice of the player, held by the session and never saved or hashed. Investor cash and
net worth are shown in it. Company figures show their own currency where it differs.
