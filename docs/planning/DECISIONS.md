# Decision register

| ID | State | Decision | Rationale / revisit condition |
|---|---|---|---|
| D01 | Owner confirmed | Polished Great Lakes first; national later | Finish a coherent iOS product before expansion. |
| D02 | Owner confirmed | Investor persists across company loss | Makes control and bankruptcy meaningful without ending career. |
| D03 | Owner confirmed | Open-ended plus five-year score | Both empire-building and bounded sessions. |
| D04 | Owner confirmed | Premium offline | No mandatory account, server or ads. |
| D05 | Owner confirmed direction | Rust + SwiftUI, weekly simulation, regional model | Challenge if early packaging or economic evidence warrants it. |
| D06 | Proposed | January 2000, 2004 BEA boundary vintage | Baseline vintage is promising; source coverage still unproved. |
| D07 | Proposed | All named corporate systems in bounded release versions | Owner said most if not all; do not silently omit one. |
| D08 | Proposed | No circular equity ownership in first release | Cycle rejection avoids recursive valuation/control exploits; broad subsidiary trees remain allowed. |
| D09 | Proposed | One ordinary voting share class initially | Separates votes from ownership in the model; dual-class issuance can wait for a justified requirement. |
| D10 | Proposed | Quarterly/annual finance using a 52-week game calendar | Clear integer cadence; UI calls it a game calendar, not exact civil weeks. |
| D11 | Proposed | No iCloud in first release; export/import saves | Avoid a new entitlement/conflict domain before local recovery is reliable. |
| D12 | Proposed | Relative CI performance checks plus device absolute targets | Shared runners cannot prove A15 wall-time limits. |
| D13 | Proposed | Session coordinator owns AI plus simulation | Prevents cyclical crate dependencies and preserves one command path. |

Do not use this register to invent owner approval. For changes, add date, evidence and
consequences, update affected contracts/requirements, and retain the old decision history.

## Architect-selected implementation baseline, 2026-10-25

The owner clarified that DeepSeek should execute the thinking done here. The exact
initial algorithms, economic constants, 52-week calendar, one-pass rationing, funded
banking, one share class, five-seat boards,90-percent fictional squeeze-out, regulator
thresholds, thirteen-week reporting, one-percent aggregate game scale, no first-release
reinsurance and bounded policy model are now prescribed in docs/rules/. These are
author-selected defaults, not newly claimed owner quotations. Implement first; change
only with documented economic/test evidence and architect decision. Product choices
D01-D05 remain owner authority. Source observations remain distinct from calibration.

## Implementation decisions, 2026-10-25 (DeepSeek goal session)

These are engineering choices inside the approved design, recorded with their evidence and
revisit conditions. None changes an owner product decision.

| ID | State | Decision | Consequence / revisit condition |
|---|---|---|---|
| D14 | Implemented | Saves use the pinned postcard payload with a BLAKE3 checksum, length bounds and no compression in v0.1. | Build stays pure Rust on every target and the 64 MB bound is enforced directly. Revisit with measured save sizes; adding zstd must preserve the checksum-over-payload and bounds rules. |
| D15 | Implemented | Spot books are per destination region; inter-region trade runs through contracts and shipments with freight and lead time. | No stock teleports and the one-pass allocator stays bounded. Cross-region ad-hoc spot can be added later without changing conservation rules. |
| D16 | Blocked fixture | Corridor uses the verified 2004 EA codes and names with no county membership rows, plus labelled calibration. | R01/R02 cannot be signed off until a provenance-traceable crosswalk is recovered. Never replace the fixture with invented counties. |
| D17 | Implemented | Within the 32-account chart, bank customer loans and customer deposit claims are carried in `Receivables`. | Keeps one chart for all legal entities and prevents double-counting outside cash. Documented in the banking module; revisit if a separate receivable class is justified. |
| D18 | Implemented | The full command catalogue crosses FFI as JSON with a typed rejection record; frequent phone flows also have typed methods. | Keeps the boundary small while every family is reachable and validated by the engine. Revisit if DTO cost or type safety on a hot path warrants typed methods for more intents. |
