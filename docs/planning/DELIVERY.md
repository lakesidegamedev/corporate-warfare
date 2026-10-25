# Delivery graph and work packets

Every packet ends with reviewed code, tests, artifacts, an updated requirements ledger,
a clean commit and a concrete next move. A dependency is not satisfied by a stub.
Do not implement all of a milestone as one enormous change. Split the packets below
into one coherent behaviour change per commit, retaining their acceptance criteria.

## Milestone 0: build path and economic proof

P00. Repository/bootstrap: private remote, pinned Rust/UniFFI, Linux checks, actual Swift
probe, fresh XCFramework, simulator compile and unsigned device archive. Record what
ran and what did not. Add a simulator runtime test before calling R31 complete.

P01. Geography: recover official crosswalk, freeze actual corridor IDs, print a review
sheet. Implement validation and provenance before downloading every economic source.
R01. Independent of ledger work. If blocked, label engineering fixtures explicitly.

P02. Units and ledger: money/rates/quantity types, checked arithmetic, postings, account
balances, hand-calculated fixtures. R03. Define stocks/flows and weekly calendar.

P03. Entities and commands: logical IDs, deletion/indirection, actor authority, atomic
reservation/commit, idempotency. R04. Include invalid-command adversarial tests.

P04. Session/replay/persistence skeleton: deterministic RNG and canonical hash, one serial
mutation path, log metadata, save envelope, corruption/atomic recovery fixtures.
R05/R30 begin; do not mark cross-platform determinism done without iOS runtime evidence.

Exit: packaging works, one command survives Swift/save/load, accounting has an oracle,
and geography is real or a named dependency. No fake five-year P&L from bootstrap code.

## Milestone 1: one honest operating economy on the phone

P05. Historical data: source manifest, era coverage research, FIPS/classification
crosswalks, imputation report and reproducible transformed data. R02.

P06. Background economy: households, firms outside named actors, government/gateways,
finite budgets and payroll feedback. R06.

P07. Shared labour and one production chain: staffing, wage offers, finite inputs,
production, quantities/services and operating expense settlement. R07/R08.

P08. Market clearing: written allocation examples, budget-constrained demand, stock
reservation, tie/rounding logic, actual sales/revenue. R09.

P09. Logistics and capital: contracts, transit, freight capacity, facility lifecycle,
CIP, build queues and cancellation. R10/R11.

P10. Early SwiftUI: start, dashboard, cash bridge, one facility, one market, run/step,
save/resume and why-results-changed. R31/R32 begin with real engine fixtures.

Exit: a small chain runs profitably or fails for explainable reasons, including on
simulator; cash, goods and workers reconcile. This is the first fun/clarity checkpoint.

## Milestone 2: investor, group and capital markets

P11. Persistent investor and firm formation/recovery. R12.
P12. Ownership/control graph and consolidated reporting. R13.
P13. Intercompany trade, loans, dividends and guarantees. R14.
P14. Public/private exchange, finite liquidity and price impact. R15.
P15. Issuance/buybacks/dividends/IPO and dilution previews. R16.
P16. Boards, CEO authority and control changes. R17.
P17. Loans, bonds, credit/covenants and external counterparties. R18.
P18. Insolvency, restructuring, claims and investor continuity. R25.

Exit: a player can build a nested group, finance expansion, lose control or fail a
subsidiary, and continue. Group P&L has no internal-sales wealth exploit.

## Milestone 3: corporate warfare and complete industry coverage

P19. Banking balance sheets/reserves/default/run scenarios. R19.
P20. Insurance underwriting/reserves/claims/catastrophe scenarios. R20.
P21. M&A: diligence, offers/escrow/tender, asset vs share transaction, minority/debt
settlement. R21.
P22. Competition review and remedies by control group/region/product. R22.
P23. Unions, bargaining, strikes and settlements. R23.
P24. Lobbying requests and bounded policy/reputation effects. R24.
P25. Extend goods, services, infrastructure, extraction and financial businesses to all
fifteen industries. Complete matrix, one worked industry scenario per row. R26.
P26. Brand, R&D, quality and obsolescence, with credible long-run bounds. R27.

Exit: every named corporate system affects other systems and has a UI route planned.
Do not leave financial/defense/media industries as generic factory labels.

## Milestone 4: credible rivals and a complete game

P27. Observation fog, fair common commands, archetypes, deterministic operation budget,
urgent response, long-run diagnostics. R28.
P28. Event validation/content, meaningful rivals, founder/guided starts. R29.
P29. Five-year score and endless continuation; thirty-year career fixtures. R34.
P30. Phone corporate workflows, previews, deadlines and explanation drill-downs. R33.
P31. Onboarding, accessible design, final copy and coherent assets. R35/R39.

Exit: a stranger can complete the full experience without CLI or debug assistance.
No debug screen or seeded data shortcut may stand in for a release workflow.

## Milestone 5: reliability, balance and TestFlight

P32. Adversarial sequence testing, persistence migrations and lifecycle fault injection.
Complete R05/R30 across platforms; run all ledger/control/exploit fixtures.
P33. Balance distributions, scripted reference policies, stress and thirty-year careers.
R36. Fix actual outliers; do not only loosen bounds.
P34. Device Instruments/profiling, thermal/fast-forward, memory/launch, DTO cost, lifecycle
and accessibility on target hardware. R37.
P35. Three independent playtests; record confusion and release defects, fix and verify.
R38. A missing human is a real blocker to that gate, not permission to invent evidence.
P36. Clean archive, privacy/assets review, signing/export and symbols. R39/R40.
P37. Explicit release upload, bounded Apple processing checks, intended tester group.
R41. Keep invalid/processing/available states distinct.
P38. Actual iPhone TestFlight acceptance route and final defect triage. R42.

Exit: RELEASE_ACCEPTED in PRODUCT. If Apple-only owner actions are pending, report
SOFTWARE_READY with exact unmet gates; do not claim the full goal complete.

## Parallelism and ownership

Geography/source research and accounting can proceed independently. Later economic rules
must follow dependencies in requirements.json. UI can use stable real-engine fixtures
while rules grow. Apple setup should run early, never as the final surprise. Check work
must use the host scheduler. Do not launch agent swarms or external providers unless
requested or required by an applicable skill. A requested DeepSeek provider must remain
that provider with provenance, not a renamed assistant.

## Scope control

New insights can refine implementation without asking about every routine choice.
Changes to player role, era, offline requirement, release systems, pricing or target
platform require a recorded owner decision. Challenge defaults with evidence, not
silent deletions. A slipping packet is split into smaller tasks; its acceptance cannot
be removed to make progress appear complete.
