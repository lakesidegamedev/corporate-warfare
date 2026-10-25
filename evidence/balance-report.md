# P33 balance distributions: first report

Date: 2026-10-25. Packet: P33. Requirement: R36 (automated gate).
Base commit: `30d1b0c` (`origin/main`), worktree `worktrees/balance` on `feat/balance`.
This is the first reproducible report, not an R36 sign-off: it covers one difficulty
(Standard), synthetic reference policies and no human playtest.

All numbers below come from the committed JSON in `evidence/balance/`. Nothing is typed by
hand except the dollar conversion of integer cents. Engine `0.1.0`, interface `1`, rules `1`,
dataset hash `8aa1dc570dd24b7e7207049c724e6a94bd8a7b683f0cc1a12be77f16c52cb489`.

## 1. What ran

Release build on the Contabo VPS (AMD EPYC, 18 vCPU, Linux 7.0, rustc 1.96.0). Runs were
sequential, one process at a time, with no other sweep running.

```sh
cd /root/projects/corporate-warfare/worktrees/balance
cargo build --release -p cw-cli
```

| # | Command (one line each) | Wall time | Runs |
|---|---|---|---|
| 1 | `./target/release/cw-cli sweep --policy conservative --scenario guided --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/guided-conservative-260.json` | 22.9 s | 100 |
| 2 | `./target/release/cw-cli sweep --policy growth --scenario guided --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/guided-growth-260.json` | 23.3 s | 100 |
| 3 | `./target/release/cw-cli sweep --policy acquirer --scenario guided --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/guided-acquirer-260.json` | 23.2 s | 100 |
| 4 | `./target/release/cw-cli sweep --policy conservative --scenario founder --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/founder-conservative-260.json` | 22.2 s | 100 |
| 5 | `./target/release/cw-cli sweep --policy growth --scenario founder --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/founder-growth-260.json` | 21.2 s | 100 |
| 6 | `./target/release/cw-cli sweep --policy acquirer --scenario founder --five-year --seeds 100 --weeks 260 --from 1 --json evidence/balance/founder-acquirer-260.json` | 22.6 s | 100 |
| 7 | `./target/release/cw-cli sweep --policy conservative --scenario guided --sandbox --seeds 20 --weeks 1560 --from 1 --json evidence/balance/guided-conservative-1560.json` | 23.4 s | 20 |
| 8 | `./target/release/cw-cli sweep --policy none --scenario guided --five-year --seeds 20 --weeks 260 --from 1 --json evidence/balance/guided-none-260.json` | 4.4 s | 20 |

Total wall time: 2 min 43 s. Seeds are consecutive ranges `1..100` and `1..20`; `--seeds N`
means "N consecutive seeds starting at `--from`". Every run reached its target week: there
were no early pauses, no session-creation errors and no invariant violations in any of the 640
runs.

Determinism spot check (same seed twice, release binary):

```
seed=7 ... acquirer hash=6214db9b715cd6ab613a5a8e9ea9443b00f51d68827bea6f1e78f57c69dd6c2d (twice)
seed=3 ... conservative 1560 hash=5400eed1e974b459f2654dcdc5ccb6930010c081c8ce6da7758c88e722383592 (twice)
```

The swept seed-3 acquirer hash also matches a direct `run` after the final build.

## 2. Harness

`cw-cli sweep` now takes:

```
--policy none|conservative|growth|acquirer   (default none)
--scenario guided|founder                    (--guided/--founder kept as aliases)
--five-year | --sandbox                      (default five-year)
--seeds N --from N --weeks N
--json PATH                                  (machine-readable report)
--verbose                                    (policy decisions and skip reasons)
```

Each JSON run row contains: seed, final week, paused reason, score total and components,
personal net worth, investor cash, controlled group size, dissolved count, past-due count,
weeks with past due, settled deals, canonical hash, policy attempt/accept/reject counts and
rejection reasons, and the conservation invariants. The report also contains min/median/max/mean
distributions, early-stop and Tukey outliers, dataset/engine versions and any errors.

Invariants checked on every final state: every firm and the investor ledger balanced, no
negative cash, no negative inventory lot, no negative facility deposit. A violation is printed
with the firm and product/facility; it is a defect, not a balance result.

"Controlled firms" in the JSON counts the controlled *group*, including subsidiaries of a
controlled firm, matching `ownership::group_value` and the score's control component. The
direct-only DTO is not used for this metric.

### Reference policies

All policies decide every four weeks (`CADENCE_WEEKS = 4`), at week 0 and then `tick % 4 == 0`,
while the clock is not paused. They read `Session::overview`/`Query::*` and two documented
projections already used by the UI (`mandatory_weekly_outflow` and secured/unsecured borrowing
capacity), then submit ordinary `CommandIntent`s through `Session::submit` with a distinct
command-id namespace. The common validator and solvency service accept or reject every action;
the policy never writes `World` state. A unit test asserts that every revision increment in a
policy turn comes from an accepted command.

* **Conservative operator.** Holds 13 weeks of mandatory payroll and debt service, runs the
  plant at 100% of capacity, spends 1% of trailing weekly revenue on marketing, nothing on
  research. It borrows only to bridge a genuine shortfall (cash below half the floor), only
  while liabilities stay below half of assets and the new principal below a quarter of assets.
* **Growth operator.** Holds 8 weeks of outflow, runs at 100%, spends 4% of weekly revenue on
  marketing and 2% on research. The thinner buffer is deliberate: a bad stretch becomes
  leverage rather than an output cut. The same credit gate applies.
* **Acquirer.** Holds 10 weeks of outflow, runs at 100%, spends 2% on marketing. When no deal
  is live and the reserve survives the escrow, it starts a funded diligence on the weakest
  operating peer (most negative trailing 13-week net income, or grade C/D), offering 110% of
  fair value or 115% of the exchange mark, whichever is higher, for a bare voting majority
  (`eligible / 2 + 1` shares). Acquired firms remain in the controlled group for the score.
* **Founder bootstrap (all three).** In the founder scenario there is no inherited firm. The
  policy founds a farm in region 035, named Lakeside Holdings, with $3.0M of paid-in capital
  ($2.0M plant plus $1.0M working capital) at week 0, then operates it as above. The bootstrap
  is identical across policies so the distribution isolates operating choices.

Policy iteration (harness, not content): the first draft throttled production to 85% (conservative)
and 95% (acquirer) when cash thinned. On this fixture that destroyed about $8.7k of contribution
a week against a demand-limited plant (week-10 seed 3: 100% target, 13-week net income
-$148,782.64; 85% target, -$262,000.70). The published policies hold full capacity and cut
discretionary marketing/research instead. The first draft also re-founded a firm inside the
guided scenario after a dissolution and borrowed every cadence into a debt spiral; the current
code gates founding on `ScenarioKind::Founder` and gates credit to a deep shortfall plus
leverage caps. Those were policy/harness defects, not content tuning.

## 3. Distributions

### 3.1 Guided, five-year (primary distribution)

100 seeds × 260 weeks for each reference policy; 20 seeds for the no-policy control. Net worth
is personal net worth in dollars; score is the integer five-year total.

| Policy | Seeds | Score min / median / max (mean) | Net worth min / median / max | Dissolved | Past due | Controlled | Weeks past due | Deals settled |
|---|---:|---|---:|---:|---:|---:|---:|
| none (control) | 20 | 62 / 62 / 62 (62) | $4,416,800 / $4,416,800 / $4,416,800 | 3 / 3 / 3 | 2 / 2 / 2 | 1 / 1 / 1 | 0 / 0 / 0 | 0 / 0 / 0 |
| conservative | 100 | 62 / 62 / 62 (62) | $4,344,800 / $4,344,800 / $4,344,800 | 3 / 3 / 3 | 2 / 2 / 2 | 1 / 1 / 1 | 0 / 0 / 0 | 0 / 0 / 0 |
| growth | 100 | 60 / 60 / 60 (60) | $4,032,800 / $4,032,800 / $4,032,800 | 3 / 3 / 3 | 2 / 2 / 2 | 1 / 1 / 1 | 0 / 0 / 0 | 0 / 0 / 0 |
| acquirer | 100 | 61 / 61 / 61 (61) | $4,032,800 / $4,038,800 / $4,044,800 | 3 / 3 / 3 | 2 / 2 / 2 | 2 / 2 / 2 | 0 / 0 / 0 | 1 / 1 / 1 |

Score components (constant within a policy except where noted):

| Policy | Return | Resilience | Control | Conduct | Total |
|---|---:|---:|---:|---:|---:|
| none | 50 | 100 | 20 | 100 | 62 |
| conservative | 50 | 100 | 19 | 100 | 62 |
| growth | 48 | 100 | 13 | 100 | 60 |
| acquirer | 48 | 100 | 25 | 100 | 61 |

The acquirer's mean net worth is $4,038,680. The spread is acquisition pricing, not a failure:
three seeds end at $4,032,800 (no net-of-premium gain) and one at $4,044,800; every seed settled
one deal and controls two firms.

All three policies exercised the legal-credit path. The opening $500,000 loan is fully amortised
by week 104, so the week-260 debt of $14,517.73 (conservative), $10,185.20 (growth) and
$13,581.38 (acquirer) is new policy borrowing against secured capacity, not the seeded loan.

### 3.2 Founder, five-year (secondary distribution)

Same 100 seeds, founder bootstrap. This scenario starts with the investor's $5.0M cash; the
policy converts $3.0M into a newly founded farm, so net worth and return are naturally lower
than the guided portfolio even when the firm is healthy.

| Policy | Seeds | Score min / median / max (mean) | Net worth min / median / max | Dissolved | Past due | Controlled | Weeks past due | Deals settled |
|---|---:|---|---:|---:|---:|---:|---:|
| conservative | 100 | 58 / 58 / 58 (58) | $3,145,000 / $3,145,000 / $3,145,000 | 5 / 5 / 5 | 2 / 2 / 2 | 1 / 1 / 1 | 0 / 0 / 0 | 0 / 0 / 0 |
| growth | 100 | 55 / 55 / 55 (55) | $2,625,000 / $2,635,000 / $2,635,000 | 5 / 5 / 5 | 2 / 2 / 2 | 1 / 1 / 1 | 0 / 0 / 0 | 0 / 0 / 0 |
| acquirer | 100 | 57 / 58 / 58 (57) | $2,595,000 / $2,625,000 / $2,635,000 | 5 / 5 / 5 | 2 / 2 / 2 | 2 / 2 / 2 | 0 / 0 / 0 | 1 / 1 / 1 |

Components: conservative 40/100/37/100; growth 38/100/24–25/100; acquirer 37–38/100/51–52/100
(total 57–58). The acquirer's higher control component (51–52) is the acquired media firm in
the controlled group; its lower return is the acquisition premium.

### 3.3 Thirty-year careers (sandbox, conservative)

20 seeds × 1,560 weeks, guided, sandbox. There is no five-year score in sandbox mode
(`score = null` in the JSON).

| Policy | Seeds | Final week | Net worth min / median / max | Dissolved | Past due | Controlled | Deals |
|---|---:|---:|---|---:|---:|---:|---:|
| conservative | 20 | 1,560 | $5,154,800 / $5,154,800 / $5,154,800 | 11 / 11 / 11 | 0 / 0 / 0 | 1 / 1 / 1 | 0 / 0 / 0 |

The guided firm survives all thirty years under the conservative policy, but eleven of thirty-one
rivals dissolve and no replacement entrants appear.

## 4. Outliers

No run stopped early: `final_week == weeks` and `paused = "five_year_score"` (or `null` in
sandbox) for all 640 runs. There was no failing tick, panic, negative cash, negative stock or
unbalanced ledger to report as a failure.

Statistical outliers (Tukey 1.5×IQR) are reported because the distributions are degenerate:
when the interquartile range is zero, any deviation is flagged. All are week-260 rows, not
failures:

| Sweep | Seed | Metric | Value | Week | Reading |
|---|---:|---|---:|---:|---|
| guided acquirer | 24, 68, 76 | net worth | $4,032,800 | 260 | Deal settled, but the premium/mark left no net gain; score still 61 |
| guided acquirer | 89 | net worth | $4,044,800 | 260 | Cheaper/favourable acquisition price; score still 61 |
| founder acquirer | 24, 68 | net worth | $2,595,000 | 260 | Same acquisition-price effect; score 57 instead of 58 |

No exploit sequence was found by the sweeps. The policies used only ordinary commands and the
validator rejected nothing in any published run (`accepted == attempted`), so there is no
rejected-command exploit evidence in this report.

## 5. Defects found and fixed

### D33-1: every facility consumed a deposit it did not have

`production::produce` unconditionally did
`facility.deposit_remaining.saturating_sub(output)`. `saturating_sub` on `MilliLots` clamps at
`i64::MIN`, not zero, so ordinary plants (deposit starts at zero) drifted deeply negative; on
seed 1 at week 260, 27 facility records were negative (for example Harvest Market Stores
-624,000,000 milli-lots). The field is only stocked for ore mines.

Fix: only a facility with a non-zero deposit consumes it, clamped at zero
(`crates/cw-sim/src/economy/production.rs`). Regression tests:
`only_ore_facilities_consume_a_deposit` and the 12-week `tick` conservation test now assert
every facility deposit is non-negative.

Before → after on seed 1 (`--policy none --weeks 260`): negative deposits 27 → 0; negative
inventory 0 → 0; ledgers balanced in both. The pinned canonical fixture hash moved
`5437029f…` → `3ef82f5b…`; the Rust and Swift constants were updated together. The iOS XCTest
was not run in this session, so R05 cross-platform equality is not re-signed here.

### D33-2: founder construction released the same escrow twice

`FoundFirm` moved the whole capex from restricted cash into work in progress and then created
the facility with `escrow_cents = full capex`. Completion released that escrow again, so a
founded firm carried work in progress and *negative* restricted cash forever. On seed 3,
founder conservative at week 260, the firm had $558,706 of cash but fair value **0**; work in
progress was $2.0M and restricted cash -$2.0M.

Fix: `FoundFirm` now mirrors `BuildFacility`: 40% construction service is paid from escrow into
work in progress and to contractors, 60% equipment stays in escrow until completion
(`crates/cw-sim/src/commands.rs`). Regression test
`a_founded_firm_capitalises_its_plant_once` asserts balanced books, zero restricted cash, zero
work in progress, capitalised fixed assets and a positive fair value.

Before → after (seed 3, founder conservative, week 260): fair value $0 → 117 cents per share ($1.17); score
50 → 58; personal net worth $1,975,000 → $3,145,000; work in progress $2.0M → $0; restricted
cash -$2.0M → $0.

### D33-3: reference-policy defects (harness, not content)

* The first draft's 85%/95% output cuts lost about $8.7k a week against a demand-limited plant,
  and the conservative firm dissolved around week 150. Full-capacity policies now survive the
  five-year horizon (conservative score 56 → 62; net worth $3,492,800 → $4,344,800). No content
  was changed.
* The first draft founded a replacement firm after a guided firm dissolved, spending $2.5M of
  investor cash. Founding is now allowed only in `ScenarioKind::Founder`.
* The first draft borrowed every cadence to top up a reserve, compounding debt service into the
  reserve. Credit is now limited to a deep shortfall with leverage caps.

## 6. Defects and limitations not fixed

These are root causes named by the sweep. Fixing them is a content/economy change, not a
mechanical defect, so P33 records them instead of tuning the score.

1. **Macro price drift is a no-op.** `crates/cw-sim/src/tick.rs::apply_macro_price_drift`
   literally does `let _ = index; let _ = dataset;`. The 200 BPS annual nominal price index
   accumulates in `macro_state`, but reference prices and demand never see it. Macro impulses
   therefore only move the policy rate. This is the main reason the 100 guided conservative and
   growth seeds are identical to the cent; acquirer variation comes from loan pricing and deal
   marks. Next: implement nominal drift with the rational weekly accumulator from
   CONTENT-PACING, then re-run this report.
2. **Optional event cards are drawn but never resolved.** The quarter check can open cards, but
   neither the rival AI nor the reference policies submit `RespondEvent`, so the cards have no
   economic effect. Seed-driven event variety does not reach the distribution. Next: an
   event-response policy fixture (separate from these reference players) and a seeded event
   distribution check.
3. **The starter farm is structurally loss-making at reference prices.** Week 10, seed 3,
   100% capacity: revenue 4,910,810 cents against cogs 1,987,826 + wages 3,584,400 + opex
   181,923 + interest 87,833 → EBIT -1,035,647 cents (about -$10.4k) per week. The guided firm
   stays solvent on its $1.0M opening cash, and its fair value is supported by the 40% book
   floor, not earnings. This is the P29/P35 economy question behind the flat return component;
   it was not tuned here.
4. **Score discrimination is low on this fixture.** Return is 50 for the control and
   conservative policies (ending net worth equals the seeded starting value), resilience and
   conduct are 100, and only control moves (13–25). A $12,000 acquirer net-worth spread still
   rounds to the same score of 61. The rounding is the documented rule; the low spread is a
   content/scoring follow-up.
5. **Long-run attrition without entry.** Eleven of thirty-one rivals dissolve over thirty years
   and no national entrant replaces them, so the sandbox market shrinks even while the guided
   firm compounds.

## 7. Limitations

* Synthetic, explainable policies, not human players. No win-rate claim is made; a 40–60%
  human win rate is not measurable from these runs.
* One difficulty (Standard) and one dataset hash. Easy/Hard do not exist yet per
  CONTENT-PACING ("begin with Standard only"), so "distributions across difficulty baselines"
  is not satisfied.
* Consecutive seed ranges 1–100 and 1–20, not an independent random sample.
* The five-year score exists only at week 260 in five-year mode; sandbox runs have no score.
* The policies are reference points, not an optimum. Changing their cadence, reserve or
  marketing constants will change the distributions; the constants are documented in
  `crates/cw-cli/src/policy.rs`.
* Linux release build only. No macOS/iOS XCTest, no hosted CI (GitHub Actions billing is still
  blocked on the host), no device/thermal measurement, no live database or host worldsim.
* The Swift canonical-hash constant was updated with the Rust constant but not re-verified on
  the Mac runner; R05 remains a platform gate.

## 8. Reproduction

The eight commands in section 1 reproduce every number. To inspect a single seed:

```sh
./target/release/cw-cli run --weeks 260 --seed 3 --guided --five-year --policy acquirer
./target/release/cw-cli diagnose --weeks 10 --seed 3 --guided --focus "Lakeside Holdings"
```

Committed evidence: `evidence/balance/guided-*.json`, `founder-*.json` (640 run rows in total).
Checks for this change: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.

## Post-calibration re-run, 2026-10-25 (after applied modifiers, macro regimes and training)

Release build, 100 guided five-year seeds per policy, 260 weeks, seeds 1-100. Commands:

```
./target/release/cw-cli sweep --seeds 100 --weeks 260 --guided --five-year --policy <policy>
```

| Policy | Score min/median/max | Net worth min/median/max | Dissolved median (min-max) | Controlled median (min) | Outliers |
|---|---|---|---|---|---|
| conservative | 56 / 62 / 62 | $3,492,800 / $4,344,800 / $4,344,800 | 5 (4-10) | 1 (0) | 23 |
| growth | 56 / 60 / 60 | $3,492,800 / $4,032,800 / $4,038,800 | 5 (4-10) | 1 (0) | 28 |
| acquirer | 56 / 61 / 66 | $3,492,800 / $4,038,800 / $4,044,800 | 5 (4-10) | 2 (0) | 37 |

Interpretation: the corridor stays populated (median five rival exits across five years) and most
careers end solvent. A minority of seeds (roughly five per hundred under the conservative policy)
lose the guided firm entirely: the starter farm's main buyers fail during a demand or supply
shock, contracts stop paying and the farm's thin margin cannot cover the gap. That is a real
failure mode, not a crash, and the investor keeps personal cash and the recovery route. The
reference policies are deliberately simple and do not replace a human player's pricing, buyer
diversification or expansion decisions.

Still open for R36: score dispersion is narrow for passive policies; the outlier flags are mostly
acquisition-price variants; no human playtest has judged whether the difficulty reads as fair.

## Post-finance re-run, 2026-10-25

After the interest-carry, board, merger, asset-purchase, bank-run and claim-timing changes:

```
./target/release/cw-cli sweep --seeds 100 --weeks 260 --guided --five-year --policy conservative
./target/release/cw-cli sweep --seeds 20 --weeks 1560 --from 1 --guided --sandbox --policy conservative
```

| Horizon | Runs | Score min/median/max | Dissolved median (min-max) | Controlled median (min) | Net worth min/median |
|---|---|---:|---:|---:|---:|
| 260 weeks | 100 | 56 / 62 / 62 | 5 (4-10) | 1 (0) | $3.49M / $4.34M |
| 1,560 weeks | 20 | n/a | 12 (10-17) | 0 (0) | $3.49M / $4.15M |

At five years the corridor is stable and most careers end solvent. Over thirty years the
conservative reference policy loses control of the guided firm in most seeds, keeps personal cash,
and does not found a replacement. That is the designed failure-and-recovery path, but the
reference policy does not exercise recovery, so it understates what an active player can do. R36
remains open: the score distribution is narrow for passive policies and no human has judged the
difficulty.

## Defense procurement re-run, 2026-10-25

After replacing the defense spot book with funded 52-week contracts, deposits and accepted
delivery settlement:

| Horizon | Runs | Score min/median/max | Dissolved median (min-max) | Controlled median (min) | Net worth min/median |
|---|---|---:|---:|---:|---:|
| 260 weeks | 100 | 56 / 62 / 62 | 5 (3-10) | 1 (0) | $3.49M / $4.34M |

Seed 3 (guided, conservative, 260 weeks) is back to four dissolved rivals, controlled firm in
control, score 61. The contract renewal path keeps defense demand open after the first 52 weeks
and is available to new producers.
