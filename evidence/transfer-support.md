# Background transfer support

Rule source: `docs/rules/CONTENT-PACING.md` (background household transfer support is the
minimum of the remaining fiscal budget and thirty percent of the baseline wages of the
unemployed, paid only from treasury; report unpaid needs and fiscal stress).

## Change

- The need is now computed for every skill tier from that tier's baseline wage, rather than
  applying the low-tier wage to all unemployed workers.
- The treasury and the residual training/welfare budget still cap actual payment. Any
  difference is accumulated in `government.unpaid_transfer_need_cents` instead of being
  invented as income or silently dropped.
- The field is reset every fiscal week before the regions are processed.

## Evidence

- `tick::tests::unfunded_transfer_support_is_reported` sets employment to zero in one region,
  sets the residual budget to zero, runs the household income pass, and checks that the unmet
  need is positive and the books stay balanced.
- Seed 3 remains at four dissolved rivals and score 61; the 100-seed conservative five-year
  distribution is unchanged at score 56/62/62, dissolved median 5, controlled median 1.
- Canonical fixture hash: `6731ed7cf42d0a410e94b55f3aefad6fbe51453cbe3a6803548fa7edbfb633ad`.

## Limits

- In normal clearing the background employer absorbs the residual workforce, so recorded
  unemployment is usually zero and the support payment is usually zero. The mechanism now
  reports correctly when a region does have unemployed workers.
- The cap still uses the training budget as the residual welfare envelope. The rules say
  "remaining fiscal budget"; splitting welfare from training is a separate product decision.
