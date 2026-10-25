# Economy and corporate mechanics contract

Status: proposed implementation contract. Resolve worked examples before each system's
code packet. Balance constants are data, identities and settlement rules are code.

## Units and accounting

USD cents use checked i64 storage, i128 intermediate arithmetic, explicit round-half-even
where proportional amounts need rounding, and stable-ID allocation of residual cents.
Product quantities declare their units and fixed-point scale. Rates use named basis
points; no implicit percent/fraction conversions. Reject overflow without a partial
transaction. Annual-to-weekly interest uses a documented 52-week rule, never host floats.

Every legal firm and investor has a ledger. Use debits/credits and balanced postings per
transaction, with accounts for cash, restricted cash, receivables, payables, inventory,
construction-in-progress, fixed assets/accumulated depreciation, loans, bonds, share
capital and retained earnings. The outside household, government, lender, supplier and
securities sectors have explicit counterparty accounts. Money conservation is asserted
against those accounts; production and depreciation change real value, not cash by magic.

Examples that must be encoded as tests:

- Borrow $100: cash +100, loan liability +100, zero profit. Repay $20 principal and
  $2 interest: cash -22, debt -20, expense +2.
- Build a $100 asset: cash -100, CIP +100. Completion moves CIP into fixed assets.
  Depreciation is an expense without another cash withdrawal.
- Buy ten units for $20 then sell four for $12: inventory cost falls $8, gross profit
  is $4, remaining inventory cost is $12. Receivables timing is separate from revenue.
- Parent lends $50 to subsidiary: two legal ledgers balance; consolidated cash,
  debt to outsiders and profit are unchanged.
- Parent sells internally with markup: standalone profit and stock valuation remain
  explicit; eliminate internal revenue/expense and unrealised inventory profit in group
  reporting. No wealth or score exploit from repeated transfers.

Group reporting eliminates intercompany balances, trades and dividends. Do not add a
subsidiary's full equity value to a parent that already holds that investment. Minority
interests remain visible. Group cash is not automatically spendable by every subsidiary.

## Weekly clearing protocol

1. Validate a deterministic sequence of submitted commands against opening state;
   reserve funds and quantities atomically. Reject with a stable reason and no mutation.
2. Resolve expiries, scheduled arrivals, approved construction completions and due
   governance actions at a declared boundary. Newly arrived input can be used this week.
3. Calculate employment demand and wage offers from firm policies. Allocate finite
   skill pools simultaneously, including background employment. No firm-order advantage.
4. Allocate existing inputs and production capacity. Production consumes inputs once,
   accrues wages/energy and creates output or perishable service availability.
5. Clear intermediate and final markets using staged demand budgets. Reserve inventories,
   compute allocations and settle once. No same-week recursive production chain: purchases
   scheduled for later production become next week's usable inputs.
6. Create freight shipments; transit takes an integer number of weeks. Local delivery
   can settle this week; inter-region output cannot teleport into an earlier phase.
7. Post operating expenses, sales, defaults, interest, taxes and depreciation. Reconcile
   cash, stock and accounts. Run insolvency transitions before payouts.
8. Advance funded construction/R&D, assess credit and valuation, resolve queued events,
   and derive closing reports. AI observes only permitted closing data and submits future
   commands through the common validator.

The phase contract must name whether each quantity is opening or closing. Fast-forward
never skips a phase. Pause on critical decisions and prevent a queued choice from
silently expiring because the user backgrounded the app.

## Market allocation

Before implementation, write fixtures for one buyer/two sellers, two buyers/one scarce
supplier, imported supply, no demand, a tie and a zero-price offer. Initial proposal:
allocate demand by capped attractiveness weights based on delivered price, quality and
brand, redistribute unmet allocations until supply or budgets exhaust, and use stable
IDs for residual integer units. Revenue is fulfilled quantity times paid unit price.
Shares are measured from actual sales, never granted as an independent revenue source.

Household budgets derive from income/employment and category preferences. Intermediate
orders derive from production plans and stocks. Government procurement and service
purchases are explicit for defense/healthcare and similar products. Each has finite
budgets and payment timing. Gateways have finite volume, prices, lead time and shocks.
They are not infinite no-risk buyers for arbitrage. Define substitutes and income/price
elasticity; unreachable demand stays unmet rather than generating money.

Quotes expire or revalidate at execution. Freight, insurance, tax, financing and inventory
carrying costs enter delivered margin. A buy/sell loop over one week cannot create profit
without a documented external economic source.

## Labour and production

Labour pools have skill tiers, participation, employment and unfilled demand. Aggregate
NPC/background activity participates. Wages do not instantly reprice all existing labour;
contracts, churn and bargaining affect adjustment. Hiring, layoffs, retraining and strikes
consume time/cost. Staff cannot work for two facilities in the same week. Worker income
feeds household spending through an explicit lag/budget rule.

Unions organise at a facility or firm, bargain over wage/work rules, can strike, and
settle. Staff response depends on unemployment, pay gap, conditions and firm finances.
Explain union costs and gains rather than using a random production penalty.

Every production recipe declares stockable inputs, service inputs, labour, capital,
energy, output, waste/obsolescence and build/retool lead times. Services expire instead
of accumulating infinite hospital appointments or insurance policies in warehouses.
Capacity never implies guaranteed sales. R&D is funded, uncertain and bounded; technology
cannot make cost negative or quality unbounded.

## Ownership, governance and securities

Each share lot has holder, issuer, class, quantity and cost basis. Total issued shares
reconcile exactly. Model voting rights separately even if first release issues only one
ordinary class. Treasury shares have no vote or dividend. Outstanding shares exclude
retired stock. Define economic ownership and voting fractions with integer numerators.

Proposed control threshold: strictly more than 50 percent of eligible votes. Board/CEO
changes settle at a declared meeting or takeover boundary. Player commands must respect
management authority; owning a minority stake does not grant a firm's cash access.
Reject circular ownership and self-control paths before settlement, including indirect
cycles. Multilevel subsidiaries remain allowed without recursive stack traversal.

Trading is a simplified deterministic weekly exchange with finite background liquidity,
spread, price impact and ownership disclosure. No high-frequency order book. Issuance
requires authority, dilution preview, pricing and buyer funding. Buybacks reserve cash,
respect solvency, remove votes correctly and cannot trade against the same beneficial
owner to print valuation. Dividends transfer cash to eligible holders without creating
new profit. IPO changes disclosure, liquidity and governance, not productive capacity.

M&A has offer, escrow, due diligence, acceptance/tender, control transfer and optional
merger stages. Minority holders are paid explicitly. Offers can fail without losing
escrow incorrectly. Debt, contracts, tax losses, construction and labour transfer by
stated rules. Do not erase liabilities on acquisition. Distinguish buying assets, buying
shares and merging legal entities. Hostile bids create defensive choices, not an instant
single-button arbitrary company deletion.

## Credit, banking, insurance and regulation

Loans include issuer/lender, principal, rate, duration, collateral, covenants and schedule.
Bonds include issuance, coupon, maturity, holder lots and default seniority. Ratings are
explainable functions of leverage, cash flow and security. Loan/bond issuance is financed
by an explicit lender sector. Do not sum borrowed cash as operating profit.

Playable banking uses deposits as liabilities, loans as assets, liquidity/reserve rules,
credit losses and capital constraints. Internal transfers do not create equity. Insurance
uses policies, premiums, reserves, reinsurance if included, claims and correlated shocks.
Insurance profit cannot be simply premiums with random costs omitted. Stress bank runs
and catastrophe claims without crashing or inventing bailout money.

Antitrust uses relevant product/region market concentration, control groups, review
thresholds, delays, remedies and appeal/withdrawal options. Merger does not become legal
just because the cash exists. Lobbying spends disclosed cash toward a specific policy
request with bounded and uncertain effect, cooldown and reputational consequences.
It cannot override every insolvency or review rule. Regional policy affects taxes,
incentives and permitting without adding a full politics simulation.

## Insolvency and investor continuity

Distinguish cash shortage, covenant breach, insolvency, restructuring and liquidation.
Payment priority and grace periods are deterministic. Freeze illegal payouts, value
collateral conservatively, resolve claims by seniority and distribute residual equity.
Parent guarantees transmit losses only where contracted. A subsidiary failure does not
automatically consume investor cash or every sibling's cash.

The investor retains unpledged personal cash and other holdings after company loss.
Provide a new-company route or management opportunity; if truly destitute, a disclosed
restart package may exist as a difficulty rule, never a hidden ledger fix. Confirm its
exact amount/conditions during balance design. Career records show losses permanently.
