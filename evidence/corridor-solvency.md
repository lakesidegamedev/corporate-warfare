# Corridor solvency measurement, 2026-10-25

Measured on main after the P25 production-accounting fixes (`8674b93`).

Command: `cargo run -q -p cw-cli -- run --weeks 52 --seed 3` (guided, five-year defaults).
Result: `firms=31 controlled=0 past_due=0 dissolved=23`; the guided player firm failed and the
investor kept only personal cash (`net_worth=349280000`, `firm_cash=0`).

Experiment (not merged): funding the outside gateway account and using its finite buy side for
leftover local stock produced `dissolved=26`, worse than baseline, because firms exported inputs
they still needed. The change was discarded.

Interpretation: the failures are not a test artefact. After inputs and wages were charged
correctly, several seeded industries have no paying buyer in their own region (media advertising,
construction contractor service, some out-of-region farms and component works), local household
demand is smaller than seeded capacity, and the corridor has no working export channel. Cash then
drains through wages, maintenance and rent until the four-week grace expires.

Next investigation, in order:
1. Add a `cw-cli inspect --firm N` diagnostic printing cash, output, sold lots, revenue, input
   cover, staff ratio and bottleneck per week. Do not guess from aggregate counts.
2. Decide and test the export rule: gateway buy side limited to surplus above a firm's four-week
   input cover and never below variable cost; inter-region spot offers with freight and lead time.
3. Wire marketing budgets to media advertising demand and construction projects to contractor
   service/equipment supply.
4. Rebalance seeded capacity against regional demand only after the flows exist; record every
   parameter change in DECISIONS with before/after distributions.
5. Re-run 100 five-year seeds and 20 thirty-year careers (P33) and require explainable failures.

## Second measurement, same day (after the viability pass)

Seed 3, guided, 52 weeks: `firms=31 controlled=1 past_due=6 dissolved=2` (was 23 dissolved).
Seed 3, guided, 260 weeks: 16 dissolved, 0 past due, the guided firm in restructuring with zero
cash and a fair value of 63 cents per share.

Weekly industry report at week 52 shows the remaining structural losses: retail -117k (imported
goods at the gateway premium and full payroll at partial utilisation), media -30k (marketing
demand is 1 percent of corridor revenue), carriers -14k, energy -37k, chemicals -27k, defense
-22k. Profitable: agriculture +23k, manufacturing +27k, healthcare +42k, real estate +43k,
technology +53k, telecommunications +52k.

Next pass: solve output prices as cost-plus at realistic utilisation, and make media/carrier
demand scale with corridor activity. Do not tune by loosening the pass condition.

## Third measurement, 2026-10-25 (viability achieved at baseline)

After the calibration described in DECISIONS.md:

- 52 weeks, seed 3: `dissolved=0 past_due=0`, guided firm operating with $783,775 cash and 60
  percent of votes.
- 260 weeks: `dissolved=3 past_due=2`, guided firm operating with $935,232 cash, fair value 154
  cents, score 62 (return 50, resilience 100, control 20, conduct 100).
- 1,560 weeks (sandbox): `dissolved=11 past_due=0`, guided firm operating with $3.53M cash and
  fair value 353 cents (2.7x the opening mark).

This retires the blocker. What remains is balance breadth (P33) and human playtesting, not a
collapsing corridor. Eleven rival exits over thirty years with no entry is a known limitation.
