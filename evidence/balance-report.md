# P33 balance distributions: release report

Date: 2026-10-25. Packet: P33. Requirement: R36 (automated gate).
Tested commit: `3fa5a327af92b62415627d3a344c18bb774cf811` on `release/1.0.0`. Engine `1.0.0`, interface `1`,
rules `1`, dataset hash `8aa1dc570dd24b7e7207049c724e6a94bd8a7b683f0cc1a12be77f16c52cb489`.
This replaces the first report (base `30d1b0c`). Standard difficulty only; scripted reference
policies, not human play. It is not a fun sign-off: R38 independent playtests remain.

Every number comes from the committed JSON in `evidence/balance/` or from the diagnostic
`cargo run --release -p cw-session --example dissolution_census -- 20`. Nothing is typed by
hand except the dollar conversion of cents.

## 1. What ran

Release build on the Contabo VPS (AMD EPYC, 18 vCPU, Linux 7.0), one sweep at a time.

```sh
cargo build --release -p cw-cli
./target/release/cw-cli sweep --policy <conservative|growth|acquirer> --scenario <guided|founder> --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/<scenario>-<policy>-260.json
./target/release/cw-cli sweep --policy <conservative|growth|acquirer> --scenario guided --sandbox --seeds 20 --weeks 1560 --from 1 --json evidence/balance/guided-<policy>-1560.json
./target/release/cw-cli sweep --policy none --scenario guided --five-year --seeds 20 --weeks 260 --from 1 --json evidence/balance/guided-none-260.json
```

Wall times:

```
guided-conservative-260 rc=0 25.7s err=0
guided-growth-260 rc=0 25.3s err=0
guided-acquirer-260 rc=0 27.2s err=0
founder-conservative-260 rc=0 24.8s err=0
founder-growth-260 rc=0 25.0s err=0
founder-acquirer-260 rc=0 24.6s err=0
guided-conservative-1560 rc=0 24.6s err=0
guided-growth-1560 rc=0 23.5s err=0
guided-acquirer-1560 rc=0 23.2s err=0
guided-none-260 rc=0 4.9s err=0
```

860 careers in total. Every run reached its target week, with no session errors and no
invariant violations: every ledger balanced, no negative cash, stock or deposit. Determinism
spot check: seed 7 acquirer, run twice from the release binary, gave
`0e447f65bb9fee21cc4a3d97099b650fcccef7ca2b4dcbe53c1e8695caff05a0` both times.

What changed in the harness since the first report: reference policies now answer mandatory
event cards through `RespondEvent` (first choice; the no-action baseline takes its passive
choice), because the corrected event deck now raises real mandatory cards; the acquirer
respects private-company information rights; the sweep reports rival outcomes by archetype;
and "player controls nothing" counts companies the investor ultimately controls, where the
first report counted any company it held a share in.

## 2. Distributions

| Run | Seeds | Score min/median/max | Personal net worth $m min/median/max | Rivals dissolved min/median/max | Rival deals settled min/median/max | Player controls nothing | Early stops | Invariant failures | Tukey outliers | Policy rejections |
|---|---:|---|---|---|---|---:|---:|---:|---:|---|
| guided-conservative-260 | 100 | 55/62/62 | 4.01/4.33/4.34 | 0/1/5 | 6/13/20 | 0 | 0 | 0 | 15 | none |
| guided-growth-260 | 100 | 53/60/60 | 3.69/4.02/4.03 | 0/1/5 | 6/12/20 | 0 | 0 | 0 | 12 | none |
| guided-acquirer-260 | 100 | 51/60/60 | 3.49/4.06/4.09 | 0/1/5 | 9/14/20 | 1 | 0 | 0 | 29 | none |
| founder-conservative-260 | 100 | 47/57/57 | 2.01/3.00/3.02 | 0/1/6 | 9/16/24 | 1 | 0 | 0 | 15 | none |
| founder-growth-260 | 100 | 44/54/55 | 2.01/2.50/2.58 | 0/1/6 | 9/17/24 | 4 | 0 | 0 | 17 | found_firm 33 |
| founder-acquirer-260 | 100 | 47/53/54 | 1.98/2.40/2.46 | 0/1/6 | 8/15/22 | 17 | 0 | 0 | 27 | found_firm 162 |
| guided-conservative-1560 | 20 | n/a | 3.67/4.25/5.02 | 5/9/13 | 28/35/46 | 0 | 0 | 0 | 0 | none |
| guided-growth-1560 | 20 | n/a | 3.49/3.49/3.76 | 5/9/14 | 30/38/48 | 17 | 0 | 0 | 4 | none |
| guided-acquirer-1560 | 20 | n/a | 3.49/3.66/4.46 | 5/10/15 | 26/36/45 | 8 | 0 | 0 | 0 | none |
| guided-none-260 | 20 | 55/62/62 | 4.21/4.40/4.40 | 0/1/4 | 6/12/17 | 0 | 0 | 0 | 7 | none |

| Guided five-year policy | Return | Resilience | Control | Conduct | Total (medians) |
|---|---:|---:|---:|---:|---:|
| conservative | 50 | 100 | 19 | 100 | 62 |
| growth | 48 | 100 | 13 | 100 | 60 |
| acquirer | 48 | 100 | 14 | 100 | 60 |
| none | 50 | 100 | 20 | 100 | 62 |

`guided-conservative-260` rival archetypes (a win is a surviving firm whose book equity grew):

| Archetype | Firm-runs | Wins | Losses | Dissolved | Deals settled/started | Commands accepted | Rejected |
|---|---:|---:|---:|---:|---|---:|---:|
| brand_builder | 600 | 438 | 162 | 11 | 0/0 | 100561 | 0 |
| consolidator | 700 | 555 | 145 | 25 | 1000/4083 | 18252 | 69 |
| cost_leader | 700 | 387 | 313 | 77 | 0/0 | 72316 | 0 |
| technologist | 600 | 562 | 38 | 9 | 0/0 | 7551 | 0 |
| vulture | 400 | 367 | 33 | 9 | 295/1463 | 7758 | 4 |

`guided-conservative-1560` rival archetypes (a win is a surviving firm whose book equity grew):

| Archetype | Firm-runs | Wins | Losses | Dissolved | Deals settled/started | Commands accepted | Rejected |
|---|---:|---:|---:|---:|---|---:|---:|
| brand_builder | 120 | 57 | 63 | 40 | 0/0 | 60547 | 0 |
| consolidator | 140 | 93 | 47 | 32 | 529/2077 | 21479 | 208 |
| cost_leader | 140 | 38 | 102 | 48 | 0/0 | 42062 | 0 |
| technologist | 120 | 76 | 44 | 41 | 0/0 | 7670 | 0 |
| vulture | 80 | 46 | 34 | 20 | 199/980 | 11518 | 200 |

Read with care: "Player controls nothing" means the investor ends the run without a company,
because it failed and was liquidated or, for a founder, was never founded.

## 3. Rival population by industry and archetype

Twenty guided five-year careers with no player policy, 600 rival firm-runs; rows with zero
dissolutions are omitted:

| Industry | Archetype | Firm-runs | Dissolved |
|---|---|---:|---:|
| construction | consolidator | 20 | 1 |
| construction | cost_leader | 20 | 7 |
| defense | vulture | 20 | 1 |
| extraction | consolidator | 20 | 2 |
| healthcare | technologist | 20 | 1 |
| manufacturing | technologist | 20 | 1 |
| retail | cost_leader | 20 | 2 |
| technology | vulture | 20 | 2 |

Total: 17 of 600 (2.8 percent). On `main` at `3814eca` the same census dissolved about 99 of
600, and every seed lost both agriculture firms, the construction cost leader and the energy
cost leader.

## 4. Outliers and exploits investigated

Defects found by these distributions and fixed before this run (each has fixtures):

1. **Agriculture extinct every run (rival AI).** Farms lose money in Q1/Q4 and earn it in Q2/Q3
   at reference prices. After one losing quarter the AI cut production to 60 percent, which
   kept most wages and fixed cost, lost the profitable season and spiralled. The rules order a
   cash squeeze as discretionary cuts, sale, funded credit, restructuring. Rivals now cut output
   only when the plant loses money per unit at full capacity or is demand-limited (revenue under
   half of capacity, or four weeks of output unsold), and otherwise borrow. Farm deaths went
   from 40/40 to 0/40.
2. **Energy cost leader always failed.** Profitable but bleeding cash through principal, and the
   AI only reacted to negative income. It now answers a cash squeeze directly. 20/20 to 0/20.
3. **Construction cost leader always failed.** It doubled capacity after an opening quarter of
   full order books. Rivals now wait a quarter, expand only into observed unmet demand, and
   contractors only when projects queue in the region. 20/20 to 7/20.
4. **Rival M&A never happened.** Every rival bid targeted a past-due firm, which the validator
   refuses, so rival takeovers were dead code on `main` (0 settled in every seed). Now a median of 12
   to 17 settle per five-year career.
5. **Money leaks in liquidation.** Asset-sale proceeds were minted, lenders were never paid and
   dissolved firms kept cash and live contracts. Replaced by a conserving waterfall (R25
   evidence, `tests/liquidation.rs`).
6. **Loan quote exploit.** `AcceptLoanQuote` let the borrower name its own rate, term and size
   (for example 0.01 percent over 10,000 weeks, above capacity). It now requires the quoted rate,
   a 13 to 260 week term and the borrower's credit capacity.
7. **Token-stake exploit.** One share in a rival froze its AI, and score control counted every
   company the investor held a share in. Both now use ultimate control.
8. **Conduct charged for rivals.** Rejected rival bids and rival wage failures reduced the
   player's conduct score. Now only the player's own record counts.
9. **Hidden information.** The rival takeover screen read private companies' income statements.
   It now uses public data only (`a_private_companys_books_do_not_reach_rival_decisions`).

Remaining outliers, judged and left as they are:

* **Founder acquirer, 17/100 lose the company.** The reference acquirer locks about a third of
  a fresh founder's cash into a tender in week 4, then borrows. A young firm that over-reaches
  can fail; 162 rejected `found_firm` attempts are the policy trying to re-found without enough
  capital. This is the policy's deliberate risk profile, not an engine defect.
* **Thirty-year growth policy, 17/20 end without a company.** The growth reference policy
  borrows every few weeks through year 17 and fails with four weeks past due; equity is
  correctly wiped out by the waterfall. The conservative policy keeps its company in 20/20.
  Leverage without a repayment rule is a real way to lose, and is left to human playtests to
  judge.
* **Cost leaders lose more often than other archetypes** (387 wins and 313 losses over five
  years, against 562 and 38 for technologists). Cost leaders are seeded into the low-margin
  primary industries; the gap is industry structure, now without deterministic extinction.
* Tukey outliers in the JSON are tails of the same distributions; there are no early stops,
  and no seed shows conservation or invariant failures.

## 5. Limits

Standard difficulty only. Easy and Hard need published rule sets first (CONTENT-PACING).
Reference policies are scripted. A balanced distribution is necessary but not sufficient: the
fun and fairness judgment belongs to R38.
