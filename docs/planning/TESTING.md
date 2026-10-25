# Testing and evidence policy

## Independent oracles

Test economic identities, conservation and command outcomes, not private implementation
shapes. Worked examples in ECONOMY are hand-computed oracles. Golden state hashes catch
change but cannot prove balance or arithmetic is correct. Never refresh them solely to
make CI green. Record changed metrics and explain each intentional baseline update.

| Layer | Required evidence |
|---|---|
| Numeric/ledger | Overflow/rounding boundaries; balanced postings; cash/profit bridge; capex/principal examples |
| Commands | Authority, invalid IDs, stale revision, idempotency, no partial mutation, exploit sequences |
| Physical economy | Inputs/outputs/shipments, finite workers/capacity/budgets, no negative stocks, service expiry |
| Ownership | Voting vs economic rights, indirect control, cycle rejection, minority treatment, deep iterative trees |
| Finance | Dilution, treasury treatment, coupons/maturity, liquidity/reserves, correlated claims, seniority |
| Integration | Supply chains, payroll-to-demand lag, M&A with debt/contracts, strikes, policy and insolvency |
| Determinism | Same seed/log hash on Linux/macOS/iOS; pause, save and batching do not change outcomes |
| Persistence | Atomic write failure, truncation/bit flip/future schema/oversize rejection, prior-version migration |
| FFI | Swift runtime round-trip, typed errors, stale artifact detection, concurrency ownership, DTO cost |
| UI | Real scenario paths, accessible controls, disabled reasons, small/large text, offline states |
| Device | Frame stalls, memory/thermal, lifecycle, cold/resume start, real TestFlight install |

Use property tests for quantities, ledger balance, ownership totals and arbitrary valid
command sequences. Include adversarial invalid sequences and targeted fuzzing of save
import and command parsing with bounded resources. A rejected command must retain the
same authoritative hash, except a separately documented rejection audit log.

## Scenario matrix

Maintain deterministic fixtures for: baseline 5-year founder; struggling-company tutorial;
vertically integrated group; deep/wide subsidiary tree; near-monopoly review; price war;
credit crunch/default cascade; worker shortage/strike; port/freight disruption; tech shock;
bank run; correlated insurance losses; hostile control loss; bankruptcy/rebuild; zero
sales/no cash; and a 30-year sandbox. Run representative seeds under at least three
rival archetype mixes. Every failure yields seed, build/data versions, failing tick,
command prefix and readable invariant diff.

Initial balance work: 100 seeds x 260 weeks across difficulty baselines, plus at least
20 x 1,560-week careers. These are this project's synthetic CLI scenarios, never the
host's AHD worldsim service and never the live database. Parameter sweeps are scheduled
heavy checks. Report ranges and outliers; a 40-60 percent player win rate is meaningless
without a defined policy and difficulty. Use several explainable scripted player policies,
not only the same AI policy being tested. Tune distributions after human feedback.

## Performance protocol

Separate engine compute from AI, report encoding, FFI transfer, view publication and
save I/O. Proposed initial A15-class targets: corridor tick p50 <= 8 ms, p99 <= 20 ms,
command-to-visible-result <= 100 ms for ordinary actions, steady memory <= 150 MB,
interactive cold start <= 2 seconds, no recurring main-thread stall above a frame budget.
These replace contradictory historical 4/8/12 ms promises as provisional measured goals.
Owner sees any relaxation of the player-facing instant-turn promise. Stress-world timing
is separately reported and cannot be hidden by timing a mostly empty world.

Representative fixtures: 30 firms/300 facilities/1,000 contracts; grown world 200 firms/
2,000 facilities/10,000 contracts; stress world 1,000 firms/10,000 facilities/50,000 contracts,
including 100 player-controlled firms. Include actual financial, logistics and AI work,
not merely empty entity records. Freeze fixture revision and scenario hashes. Reserve
scratch buffers, then report allocations for steady ticks and structural-event ticks
separately. No hard zero-allocation rule until justified by a profiled path.

Use release builds, warm-up, enough ticks for p99, fixed device/OS/toolchain, and repeat
at least three batches. Profile baseline and 10-minute fast-forward under normal thermal
conditions; report throttling and energy. Linux CI is a relative warning/gate against
same-run baseline comparisons with noise bounds; it is not an iPhone timing certificate.
A threshold breach fails with artifacts, not a quiet baseline reset. Define per-phase
budgets only after the first full representative pipeline is measured.

## CI and local execution

Rust/plan checks run on Linux. Apple changes and core integration run on macOS with
real generated bindings, simulator execution and unsigned device archive. Bootstrap
compile-only Apple smoke is not the later gameplay gate. Add XCTest/XCUITest once
sessions and fixtures exist. Require a single aggregate verification job once the
required platform checks are reliable. Protect default branch where account plan allows;
record unavailable enforcement honestly, never claim protection from a failed API call.

Full tests/builds on this shared VPS normally use `/root/bin/lakeside-check-queue`.
Queue once, inspect terminal success/failure/cancellation/timeouts, and retain output.
Use targeted checks while iterating. Do not install services, fetch live game databases
or run a full check concurrently outside the scheduler.

## Evidence record

For every requirement marked done, attach a local reviewed evidence JSON with requirement
ID, tested commit, verification date, command/procedure, result, artifact references and
limitations. Automated tests cannot sign off a human/device gate. Link hosted CI runs and
release build numbers. Keep secrets and private personal data out of artifacts.
`plan_check.py` validates completeness of records; it does not authenticate their truth.
Final acceptance includes manual review of evidence, never only JSON state changes.
