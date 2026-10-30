# Corporate rules v0.1

Author-selected initial rules. USD figures are game calibration, not historical claims.
Every money flow has an opposing ledger posting. Implement tests before changing rules.

## Investor and formation

Standard founder investor starts with $5 million personal cash, no debt and no shares.
Formation costs $25,000 paid to outside legal-services sector. The investor chooses
firm name, region and paid-in capital, minimum $250,000; that capital becomes firm cash
and investor equity investment. Issue 1,000,000 ordinary shares initially. One class,
one vote/share, no fractional share lots. Other investors can subscribe in later rounds.
Legal entity is a corporation; no fictional LLC tax special cases in v0.1. The founder
start picks a template facility but shows full formation/build/funding transactions.

The guided start grants the investor 60 percent of a seeded firm with $1 million cash,
a $500,000 loan and an operating facility with a documented $2 million opening asset
basis. Outside investors own 40 percent. Opening retained earnings/capital reconcile,
not a plug added each week. Career scenario includes the opening transfer in seed state.

At true destitution (personal unpledged cash < $25,000, no positive-value saleable holding,
and no controlled viable firm), offer an explicit new management contract: one-time
$100,000 signing bonus from an outside distressed investor plus management of its firm.
Career assistance counter increments; assisted five-year scores are labelled. No automatic
bonus, no repeated eligibility within 260 weeks, no deletion of prior losses.

## Group/control and transactions

Strictly >50 percent of eligible votes establishes corporate parent control. Exactly
50 percent is not control. Reject any purchase/issuance creating direct or indirect equity
cycles, including minority cycles. Firms can hold minority stakes in several independent
groups, but at most one immediate controlling parent. Use iterative graph validation.
No arbitrary subsidiary count/depth limit. Resource exhaustion rejects a command safely
with diagnostics rather than silently pruning companies.

Parent authority can appoint managers at the governance boundary, not spend subsidiary
cash directly. Internal loans require lender and borrower authority, positive term/rate,
and solvency previews. Transactions with minority-owned subsidiaries disclose terms;
explicit transfer contracts must lie within +/-20 percent of the current regional reference price,
except explicit approved restructuring. Standing group supply (below) is priced by the
supplier's transfer setting instead, and its consequences replace the band. Same beneficial-owner wash trades cannot move
exchange reference prices. Guarantees identify guarantor, covered claim and maximum sum.

Board has five seats allocated at the 13-week meeting by largest-remainder voting share.
An eligible holder of >50 percent can call an extraordinary meeting four weeks ahead.
Three seats elect/remove CEO and approve merger or large issuance. CEO can operate and
borrow inside board-authorised limits. Initial limits: debt <=60 percent of tangible
assets and single capex <=25 percent of total assets; exceeding either queues a board
vote. Player investor must actually control votes/seats or obtain other holders' support.
NPC directors support positive solvency-preserving proposals with expected 26-week
cash benefit; reject transfers below fair-value bound and insolvency-causing dividends.
Tie means status quo. Public minority holders can vote against a controller's conflict deal.

## Shares, payouts and values

Weekly exchange uses external float plus explicit holder orders. Outstanding treasury
shares have zero votes/dividends. Quotes: fair value/share +/-2 percent spread; external
weekly executable float is 2 percent of freely held shares. Price impact = 10 percent
of requested quantity / weekly float, capped at 25 percent. Quote reserves cash/shares,
expires next boundary and revalidates solvency/ownership. Same beneficial owner cannot
match itself. Selling below a prior cost is a realised investor loss, not company expense.

Issue shares at >=90 percent of fair price with a funding buyer, board authority and
100 percent paid cash at settlement. Show exact dilution before acceptance. IPO costs
2 percent proceeds, minimum $100,000, requires positive equity and 13 weeks of statements,
and sells at least 10 percent float. Secondary sale transfers cash between holders only.
Buybacks cost firm cash, move bought shares to treasury, and cannot leave less than
four weeks forecast obligations. Cancellation reduces issued/treasury shares together.
Dividends require positive distributable retained earnings and the same cash buffer;
allocate cents across eligible shares by largest remainder. No dividends to treasury.

Weekly fair enterprise value = max(40 percent net tangible operating asset book value,
6 * max(0, trailing_13_week_EBITDA * 4)). Before 13 observations, annualise available
weeks explicitly. Equity fair value = max(0, enterprise + unrestricted_cash + stake_values
- loans - bonds - deposits - insurance_claim_liabilities - overdue_payables). Exclude
operating cash already capitalised elsewhere; ledger investment assets are replaced by
look-through stake fair values for valuation only. Calculate share values in reverse
acyclic ownership order. Do not add parent book investment and stake fair value twice.
Bank/insurance enterprise uses 1.0 * adjusted tangible equity rather than EBITDA multiple;
then value outside owned stakes separately, without subtracting the same debt twice.

Reference exchange mark moves 1/8 toward fair value weekly; actual transactions affect
mark only from genuine different-beneficial-owner fills, capped +/-10 percent/week.
Pledged/escrow shares cannot be sold twice. UI distinguishes traded mark, fair estimate
and liquidation value. No player wealth from changing a price slider without a sale.

## M&A state machine

DRAFT -> FUNDED -> DILIGENCE -> OFFER_OPEN -> REVIEW -> SETTLED; terminal alternatives
are WITHDRAWN, EXPIRED, REJECTED. No path skips escrow or legal review. Buyer puts full
maximum cash price plus 2 percent transaction fee in escrow. Diligence lasts two weeks,
costs 0.5 percent bid value and reveals target statements/contracts; offer lasts four weeks.
Hostile offer default premium is 25 percent above pre-bid mark; target holders tender if
price >= fair value * their reservation factor (1.0 normal, 1.1 loyal, 0.9 distressed).
Freeze valuation reference for bid fairness tests to avoid bidding against own mark push.

Crossing >50 percent buys voting control; merger/squeeze-out requires >=90 percent plus
review and full minority cash at max(offer price, fair value/share). The first release
uses this explicit game threshold, not a claim of US statutory law. Acquired shares
retain issuer entity until legal merger. A failed tender returns unused escrow minus
incurred costs. Settlement is atomic and checks current share availability/funding.
Asset purchase enumerates assets and attached assumed liabilities; share purchase leaves
all target liabilities in the target. Merger transfers the complete ledger/contracts,
resolves intra-group balances and retires target shares with a durable event trace.

Antitrust computes HHI = sum((group regional product sales share *100)^2) in 0..10000.
Use trailing 13-week unit sales and controlled group aggregation. Review if combined
share >=35 percent OR post-HHI >2500 and delta-HHI >200. Four-week review; approval if
neither trigger applies. If trigger applies, offer divestiture sufficient to put combined
share <=35 percent and delta <=200; failing remedy rejects merger. If a feasible asset
bundle cannot be found, reject rather than invent regulator discretion. Fees are spent,
escrow otherwise returns. These are transparent fictional game rules, not legal advice.

## Insolvency and tax

Cash may not silently become an unbounded negative balance. Missed due payments become
past-due liabilities. Freeze dividends/buybacks/new unsecured borrowing. Four-week grace
permits asset sale, equity raise or approved rescheduling. After grace, enter restructuring:
secured claims paid from collateral, administrative/payroll claims next, unsecured
including deposits/bonds proportionately next, equity residual last. No assets distributed
before claim resolution. Restructuring offers creditors 50 percent principal repayment
with four-quarter schedule; accept only if estimated liquidation recovery is lower, and only
against a plan: every operating plant that does not pay its way at reference prices is mothballed
(output sold less inputs, payroll at the firm's own wages, upkeep and land; a carrier plant is
judged on whether it moves a quarter of its capacity, a terminal is not judged alone), so the firm
returns after its year with a smaller cost base. A firm whose judged plants all lose money has no
plan and is liquidated; the player's request to restructure is refused for the same reason. The
firm stays in restructuring for its full 52 weeks: a bill it defers meanwhile joins its overdue
payables and does not turn the plan back into a four-week grace and a second haircut.
Otherwise liquidate assets at 40 percent carrying value through finite outside buyer
funds. Insufficient buyers delay sale, never mint cash. Parent guarantee shortfalls become
claims against guarantor up to the contracted cap. No implied guarantee for siblings.

Voluntary wind-down (`WindDown`) lets a solvent company with nothing left to run close itself.
It is allowed only when the company is trading normally, is not the career's primary company,
has no plant operating, building or retooling, holds no stake in another company, has no
trade payables, accruals, prepayments, insurance claims or provisions outstanding, and its
cash plus 40 percent of the carrying value of everything else covers every loan, bond, deposit,
arrear and accrued tax in full. Accrued tax is paid to the state, then the liquidation above
runs: assets sold at 40 percent to finite outside buyers, creditors paid by seniority (in
full, by the test), the residual to shareholders pro rata, and the company is dissolved. The
player and rivals meet the same test. A rival proposes it after thirteen closed weeks without
a sale when every plant has been mothballed for 26 weeks, or when it has had no plant at all
past its first year.

Corporate tax is 25 percent positive taxable quarterly profit; region modifier initially
0 until evidenced calibration, not fabricated state law. Carry losses forward for four
years in game accounting. Quarterly tax reserve accrues weekly; payment at week multiple
of 13. Dividends never reduce company taxable profit. Investor dividend/capital gains
personal tax is 15 percent, collected at realised payment/sale. Display rules as simplified
scenario policy, not a reconstruction of all 2000 tax codes.

## Divisions, market share and raids (1.2.0)

Implemented in `crates/cw-sim/src/economy/sectors.rs`. A division is a firm's plants of one
sector type in one region; it is derived from the plants, never stored apart. The first plant
of a sector type in a region opens a division for a $100,000 entry fee, paid to the outside
legal-services sector, with the founding plant at half its strategy's capex. Later plants join
the division and must run its strategy; `RetoolFacility` on any plant retools every plant in
its division (a quarter of the new capex per plant, a third of its build time, at least four
weeks). An order may add up to 255 plants; past construction capacity (below) it costs more and
takes longer.

Market share in a cell (region, sector type) is a firm's capacity value at reference price
against all operating capacity in the cell plus the value of imports local plants are not
meeting. Expanding an existing division past 50 percent share multiplies its capex by
1 + 2 * ramp * rivals, where ramp is the square of the share above half as a fraction of the
other half, and rivals is 0.35 with no rival in the cell rising to 1.0 at four or more.
Opening a division never pays it.

Sprawl overhead counts divisions, not plant sites. It grows smoothly with the square of the division count; logistics strength and owned logistics divisions take part of it off (see MARKETS.md, owned logistics).
A plant in the firm's home region is the input baseline; another region of the home state uses
2.5 percent more inputs and another state 5 percent more. Each state's primary and secondary
specialized sector types use 3 and 1.5 percent fewer.

`RaidMarket` spends a marketing budget against a rival's division in a cell the raider also
operates in. Attack strength is the budget plus thirteen weeks of standing marketing; defence is
thirteen weeks of the defender's marketing plus five percent of its division's quarterly output
value. One percent of the defender's plants are at stake at even strength, rising linearly to
twenty percent at two to one. Success probability is attack over attack plus defence, clamped
to 5..95 percent, from the deterministic raid stream. The budget buys advertising whatever
happens; on success the raider takes the plants into its own division at its own strategy and
pays the defender half their book value. A group cannot raid itself. The 1.7 raid
rules below replace the one-plant floor and the cooldown. Rival AI raids when the
chance-weighted three-year contribution of the captured plants exceeds the campaign and price.

## Board objectives (1.2.0), replaced in 1.9

The 1.2.0 quarterly objectives (`crates/cw-sim/src/economy/objectives.rs`) are retired. Their
goals live on as mandate kinds below, so there is one goal system, not two. The old types remain
in the file only because `World.objective` and `World.objective_history` are part of the save
layout; nothing writes them, and a goal an older save still carries is cleared, unpaid and
unpenalised, on the first week played. The FFI calls `objective()` and `objective_history()` keep
their shape and now report the mandate in force and the closed mandates.

## Chapters and mandates (1.9)

Implemented in `crates/cw-sim/src/economy/chapters.rs` and `achievements.rs`; numbers in
`content/mandates.json`; names in `content/scenarios/<id>/chapters.json`.

**Chapters.** Every 52 weeks of a career is a chapter: chapter `n` runs from week `52(n-1)+1` to
week `52n`. Its name is "Chapter n: " plus the era name of the first timeline event that plays in
it (the event's date, as moved by Shuffled mode), else the next generic name from the scenario's
list. Eras and generic names are fictional and plain; a scenario with no file uses a neutral list.

**Offers.** In the first week of a chapter the board of every operating company that runs plants
and is not a state firm offers three mandates. The player's board is the primary company's; a
company the player controls but did not make primary gets none. The offer is a pure function of the
world: kinds are tried in a fixed rotation (offset by chapter and company handle) and the first
three that the company can be asked for are offered. A kind the company cannot be asked for is
skipped (a market it already holds nine tenths of, debt it does not have, a record too short to
measure). A young company with no profit record is asked to turn a profit from zero.

| Kind | Asked when | Target (small, medium, large band) | Met when |
|---|---|---|---|
| Market share | the largest division holds under 90% | share now plus 8, 6 or 4 points, at most 95% | at chapter end the share is at or above the target |
| Profit growth | always | average weekly operating profit over the last 13 weeks, raised by 15, 10 or 7% (at least $5,000, $15,000 or $50,000 a week) | at chapter end the last-13-week average is at or above the target |
| Integrate | an input is bought that no plant of the company makes and some recipe makes it | the input bought in the largest volume | an operating plant of the company makes it |
| Cash cover | cash is already 10% above the line | 6, 8 or 10 weeks of weekly costs | cash is at or above the line in every week; one miss loses it |
| Debt to equity | debt to equity is over 0.25 | the ratio cut by 25, 20 or 15%, at least 0.20 | at chapter end the ratio is at or below the target |
| Win a raid | the company has a division | none | a raid it launched succeeds in the chapter |
| List a subsidiary | a group member is unlisted | none | more group members are listed than at the start |

The band is the company's total plant blocks at chapter start (4 or fewer, 12 or fewer, more).
Difficulty multiplies the stretch (the share step, profit rise, cover weeks and debt cut): Easy 80%,
Standard 100%, Hard 125%. Event kinds latch (once true they stay true); share, profit and debt are
tested only at chapter end; cash cover is tested every week.

**Choosing.** `ChooseMandate { firm, pick }` takes the offer in position `pick` (0 to 2). It goes
through the ordinary command path: the actor must run the company (`NoAuthority` otherwise), the
company must have an open offer inside its four-week window (`WrongPhase`), and `pick` must be one
of the offered (`Arithmetic`, out of range). An offer not taken inside the window lapses with no
penalty. Rivals' boards pick through the same command, proposed by `cw-ai`: the offer already
furthest along, the first on a tie.

**Results.** At chapter end a met mandate pays the same management bonus as the old objectives,
now on a year: 2% of the company's operating profit over a year (the last 13 weeks, four times),
at least $50,000, never more than the company's free cash, moved from the company to the
investor, never minted. Player side only: a rival has no personal purse. Reputation moves for any
company: +150 basis points for a met mandate, -100 for a missed one, kept between -2,000 and
+5,000 (or where it already stood, if outside). "Board confidence" has no separate counter in the
engine: a miss costs reputation, and **two missed mandates in a row** in the player's primary
company is the `MissedGoals` trigger for an activist campaign (`structure.rs`), the same
governance path the two missed quarterly goals used. A chapter with no pick, or whose company
stopped operating, does not count as a miss.

**State and saves.** Chapter state is `World.chapters` (`#[serde(skip)]`), written in the `CHAP`
trailer after `SYST` and folded into `canonical_hash` when not empty, so a game reloaded
mid-chapter plays on identically. It is outside the postcard payload, so the save schema stays 3
(see INTERFACE.md). A save without the trailer loads with no chapter state and opens the current
chapter at once if it is inside its first four weeks, otherwise at the next one.

**Achievements.** Twenty fixed milestones (`achievements::LIST`), checked once a week from facts
the engine keeps, each earned once and dated by its first week: a chapter closed, a first mandate
met, five met, three in a row, a raid won, the main company listed, a subsidiary listed, a merger
completed, ten companies controlled, three states, five divisions, the largest capacity in a
market (checked every fourth week), a thousand workers, net worth doubled and tenfold, no debt
after the first year, a hostile bid survived, a bank rescued by the state, every event of the
scenario's timeline played, and a thirty year career (week 1,560). They are the player's: a career
with no primary company earns none.

**Size of the reward.** One bonus a year at most, at the old 2% rate on a year of profit, where the old objectives could pay four a year on a quarter each. The reputation steps are the size of the lobbying penalties already in the rules (100 to 250 basis points).

## Executives (1.9)

Every company has three executive posts: chief financial officer, chief operating officer and
logistics chief. Each post holds one person at a time, with a level from 1 to 5 and a temperament,
and each level moves one real number for that company only:

| Post | What it moves | Per level | At level 5 |
|---|---|---|---|
| Chief financial officer | Credit spread on loan and bond quotes (the quote, the rate floor a loan must meet, and the bank loan rate) | 15 basis points off, never below 25 | 0.75 points off |
| | Interest earned on cash, paid by the outside world | 4 basis points a year | 0.20 points |
| Chief operating officer | Build lead time (new plants and expansions), rounded down, at least 1 week | 4 percent shorter | 20 percent shorter |
| | Ceiling for a plant that is fully staffed, powered and rated at full output (not utilities) | 80 basis points above rating | 4 points above rating |
| Logistics chief | What outside carriers charge over the owners' empty-lane rate, for the shipper's own shipments | 5 percent of that premium | 25 percent of that premium |
| | Fee for reserving lane capacity | 100 basis points off | 5 percent off |

**Candidates.** A pool is drawn in a world's first week. The scenario's own named people for hire
(roles `cfo`, `coo`, `logistics` in `content/cast.json`, two per scenario) come first, then eight
generated people for each post from the extras with their temperaments and portraits. A candidate
holds a hire index: below 1000 it is an extra, from 1000 up it is the nth named person for hire, so
saves keep working as people are added. A world with no scenario id draws generated people only. Levels
weighted 30, 28, 22, 14 and 6 percent for levels 1 to 5, nobody twice. A post with fewer than three
people available gets two more every thirteen weeks. A person who leaves a post is away for 26 weeks.

**Pay and contract.** Weekly pay is `level rate x sqrt(total assets in dollars) x 10 cents`, with
the level rate 3, 5, 8, 12 and 18 tenths of a dollar for levels 1 to 5, so pay grows with level and
with the square root of company size. A temperament adds to the ask: raider and speculator 8
percent, financier 3 percent, builder and integrator nothing, cautious 4 percent less. The company
pays four weeks of pay when it signs and must keep another 26 weeks of pay in cash after it.
A contract runs 104 weeks. At the end the person renews at the current ask if the effect is worth
at least half a year of the new pay, 85 percent of the time (a draw named by the world seed, the
company and the week); otherwise they leave. Pay is a cash cost paid to the outside world; a
company that cannot pay a week of it loses the executive. Dismissal costs severance of a quarter of
the pay left on the contract, at least 4 and at most 26 weeks of pay.

**Commands.** `HireExecutive { firm, post, person }`, `DismissExecutive { firm, post }` and
`PoachExecutive { firm, from, post, offer_cents }` are validated the same for player and rival: the
company must be operating, the post empty for a hire or a poach, and the cash covered.

**Poaching.** An offer is accepted when it is at least the executive's current pay plus the
temperament's hurdle: speculator 8, raider 10, financier 15, builder 20, integrator 25 and cautious
35 percent. A refused offer still counts as the company's attempt. The poacher pays eight weeks of
the new pay on signing. The move is one story in the paper, and the old boss writes to the player
when the player took their executive.

**Rivals.** Every fourth week a rival with a vacant post hires the candidate whose yearly effect, at
the rival's own figures, exceeds a year of their pay by the most, provided the effect is at least 1.5
times a year of pay and the company holds the fee plus 26 weeks of pay. Effect values: the CFO is
debt times the spread cut plus cash times the yield; the COO is half of a year of last week's
revenue times the ceiling bonus plus 5 percent of a plant's cost per week of lead time saved per
plant being built; the logistics chief is the outside premium share of last week's outside carrier
spend times the cut plus the lane fees times the fee cut. A raider or speculator led rival may
instead offer a level 3 or better executive of another company the price that moves them, at most
once every 26 weeks, when the effect is at least 1.5 times a year of that pay.

**The board and the player's chief.** When the player's primary company has missed two board
mandates in a row (lapsed offers do not count), the board names a chief executive from the free
cast people at the close of the second missed chapter, with a letter from the board chair. The
count of misses starts again after that chapter, so an activist is not drawn by the same record.

**Brief.** The weekly brief suggests a hire when the effect, less the pay, pays back the signing
fee inside 104 weeks and the company can afford it.

**Save.** Executives and the candidate pool are saved state that affects play: they travel in the
`EXEC` trailer outside the postcard payload and `canonical_hash` folds them in while they are not
empty (see INTERFACE.md). No schema bump is needed.

## State rescue, nationalisation and privatisation (1.7)

A firm is strategic when it carries freight, stores goods or makes energy, or is a bank or
insurer. The rescue law is set by the party in power (see MARKETS.md, Government laws):

- None: normal restructuring and liquidation.
- Bailout (Commerce Party programme): when a strategic firm's payment grace has run out, the
  state lends it the whole of its arrears (overdue payables plus loan arrears) at the policy
  rate plus 3 points over 104 weeks, and the arrears are paid at once. The firm leaves
  past-due status and is not rescued again for three years: a second failure goes to the court.
- Nationalisation (Workers' Party programme): when the court would liquidate the firm (its
  assets would not cover half its debt) the state buys every private share at the higher
  of fair value and the market mark, paid from the government's cash to each holder (the
  investor, outside investors, other firms); then recapitalises the firm with the arrears in
  exchange for new shares at that price. All shares sit with the holder State. A firm the
  investor controls is never nationalised: it gets the bailout loan, as does any sound firm
  that is merely out of cash.
- The firm must have real systemic size (in the top quarter of firms by employees, or holding at
  least 30% of a lane) and its last 13 weeks' operating profit must not be negative, so a
  rescue buys a firm that can stand on its own. Small or structurally losing firms go to the
  court.
- The state acts only when the whole cost is at most 30% of its cash. Otherwise the court
  proceeds as before. Every payment has a counterparty on a real ledger.
- A state firm asks cost plus 5% for its goods and never more than the market ceiling. The
  rival AI makes no strategic moves for it. State dividends go to the government's cash.
- Privatisation (Commerce Party programme): a state firm that has been held 52 weeks, is
  operating and earned over the last 13 weeks is sold, one firm a week, to outside investors
  at the higher of fair value and mark, as far as their cash goes. Once the state no longer
  holds more than half the votes the firm is private again.

Merger review covers vertical integration when the law sets a threshold (Workers' Party,
35%): buying a firm that supplies a good your plants use, or a customer for what you make, goes
to the same four-week review as a horizontal deal when the maker's group holds at least the
threshold share of trailing unit sales in any regional market for that good. The usual remedy
rules apply.
## Corporate structure and integration (1.7)

Implemented in `crates/cw-sim/src/economy/structure.rs`. Every rule works the same in every
scenario and runs on defaults; alerts appear only when a decision is needed.

**Group supply.** Firms with one ultimate controller form a group. Each week the engine keeps one
standing contract for every (buyer, supplier, product) where a member burns what another makes,
sized to the buyer's need and the supplier's output. Contracts settle before the open market, so
group members get first call on each other's stock. Freight, intercompany inventory margin and
consolidation work as for any contract; there is no shortfall penalty inside a group.

**Transfer price.** Each supplying division (firm, region, sector type) has one setting, 0 to
12,000 basis points. 0 is the supplier's unit cost, 10,000 is the market price (median ask of
other makers, the default) and above is market plus a markup. Between 0 and 10,000 the price is
cost plus that share of the gap to market. Consequences, all measured on the goods actually
delivered:

- The squeeze. The firm on the losing side of a price away from market (the supplier when the
  price is under market, the buyer when over) has its non-controlling holders bear their share of
  the gap. That sum accumulates per firm and fades by 1/52 a week. An annualised squeeze of 3
  percent of equity draws an activist challenge (below).
- Tax. Each firm is taxed in its own region at the corporate rate plus any regional modifier.
  Profit moved to the lower-taxed firm saves the group the difference; the running total is in
  telemetry.
- Markdown. A group supplier whose trailing 13-week net income stays negative is marked down by
  half a point of fair equity for every week beyond 13, up to 25 percent. The count falls two
  weeks for every week it is not losing.

**Reputation.** Each division has a reputation from 0 to 10,000, starting at 2,500. The target is
2,500 plus twice the quality margin over 10,000 (-2,000 to +6,000, averaged over its products),
plus five points per basis point of revenue spent on marketing (up to 3,000) and the brand stock
above 10,000 (up to 3,000). It rises by 1/26 of the gap a week and falls by 1/52, so it is earned
faster than it is lost. It scales a seller's perceived quality in every market by up to +15
percent at 10,000 and -5 percent at 0, which is both a price premium and customer loyalty: the
seller keeps its share at a higher price. A successful raid costs the defender 300 plus a fifth
of the stake taken in reputation points, a failed one 100; a repeat raider loses 120 per raid on
record.

**Raids, smoothed.** The raid cooldown is gone. Each raid on the same rival still on the record
(one is forgotten per quarter) adds forty percent of the campaign budget in legal and
retaliation cost, paid to the outside legal sector. The stake at risk rises from 1 percent of the
defender's blocks at even strength to the whole division at five to one. The defender keeps no
guaranteed plant; instead the success chance is the attack-over-total chance times one minus
85 percent of the stake share, so a whole-division raid succeeds at a fifth of the base chance.

**Opening grace (1.7.1).** A division cannot be raided until 26 weeks after its oldest plant was
started. That covers a new game's starting divisions and any division opened later; a raid
against a division inside the window is refused, and rival AI does not attempt one.

**Rival raid pacing (1.8 preview 3).** A rival launches at most one raid a year: one that
campaigned in the last 52 weeks does not choose another. This is a rival's choice, not a rule;
the validator still accepts a raid from any company, the player's included, at any time. Without
it raids grew fourfold over five years, since each raid leaves grudges that invite the next, and
the write-offs and spent campaigns pushed the share of established companies losing money from 13
to 18 percent between years 2 and 5. With it raids level off (about 110 to 160 a year across 16
five-year careers, against 85 to 335) and that share goes from 12 to 16 percent; seed noise is
about 2 points.

**Construction capacity.** A firm builds at normal cost and speed while the blocks under
construction, including a new order, stay within 40 plus a tenth of the blocks it already runs
(at most 400). Each capacity's worth of excess adds fifty percent to capex (at most 2.5 times) and
a full base build time to the lead time (at most three times). Orders of up to 255 plants are
allowed.

**Bonds.** Price = base yield + credit spread. The base is the central bank's ten-year yield for
terms of ten years and more, blending linearly toward the policy rate for shorter terms. The
spread is 75 points plus 2.5 points per point of leverage (debt after the issue over net tangible
assets) plus 6 per point squared, plus 3,000,000 over coverage (annual EBITDA over annual interest
with the issue; capped at 900; 900 when EBITDA is not positive; neutral for firms with under four
weeks of results), plus 1.5 points per point of the issue over equity (capped at 300). The market
refuses spreads over 1,200 or leverage over 150 percent; otherwise there is only a price. Terms
are 26 to 780 weeks, the minimum issue $250,000, the fee 2 percent. Coupons are paid quarterly
from the ledger; unpaid coupons and principal become past-due liabilities and feed insolvency.
Principal returned at maturity comes off the holder's cost, never into interest income. Bonds trade
on a simple board at the mark, 10,000 less the gap between the issuer's yield now and its coupon
times the years left (3,000 to 11,500; at most 6,000 for a distressed issuer), plus accrued
coupon.

**Carve-out, listing, spin-off.** A carve-out moves a completed division into a new subsidiary
the parent wholly owns, with working capital of 5 percent of book (at least $100,000); the parent
keeps at least one other division. The division takes its trade with it: the parent's supply
contracts shipped from the division's state for a good its plants make, and its purchase
contracts delivered to that state for an input its plants use that no plant the parent keeps
there uses, pass to the new company (contracts holding escrow stay with the parent). A purchase
contract delivered to that state for an input that neither the new company's plants nor any plant
the parent keeps there uses ends at the carve-out, with no penalty on either side. A listing sells 5 to 49 percent of a subsidiary's votes to
outside investors at 95 percent of its mark, after 13 weeks of results, never giving up control;
the fee is 2 percent (at least $100,000). A spin-off hands the parent's whole stake to its
shareholders in proportion to their shares, with no cash.

**Joint ventures.** Two groups pay capital in proportion to stated stakes (20 to 80 percent for
the proposer) into a new firm that builds one division. The larger holder controls it; an even
split leaves it independent and run by the standing rules. Surplus cash above two months of
outflow is paid out as a dividend each quarter, 60 percent of it, in proportion to the stakes.
After 26 weeks a partner can buy the other out at fair value plus 10 percent or leave: the other
partner has first refusal at 95 percent of fair value, otherwise outside investors take the stake
at 85 percent.

**Activists.** Each quarter, for each firm with at least 3 percent of its votes held outside and 26
weeks of results, the engine looks for a grievance: a mark 30 percent or more under its peak
(peaks fade 0.5 percent a week), two missed board goals in a row (the player's primary company),
an annualised squeeze of 3 percent of equity, or a dividend that stopped after two quarters of
payments while cash covers eight weeks of outflow. The strongest case opens a campaign with a
demand (special dividend, divest the weakest subsidiary or division, a new chief executive, or
restitution with prices back at market). One campaign per firm per year. The player answers with
Concede (demand met in full), Negotiate (half, small fee) or Fight; unanswered, the board answers
by its temperament after four weeks. A fight is won by the activist with probability 60 percent of
the case's justification, plus up to 20 percent for its stake, a fifth of the outside float, less
30 percent of the controller's holding (5 to 90 percent). A win means the full demand plus damages
of 0.4 percent of equity; a loss costs 0.15 percent in legal fees.

**Earnings season (1.9).** A listed firm is one that is operating, has shares in issue and has at
least 1 percent of them held outside (not state owned). Each reports once a quarter, on the
quarter's close (weeks 13, 26, 39 and so on), and the week it reports the analyst writes the
estimate for the next. The estimate is the quarter just closed (operating profit, EBIT of the
thirteen closed weeks) plus an eighth of the change from the quarter before (the sim's quarters are
noisy, so the trend counts for little; at most a quarter of the estimate's scale either way), plus a quarter of the average change of the listed firms in the same
industry (at most 10 percent of the scale), plus noise of up to 10 percent of the scale drawn from
the world seed (a stream of its own per firm and week). The scale is the larger of the estimate's
size and four percent of the firm's market value (about a healthy firm's quarterly operating
profit), so a near-zero estimate cannot make a huge surprise out of nothing. A firm needs one full quarter of results before it has an estimate.

On results day the surprise is actual less estimate over the scale, bounded to plus or minus 100
percent. The share price moves 10 percent of itself per 100 percent of surprise. A run of
quarters surprising the same way adds 15 percent to the move per quarter in the run (up to four,
and not on a reversal). A miss is damped by management's credibility, up to 40 percent at full
credibility (10,000). The move is capped at 12 percent either way. It changes the share mark
only: no cash, share, account or ownership moves, so no money is created or destroyed, and the
usual price formation then pulls the mark back toward fair value an eighth a week.

**Guidance.** `GiveGuidance(firm, low, high)` (player and rivals, same checks) states a range for the
quarter's operating profit, once a quarter, before results day. The range must sit within half the
estimate's scale of the analyst's own number. The estimate moves toward the middle of the range by
20 percent plus 60 percent of credibility (at most 80), but never by more than 15 percent of the
scale. On results day, actual at or above the low end meets it: credibility rises by 1,500 (starting
at 3,000, ceiling 10,000), unless the low end was more than 10 percent of the scale under the
analyst's own number, which is sandbagging and earns nothing. Under the low end misses it:
credibility halves, reputation falls by 150 plus a fortieth of the shortfall in basis points (at
most 400). One small miss is forgiven; a second miss in a row, or one that falls 50 percent of the
scale or more under the low end, gives the firm a grievance for a quarter that the activists can act on
(`Reason::MissedGuidance`, justification 5,000 plus 1,500 per miss in a row, at most 9,000), which
opens a campaign by the usual scan and cooldown. Rival firms guide on their own: the week after their estimate, every fourth
listed firm in turn, a range of plus or minus 15 percent of the scale around the analyst's number,
unless profit fell last quarter.

**Ratings and short campaigns.** The analyst rates every listed firm from valuation against price:
buy when fair value is at least 120 percent of the price, sell when it is at most 83.33 percent
(the price 20 percent over value), otherwise hold. A firm whose fair value is half the price or
less (the price twice its value, about one listed firm in a hundred) draws an activist short
campaign (`Reason::Overvalued`, justification 1.5 times the gap, at most 9,000) through the ordinary
quarterly scan, player's firm included. The campaign knocks 3 to 8 percent off the mark when it
opens (prices only), then runs as any other activist campaign.

**Temperament.** Rival decisions here read `structure::temperament`, a table of weights (tax
shifting, leverage, listing, partnering, resistance) derived from the archetype today. The
character AI replaces that one function.
