# Labour income tax and trailing fiscal budgets

Rule sources: `docs/rules/CONTENT-PACING.md` (taxable labour income pays twenty percent;
opening treasury is thirteen weeks of baseline tax receipts; weekly procurement, healthcare
and training budgets are eighty percent of the trailing thirteen-week mean of receipts,
split fifty/forty/ten) and `docs/rules/CORPORATE.md` (quarterly corporate tax).

## Model

- Each week payroll records the wages each firm actually paid, attributed to the regions that
  supplied the labour. Household income is paid named wages plus background employment, not
  gross contractual wages. Twenty percent is posted to the government as cash and revenue
  (`labour income tax`); households receive the remaining eighty percent. Deferred wages are a
  liability, not household cash, and are not taxed. Transfer support is added after tax and is
  not itself taxed.
- Corporate tax already accrues weekly and is paid at week multiples of thirteen. Each
  actual payment is added to the same weekly receipt total.
- At the close of the week the total is appended to a bounded thirteen-week history. The
  three budgets are recomputed as eighty percent of the window mean, split fifty/forty/ten.
  Unspent budget is not carried forward. A week with no receipts lowers the following
  budgets honestly rather than holding a fabricated baseline.
- The seed history is thirteen copies of the baseline receipt estimate, so week one starts
  from the prescribed opening position and then follows actual receipts.

## Evidence

- `fiscal::tests::budgets_are_eighty_percent_of_the_trailing_thirteen_week_mean` checks the
  split and window bound.
- `fiscal::tests::receipts_outside_the_window_are_forgotten` checks that old receipts leave
  the window and budgets fall to zero when nothing is received.
- `tick::tests::labour_income_tax_is_withheld_and_budgeted` runs a real week, recomputes
  paid named plus background labour income, and checks the treasury receipt is exactly
  twenty percent and all three budgets are positive.
- `tick::tests::deferred_wages_are_not_household_income_or_tax` drains an employer's cash,
  checks the firm defers payroll, and checks the treasury receives tax only on paid wages.
- The healthcare and defense industry payer tests now reconcile government cash with the
  labour tax receipt as well as the spending leg.

Distributions after the change:

| Horizon | Runs | Score min/median/max | Dissolved median (min-max) | Controlled median (min) | Net worth min/median |
|---|---|---:|---:|---:|---:|
| 260 weeks | 100 | 56 / 62 / 62 | 5 (3-10) | 1 (0) | $3.49M / $4.34M |
| 1,560 weeks | 20 | n/a | 10 (6-14) | 1 (0) | $3.49M / $4.83M |

Seed 3 remains at four dissolved rivals, controlled firm in control and score 61. The
thirty-year conservative policy now keeps control in the median seed instead of losing it.
The canonical fixture hash is `0f6b4a862704ec11d546254abd69ef1421196633ce97c577a7623228cb07f840`.

## Known limits

- Withholding is a regional aggregate: paid wages are attributed pro rata to the regions of
  the employing facilities, not to individual workers. Severance is not included in the
  labour-tax base.
- Background transfer support still draws from the training budget and is capped by it; the
  rule says remaining fiscal budget. This is a separate pre-existing fiscal gap.
- The demand quantity uses fixed baseline lots with the category budget as a ceiling. When
  supply is the binding constraint, lower disposable income increases unspent savings rather
  than visibly reducing quantities; that is consistent with the current one-pass market but
  should be reviewed when household demand is next calibrated.
