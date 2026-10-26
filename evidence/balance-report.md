# Balance distributions: 1.2.0 report

Date: 2026-10-26. Requirement: R36 (automated gate). Tested commit: `6a36dcb` on
`feat/regions`, before the 1.2.0 version bump, which changes no behaviour; the sweep files
still report engine `1.1.0`. Dataset hash `5112c949d0a417a96326a0bb1482793b9d75aa83940e7664d3861e2007082e1b`. Standard difficulty only; scripted reference
policies, not human play. Not a fun sign-off: R38 independent playtests remain. Earlier
reports are in git history at tags `v1.0.0` and `v1.1.0`.

Every number comes from the committed JSON in `evidence/balance/` or from
`cargo run --release -p cw-session --example dissolution_census`, each queued through the
host check scheduler one at a time.

## 1. What ran

The 1.1.0 procedure plus the new `expander` policy, which every twelve weeks builds the plant
whose three years of contribution at reference prices best repay its full cost, in any sector.

```sh
cargo run --locked --release -p cw-cli -- sweep --policy <conservative|growth|acquirer|expander> --scenario <guided|founder> --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/<scenario>-<policy>-260.json
cargo run --locked --release -p cw-cli -- sweep --policy <conservative|growth|acquirer|expander> --scenario guided --sandbox --seeds 20 --weeks 1560 --from 1 --json evidence/balance/guided-<policy>-1560.json
cargo run --locked --release -p cw-cli -- sweep --policy none --scenario guided --five-year --seeds 20 --weeks 260 --from 1 --json evidence/balance/guided-none-260.json
```

## 2. Distributions

Score and investor net worth are min / median / max. "Company lost" counts runs that end
controlling no company.

| Run | Weeks | Seeds | Score | Investor net worth | Rivals dissolved (median) | Company lost | Errors |
|---|---|---|---|---|---|---|---|
| guided conservative | 260 | 100 | 32 / 35 / 36 | $4.35M / $4.77M / $4.86M | 1 | 0 | 0 |
| guided growth | 260 | 100 | 24 / 32 / 33 | $3.79M / $4.39M / $4.47M | 1 | 1 | 0 |
| guided acquirer | 260 | 100 | 27 / 43 / 44 | $4.14M / $4.47M / $4.57M | 1 | 0 | 0 |
| guided expander | 260 | 100 | 43 / 59 / 66 | $4.45M / $4.60M / $5.11M | 1 | 0 | 0 |
| founder conservative | 260 | 100 | 28 / 29 / 30 | $4.62M / $4.71M / $4.84M | 1 | 0 | 0 |
| founder growth | 260 | 100 | 27 / 28 / 29 | $4.50M / $4.58M / $4.72M | 1 | 0 | 0 |
| founder acquirer | 260 | 100 | 15 / 32 / 32 | $2.66M / $2.75M / $2.88M | 1 | 0 | 0 |
| founder expander | 260 | 100 | 14 / 32 / 76 | $2.46M / $3.19M / $6.76M | 1 | 0 | 0 |
| guided none | 260 | 20 | 23 / 36 / 36 | $3.94M / $4.85M / $4.89M | 1 | 0 | 0 |
| guided conservative | 1560 | 20 | n/a | $4.22M / $7.62M / $7.91M | 9 | 2 | 0 |
| guided growth | 1560 | 20 | n/a | $4.02M / $5.16M / $5.61M | 10 | 4 | 0 |
| guided acquirer | 1560 | 20 | n/a | $4.57M / $6.91M / $7.40M | 9 | 1 | 0 |
| guided expander | 1560 | 20 | n/a | $4.09M / $5.03M / $17.08M | 10 | 13 | 0 |

## 3. The score now rewards building

The 1.1.0 finding (doing nothing scored like active play) is closed. The rebased score
(growth, expansion and leadership over the opening position) puts the no-action career at a
median of 36 and the expander at 59 (43 to 66); the acquirer reaches 43. Policies that never
open a second division stay in the low 30s.

## 4. Rival population

Five-year guided careers with no player policy, 20 seeds, 30 rivals each:

```
agriculture            brand_builder  firms= 20 dissolved=  0
agriculture            cost_leader    firms= 20 dissolved=  1
chemical_industries    brand_builder  firms= 20 dissolved=  0
chemical_industries    consolidator   firms= 20 dissolved=  0
construction           consolidator   firms= 20 dissolved=  6
construction           cost_leader    firms= 20 dissolved=  3
defense                technologist   firms= 20 dissolved=  0
defense                vulture        firms= 20 dissolved=  0
energy                 cost_leader    firms= 20 dissolved=  0
energy                 vulture        firms= 20 dissolved=  0
extraction             consolidator   firms= 20 dissolved=  1
extraction             technologist   firms= 20 dissolved=  1
financial              consolidator   firms= 20 dissolved=  0
financial              cost_leader    firms= 20 dissolved=  0
healthcare             brand_builder  firms= 20 dissolved=  1
healthcare             technologist   firms= 20 dissolved=  0
logistics              brand_builder  firms= 20 dissolved=  0
logistics              vulture        firms= 20 dissolved=  0
manufacturing          cost_leader    firms= 20 dissolved=  0
manufacturing          technologist   firms= 20 dissolved=  5
media                  brand_builder  firms= 20 dissolved=  0
media                  consolidator   firms= 20 dissolved=  0
real_estate            brand_builder  firms= 20 dissolved=  1
real_estate            consolidator   firms= 20 dissolved=  0
retail                 consolidator   firms= 20 dissolved=  0
retail                 cost_leader    firms= 20 dissolved=  1
technology             technologist   firms= 20 dissolved=  2
technology             vulture        firms= 20 dissolved=  0
telecommunications     cost_leader    firms= 20 dissolved=  0
telecommunications     technologist   firms= 20 dissolved=  0
```

22 of 600 rival firm-runs dissolve (3.7 percent), against 8 in the 1.1.0 report. Construction
is the weakest sector (9 of 40) and the manufacturing technologist fails in 5 of 20.

## 5. Defects this gate found and fixed

- Corporate tax had no loss carryforward, so the seasonal starting farm paid tax on its summers
  and was never credited for its winters: the guided company lost money every year.
- Sprawl counted plant sites, not divisions, unlike A House Divided's per-sector rule.
- Plants were sized at millions against a million-dollar start; a plant is now a fifth of its
  1.1.0 unit with calibrated margins kept.

## 6. Open findings and limits

- **A naive expander loses its company over thirty years** in 13 of 20 runs. Every scripted
  policy holds production at full capacity whatever sells; the expander's later plants add
  about $9,000 a week of wages each while revenue stays flat, until cash runs out. A player
  sees utilisation and the limiting factor per plant and can mothball or retool. The policies
  need a sell-through rule before this run proves the long game.
- Rival dissolutions rose to 3.7 percent, concentrated in construction.
- The founder acquirer and founder expander end with lower net worth than the founder
  conservative policy; the founder pays its $3,000,000 capital in at the start.
- Standard difficulty only.
