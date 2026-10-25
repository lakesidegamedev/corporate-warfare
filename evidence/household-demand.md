# Household disposable income and the savings supplement

Rule source: `docs/rules/MARKETS.md`. The weekly household budget is ninety percent of
disposable wage income; the remaining ten percent becomes savings. Initial savings are
thirteen weeks of baseline disposable income. After a wage shortfall, at most one percent of
savings may supplement that week's budget; savings cannot go negative. Unspent category
budgets return to savings.

## Change

- The opening baseline is now disposable income after the twenty percent labour tax, so
  baseline lots, initial savings and the shortfall comparison use the same net concept.
- A weekly budget helper applies the 90 percent propensity, compares disposable income with
  the stored disposable baseline, and draws at most `savings / 100` when income is short.
  The supplement is deducted when committed; unspent category budgets return after clearing.
- Old saves without the baseline field default to zero and receive no supplement rather than
  inventing a baseline.

## Evidence

- `markets::tests::a_wage_shortfall_can_draw_one_percent_of_savings` checks the cap, the
  deduction, the no-shortfall path and zero savings.
- `markets::tests::a_week_with_no_wages_spends_at_most_one_percent_of_savings` seeds a
  zero-income week with one million cents of savings and checks that at least 990,000 remains.
- The five-year and thirty-year conservative distributions are unchanged after the change:
  score 56/62/62, dissolved median 5, controlled median 1; thirty-year dissolved median 10,
  controlled median 1. Seed 3 remains four dissolved rivals and score 61.
- Canonical fixture hash: `f75e42c2e9f6322670f05e286ae2cedba227076acb8956f95d2051aa3d8300b3`.

## Limits

- Households are regional aggregates, not individual balance sheets. The supplement and
  savings are aggregate regional values.
- Pension income is not modelled separately; transfer support is added to income before the
  spend budget and is not taxed.
