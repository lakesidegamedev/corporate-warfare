# Plan audit

Date: 2026-10-25. Status: initial audit, owner interview in progress.
Scope: audit of `../PLAN.md`, not acceptance of its locked decisions.
The original plan remains preserved. Research citations live in `../RESEARCH.md`.

## Verdict

Keep the offline simulation boundary, regional competition, explicit accounting,
seeded replay and early headless harness. Replace the milestone structure before
handing this to an autonomous implementation agent. The document currently specifies
an interesting prototype, not an implementable and verifiable iOS release.

## Findings

| ID | Severity | Finding | Required resolution |
|---|---|---|---|
| A01 | Blocking | The corridor lists cities as if each were an independent BEA economic area. No canonical IDs or county membership establish that claim. | Pin actual geography and vintage; generate a deduplicated list from a primary crosswalk before setting a region count. |
| A02 | Blocking | The player identity, ownership hierarchy, start era and end condition are unresolved. They change the command model, accounting and UI. | Owner interview; explicit product brief and domain glossary. |
| A03 | Blocking | Fifteen industries and 25 products are asserted without demand, recipes, substitution, service capacity or counterparty definitions. Finance/healthcare/defense cannot all be ordinary inventory producers. | Industry coverage matrix including differentiated rules and minimum release depth; define final and intermediate demand. |
| A04 | Blocking | Market clearing has no price/quantity allocation algorithm, budget constraint, shortage policy or tie rules. | Worked examples with competing firms, scarcity, oversupply, imported supply, zero demand and deterministic integer rounding. |
| A05 | Blocking | Profit, cash, equity and valuation are used without a complete ledger. Loan principal, capex, depreciation, inventory and M&A can create or destroy money accidentally. | Double-entry transactions; balance sheet and cash reconciliation; explicit outside-economy accounts. |
| A06 | High | A 4 ms median promise conflicts with roughly 8 ms allocated phase time. Neither is measured at a declared workload. | One provisional budget and representative, peak and late-game fixture sizes. CPU/device timings are separate evidence. |
| A07 | High | A noisy shared CI host cannot certify A15 p99, memory, thermal behaviour or UI responsiveness. A debug phase assertion is not a release benchmark. | Stable repeated release benchmarks; device profiling gate; relative regression policy with noise handling. |
| A08 | High | Absolute zero tick allocations conflict with new firms, contracts, reports and snapshots unless capacities and overflow behaviour are defined. | Bounded capacities or amortised growth policy; measure steady-state separately from structural events. |
| A09 | High | Dense indices plus swap-remove do not by themselves provide stable references. A generation check does not repair moved references. | Stable logical handles and indirection or tombstones; deletion tests covering contracts, ownership and queued commands. |
| A10 | High | The dependency direction between sim and AI is unclear; a crate cycle is likely if AI requires game rules while sim invokes AI. | Orchestration owns both; AI consumes observations and emits the same validated commands as the player. |
| A11 | High | Fixed-point state alone does not guarantee cross-platform replay. Arithmetic overflow, rounding, RNG algorithms, serialisation and scheduler cursors remain unspecified. | Versioned numeric, ordering, RNG and canonical hashing contracts. Replay captures build/rules/data versions and accepted command sequence. |
| A12 | High | Round-robin AI makes larger worlds slower to react, rather than keeping decision quality constant. | Maximum response latency and immediate handling of insolvency, offers and contract expiry; persist scheduling state. |
| A13 | High | Weekly labour before production lacks wage settlement and hiring timing rules. Freight after input draw risks teleportation or consuming stock before delivery. | Opening/closing tick balances; orders, reservations, shipments and delivery timing; no dependence on firm iteration order. |
| A14 | High | Debt default, bankruptcy, minority holders, takeover escrow and company liquidation are missing. | Explicit state transitions and asset/liability treatment with exploit scenarios. |
| A15 | High | National populations and demand are paired with only 30 named firms. Without background firms, employers and imports the world will be mostly empty or arbitrarily easy. | Accounted background economy with capacity, demand, labour and outside trade rules. |
| A16 | High | A two MB save ceiling ignores history, command logs, national firms and migrations. Blob encoding alone is not safe persistence. | Save envelope, checksums, atomic replacement, last-good recovery, schema/rules migrations and bounded history. |
| A17 | High | First Apple integration at M3 discovers toolchain, linking and signing risks late. | First milestone compiles a real Rust command through Swift on a simulator and prepares unsigned archive CI. |
| A18 | High | TestFlight upload is treated as equivalent to installability. Processing, compliance, tester assignment, expiry, symbols and crash triage are absent. | Release state machine with independent evidence for archive, upload, processing, tester availability and device installation. |
| A19 | High | Five-year game length, one-minute decision density and 40-60 percent player win rate lack a player population or difficulty definition. Ten golden seeds cannot demonstrate a distribution. | Reference play styles and difficulty baselines; split deterministic regressions, exploratory balance sweeps and human playtests. |
| A20 | Medium | Blanket public-domain claims do not establish rights for every dataset, boundary file, font, icon or third-party dependency. Missing averages can fabricate local industries. | Asset/source manifest, source-specific terms, suppression-aware imputations and visible quality flags. |
| A21 | Medium | UI tabs list destinations without decision flows, readable explanations, accessibility or failure states. | Screen contracts, small-phone layouts, Dynamic Type, VoiceOver, reduced motion and complete empty/error/loading states. |
| A22 | Medium | Cold start, save resume, background suspension and memory-pressure paths are absent. | Lifecycle tests including termination during save and foreground/background transitions mid-tick. |
| A23 | Medium | Claims about AHD performance cite a document not found at the stated path in the current checkout. Source counts and history are unverified. | Preserve as reported historical context, not measured evidence for this product; attach primary measurements if available. |
| A24 | High | Milestones allow broad declarations of completion without per-feature evidence. | Requirements-to-test ledger, small dependency-ordered work packets and a release checklist that cannot be satisfied by placeholders. |

## Expand the delivery scope

Add the following workstreams to the revised plan. Their depth depends on owner answers,
but none may disappear silently from an autonomous handoff:

1. Product identity: player role, era, reference games, difficulty, game modes,
   monetisation, release geography and supported devices.
2. Economic specification: stock/flow units, budgets, production/services, logistics,
   labour, price formation, competition, credit, ownership and failure recovery.
3. Content: industry recipes, firms, scenarios, rival personalities, events, onboarding,
   contextual explanations, art direction, iconography and source attribution.
4. Device experience: useful dashboard, actionable comparisons, forecasts with honest
   uncertainty, command previews, critical alerts, navigation and accessibility.
5. Reliability: deterministic commands, save migration/recovery, replay diagnostics,
   capacity limits, import validation and lifecycle handling.
6. Apple delivery: Rust/Swift integration immediately, simulator CI, archive checks,
   signing configuration, privacy metadata, TestFlight and real-device acceptance.
7. Execution discipline: requirement IDs, dependency graph, evidence records,
   check scheduling, blocker isolation and exact restart instructions.

## Evidence quality

Verified locally: the target directory contains only `docs/PLAN.md`; the GitHub repository
was not found; Rust and authenticated GitHub CLI are available. Existing Lakeside Ops
source includes XcodeGen, macOS CI, simulator tests and a manual TestFlight release path.
Those files show a reusable pattern, not proof that this game's Apple access is configured.

Owner questions are pending in chat. Do not interpret silence as acceptance of new product
or release decisions. Preserve old projects and avoid production service changes.
