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
