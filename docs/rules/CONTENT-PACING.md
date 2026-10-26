# Initial content, macro and pacing rules

## Calibration authority

`content/initial-balance.json` fixes engineering reference prices, 25 product/contract
IDs, baseline fixture wages and 17 physical/service facility templates across 14
industries. Financial industry has bank and insurer models in FINANCE-LABOUR-AI.
These numbers are authored initial calibration, visibly separate from real regional
data. Do not call them measured historical values. The data pipeline later supplies
regional cost/demand/employment multipliers; it does not silently replace game identities.

One product lot is a standardized game batch, not an assertion that one tonne of steel
costs the tabulated number. Each product's display metadata must label its lot meaning.
Reference lots deliberately aggregate small transactions. A financial contract is not
stock produced by a factory and never enters the goods-clearing solver.

All recipe input/output quantities describe one full-capacity week. Fractional production
uses milli-lots and a carried input remainder, no rounding-free resources. Facility base
maintenance is 2 percent initial capex/year; straight-line depreciation over listed life;
fixture land rent $1,000/week before sourced regional modifier. Construction purchases
40 percent construction service and 60 percent equipment, both within declared capex,
never capex plus another uncapped duplicate expense. Pay into construction escrow,
release by completed work and equipment receipt. Insufficient service/equipment supply
delays the project rather than generating missing inputs. The seed world starts with
working construction/equipment providers; a founder is not forced to build every industry.

Recipe profitability is NOT guaranteed at reference prices. Competition, shortages and
staffing are core risks. Before adjusting a recipe, print revenue, every input, wages,
maintenance, rent, depreciation and funding cost for the full week, and compare scenarios.
One explicit accounting fixture: farm output 600 lots at $100 = $60,000 revenue; fuel
20*$300 = $6,000; electricity 10*$1,200 = $12,000; wages 60*$400+10*$800+2*$1,400 =
$34,800; maintenance $2,000,000*2%/52 = $769.23; land $1,000; depreciation
$2,000,000/(20*52) = $1,923.08. EBITDA $5,430.77, EBIT $3,507.69 before tax/interest.
Accounting carries exact fractional cents, then uses documented rounding. No margin
field grants additional cash beyond those transactions.

## Goods, services and payer rules

Farm food and finished goods enter household demand through retail, not a second direct
household purchase. Services and subscriptions have per-week availability, no stockpile.
Electricity is a weekly local service; unused capacity expires. Ore deposits have initial
recoverable lots equal to 20 years full output; extraction consumes deposit stock and
has a 5 percent revenue reclamation provision. Farm yield seasonal index by quarter:
[8000,11000,13000,8000] BPS, annual mean 10000. Production cannot exceed harvested stock;
recipe base capacity is adjusted before input planning.

Healthcare payer split 70 percent explicit government budget, 30 percent households.
Defense demand is funded procurement with milestones, 10 percent deposit and payment on
accepted delivery. Contract failure penalty 5 percent undelivered value after two-week
grace. Government expenditure must have cash/fiscal issuance counterparty and a ceiling;
not an unlimited offer. Office leases bill occupied blocks; vacant capacity earns zero.
Software licences prepay52 weeks: buyer records prepaid asset and expenses1/52 weekly;
seller receives cash once, records unearned revenue and recognises1/52 weekly. Expiry
ends access without another automatic cash transfer. Telecom
subscriptions renew weekly with cancellation and service failure. Advertising demand
comes from actual firm marketing budgets; do not add independent infinite media revenue.
Research/training/maintenance are outside-service offers at start, finite by regional
workforce; later named firms can provide them using declared service templates.

## Macro and long run

Game starts week 1 of 2000. Use 52-week years consistently. Baseline annual inflation
200 BPS, real household demand growth 200 BPS, technology frontier +1 level/year, policy
rate 600 BPS. Compound via rational weekly accumulators, never floating-point state.
At quarter boundary draw one macro impulse from [-100,-50,0,0,0,50,100] BPS with a
separate macro RNG stream, persist before reports. Policy rate = clamp(previous + impulse,
100,1500). Nominal reference price index grows at annual inflation /52 with remainder.
Real growth changes demand slowly, constrained by actual incomes/budgets.

Recessions are explicit events reducing real demand 10 percent for 26 weeks with two
quarters recovery. Booms raise 10 percent similarly. Permit only one macro regime event
at a time; no compounding unbounded pile of multipliers. National history is not scripted
as factual replication of 2000 onward. Crisis events explain the alternate-history model.

R&D spend accumulates progress; level threshold = $100,000 * (level+1)^2, capped at
frontier+1. At threshold, deterministic success draw 7500 BPS once, failure retains
half progress and costs remain spent. Tech level max 20, per-level efficiency improvement
2 percent additive, floor input/labour intensity 60 percent baseline. Brand spend to
regional/product stock: +round(spend / baseline_weekly_category_spend *10000) BPS,
maximum +100/week, decay 10/week to 10000, clamp 7500..15000. Quality baseline 10000,
tech +200/level, quality investment +up to1000, total <=20000. No upgrade grants free
sales or an infinite productivity curve. Future frontiers beyond max20 add scenario
opportunities, not an overflow in a table.

## Rival and event seed content

Exactly two initial named firms per industry (30), with rotating five archetypes,
plus player entry. National entrants arrive through events, not six extra hidden firms.
Regional background employers/suppliers still account for the rest of the real economy.
Scale aggregate regional employment, households and final demand to 1 percent of the
source economy for the playable market, retaining published per-worker wages and relative
industry mix. Store both source totals and explicitly labelled game-scale totals. This is
a calibrated model, not a claim that the real region is that small. Seed each named firm
with one to four operating template blocks; subtract actual named worker/output totals
from the scaled background. Reject a seed exceeding its region/industry pool. Never add
named production on top of an already complete background economy. No named firm starts with market share exceeding its real fulfilled capacity.
Names use fictional region+invented brand combinations; no real company logos.

Author 24 event cards as the following mandatory list, each with at least two legal
choices when player-facing: supplier default; energy outage; fuel spike; bridge closure;
freight congestion; crop loss; mine depletion warning; quality recall; talent poaching;
union drive; wage negotiation; strike escalation; training grant; permit delay; lobbying
backlash; antitrust review; hostile stake; tender offer; covenant warning; bank withdrawal
surge; insurance catastrophe; technology opportunity; recession; demand boom.
Each card has a source trigger, no duplicate active event on the same subject, and a
26-week cooldown unless its state machine has a stricter duration. A trigger caused by
actual rules is not also an unrelated random duplicate penalty. Rarity/weights live in
content, scenario fixtures pin each card's outcome. No inaccessible CLI-only choices.

## Session pacing and score

Default paused. Step = one week. Play 1x = one week/second, 4x = four/second,
fast-forward = up to 16/second bounded by device work budget. Never loop on main actor.
Pause at pending mandatory choices, funding/default grace <=one week, a tender/board
vote needing action, first strike, save failure and week260 score. UI batches routine
news into quarterly summaries; no demand for a click every tick. Resting a screen or
backgrounding cannot silently reject a deal.

Five-year score is numeric, explainable and optional to continue. It measures what the
player built over the opening position (owner decision 2026-10-26). Components, each 0..100:
growth = clamp(20 + 80*(ending_personal_net_worth/start_net_worth -1),0,100);
expansion = clamp(20*(player divisions - 1) + 10*(states operated in - 1),0,100);
leadership = clamp(25*markets where the player's group has the largest capacity,0,100);
resilience = fraction of weeks without past-due liabilities *100;
conduct = 100 - 10*unresolved_regulatory_breaches - 5*failed_worker_settlements, clamped.
Total = 0.45*growth+0.25*expansion+0.10*leadership+0.10*resilience+0.10*conduct. Control
(controlled productive-group value / portfolio value) is reported, not scored. Integer rational
implementation with explicit rounding. Net worth is personal cash + direct holdings at
fair value - personal liabilities; no gross internal revenue or double parent/sub values.
Assisted recovery score is flagged, not erased. Start wealth includes fair value of any
inherited gifted equity. Difficulty adjusts starting leverage/rival foresight only by
published rule sets, never hidden income boosts. Begin with Standard only; add Easy/Hard
once balance fixtures establish a clear baseline, without delaying core scenario testing.

## Data mapping and background fiscal baseline

Pin NAICS2002 for the initial mapping; reconcile source SIC/other NAICS vintages in a
versioned mapping file. Assign each source cell exactly once. Priority specialisations:
325 chemicals; 3364 aerospace/defense proxy; 5112 software/technology; 517 telecom;
5415 computer systems/technology. Then residual broad sectors: 11 agriculture,21
extraction,22 energy,23 construction,31-33 manufacturing,48-49 logistics,44-45 retail,
62 healthcare,52 financial,51 media,53 real estate. Defense's aerospace proxy includes
civilian output and must be labelled estimated, not declared military spending.
Unmapped source industries belong to outside/background economy; they are not silently
redistributed into the fifteen playable rows. Keep source classification, game assignment,
proxy flag and exclusions in the transformation report.

For each region, game workforce = round(source workforce/100), household count likewise,
and demand volume uses the same aggregate scale. Per-worker wage and per-lot prices do
not divide by100. Largest remainders preserve category totals. No runtime county rows.
Taxable labour income pays20 percent to government. Government opening treasury =13 weeks
baseline tax receipts. Weekly baseline procurement/healthcare/training budgets =80 percent
trailing13week mean tax receipts, split50/40/10 percent. Preserve unmet procurement demand
as cancelled/deferred requests, never a fabricated sale. A deficit may issue funded bonds
up to52 weeks baseline tax receipts outstanding; no buyer cash means no new spending.
Background household transfer support = min(remaining fiscal budget, 30 percent baseline
wages of unemployed), paid only from treasury. Report unpaid needs and fiscal stress.

A named initial firm owns1-4 template facility blocks, assigned by largest scaled industry
employment region followed by next largest eligible region, rotating firms to create at
least two direct named competitors in each major goods chain. Seed only if actual workers
and budgeted demand fit; shrink background first, then reduce facility block count rather
than allowing negative population. Starter loan <=30 percent tangible assets; cash =13
weeks full operating costs; paid capital balances the opening ledger. Parent/company
opening equity holders use investor archetypes, no illegal circular stakes. Public/private
mix is one public and one private firm per industry. Dataset scenario validator fails
if a recipe is impossible to supply through both local offers and reachable gateways.
