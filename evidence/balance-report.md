# Balance distributions: 1.1.0 report

Date: 2026-10-26. Requirement: R36 (automated gate). Tested commit: `6586ab6` on
`feat/states-and-map`, before the 1.1.0 version bump, which changes no behaviour. The sweep
files therefore still report engine `1.0.0`. Interface `1`, rules `1`, dataset hash
`5ef0b7630807aae3ecfad02c49dfa0c67646532c42b637ab5ff682435e4c1be1`. Standard difficulty only; scripted reference policies, not human play. This is
not a fun sign-off: R38 independent playtests remain. The 1.0.0 report is in git history at
tag `v1.0.0`.

Every number comes from the committed JSON in `evidence/balance/` or from
`cargo run --release -p cw-session --example dissolution_census`.

## 1. What ran

The same procedure as 1.0.0, each command queued through the host check scheduler one at a
time (about 25 seconds per sweep on the Contabo VPS):

```sh
cargo run --locked --release -p cw-cli -- sweep --policy <conservative|growth|acquirer> --scenario <guided|founder> --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/<scenario>-<policy>-260.json
cargo run --locked --release -p cw-cli -- sweep --policy <conservative|growth|acquirer> --scenario guided --sandbox --seeds 20 --weeks 1560 --from 1 --json evidence/balance/guided-<policy>-1560.json
cargo run --locked --release -p cw-cli -- sweep --policy none --scenario guided --five-year --seeds 20 --weeks 260 --from 1 --json evidence/balance/guided-none-260.json
```

## 2. Distributions

Score and investor net worth are min / median / max. Dissolved counts rival firms per run.

| Run | Weeks | Seeds | Score | Investor net worth | Rivals dissolved, median (range) | Errors | Outliers |
|---|---|---|---|---|---|---|---|
| guided conservative | 260 | 100 | 63 / 64 / 64 | $4.63M / $4.81M / $4.84M | 0 (0 to 2) | 0 | 9 |
| guided growth | 260 | 100 | 61 / 62 / 62 | $4.23M / $4.41M / $4.43M | 0 (0 to 2) | 0 | 12 |
| guided acquirer | 260 | 100 | 62 / 63 / 63 | $4.33M / $4.51M / $4.54M | 0 (0 to 2) | 0 | 4 |
| founder conservative | 260 | 100 | 60 / 61 / 62 | $3.62M / $3.94M / $4.17M | 0 (0 to 2) | 0 | 22 |
| founder growth | 260 | 100 | 57 / 59 / 60 | $3.02M / $3.33M / $3.54M | 0 (0 to 2) | 0 | 7 |
| founder acquirer | 260 | 100 | 57 / 59 / 60 | $3.10M / $3.44M / $3.65M | 0 (0 to 2) | 0 | 27 |
| guided none | 260 | 20 | 46 / 64 / 65 | $3.86M / $4.88M / $4.91M | 0 (0 to 2) | 0 | 2 |
| guided conservative | 1560 | 20 | n/a | $3.49M / $7.78M / $8.80M | 8 (4 to 14) | 0 | 4 |
| guided growth | 1560 | 20 | n/a | $3.49M / $5.45M / $5.69M | 8 (4 to 11) | 0 | 4 |
| guided acquirer | 1560 | 20 | n/a | $3.58M / $6.98M / $9.23M | 7 (4 to 11) | 0 | 5 |

Against 1.0.0 every five-year median score is two to six points higher and five-year
dissolutions fell from a median of one to zero. Thirty-year careers keep more wealth (guided
conservative median $7.78M against $4.25M) and lose slightly fewer rivals (seven or eight of
thirty against nine or ten).

## 3. Rival population by industry and archetype

Five-year guided careers with no player policy, 20 seeds, 30 rivals each:

```
    agriculture            brand_builder  firms= 20 dissolved=  0
    agriculture            cost_leader    firms= 20 dissolved=  0
    chemical_industries    brand_builder  firms= 20 dissolved=  0
    chemical_industries    consolidator   firms= 20 dissolved=  0
    construction           consolidator   firms= 20 dissolved=  1
    construction           cost_leader    firms= 20 dissolved=  0
    defense                technologist   firms= 20 dissolved=  0
    defense                vulture        firms= 20 dissolved=  2
    energy                 cost_leader    firms= 20 dissolved=  0
    energy                 vulture        firms= 20 dissolved=  0
    extraction             consolidator   firms= 20 dissolved=  0
    extraction             technologist   firms= 20 dissolved=  1
    financial              consolidator   firms= 20 dissolved=  0
    financial              cost_leader    firms= 20 dissolved=  0
    healthcare             brand_builder  firms= 20 dissolved=  0
    healthcare             technologist   firms= 20 dissolved=  0
    logistics              brand_builder  firms= 20 dissolved=  0
    logistics              vulture        firms= 20 dissolved=  0
    manufacturing          cost_leader    firms= 20 dissolved=  0
    manufacturing          technologist   firms= 20 dissolved=  1
    media                  brand_builder  firms= 20 dissolved=  0
    media                  consolidator   firms= 20 dissolved=  0
    real_estate            brand_builder  firms= 20 dissolved=  0
    real_estate            consolidator   firms= 20 dissolved=  0
    retail                 consolidator   firms= 20 dissolved=  0
    retail                 cost_leader    firms= 20 dissolved=  0
    technology             technologist   firms= 20 dissolved=  0
    technology             vulture        firms= 20 dissolved=  3
    telecommunications     cost_leader    firms= 20 dissolved=  0
    telecommunications     technologist   firms= 20 dissolved=  0
```

Eight of 600 rival firm-runs dissolve (1.3 percent), against 17 in the 1.0.0 report. No
industry or archetype dies out; the largest is technology vulture at 3 of 20.

## 4. Defects found by this gate and fixed before the numbers above

The first 1.1.0 censuses read 25 and then 37 dissolutions. Each rise traced to an engine
defect, not a tuning lever:

- Unit sales were overwritten on every fill and read from the open week, which is empty
  between ticks, so rivals never repriced.
- Every selling path admitted only firms headquartered in the state, so a plant built in
  another state could not sell. Business offers also sold a plant's own input stock.
- Vehicle Assembly was priced for a 12 percent margin and lost money on Michigan wages at
  gateway prices (Wolverine Motor Works dissolved in 14 of 20 seeds).
- Rivals raised prices toward twice reference whenever the gateway or public works bought
  their whole output. Prices now stop at import parity, or 15 percent over reference for
  services.
- The outside economy settled through a $50 million account that the corridor's export
  surplus emptied in about two years, after which construction firms failed together. It
  now opens at $500 million and draws at most one percent a week.

## 5. Open findings and limits

- **A no-action career scores the same as active play.** Guided no-action median 64 against
  conservative 64, growth 62 and acquirer 63. This was already true in 1.0.0 (62 against
  62). The score rewards preserving the opening position more than the reference policies
  improve it. It needs an owner decision on scoring or a stronger reference policy, not a
  silent retune.
- Score spreads are narrow (two or three points inside a policy), so Tukey outliers flag
  runs one point from the median. None is an exploit.
- Standard difficulty only. Reference policies are scripted; the acquirer founder policy is
  deliberately aggressive.
