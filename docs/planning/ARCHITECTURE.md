# Architecture and persistence contract

## Boundaries and implementation order

The bootstrap has `cw-core` (version identities), `cw-ffi` (one packaging probe),
`cw-cli` (doctor) and a pinned UniFFI generator. They are not the implemented economy.
Add crates when their seam is real:

- `cw-core`: stable IDs, checked units, state and deterministic primitives.
- `cw-data`: validated immutable datasets and scenario definitions.
- `cw-sim`: validated commands and economic transitions, no I/O.
- `cw-ai`: observations to commands, no direct world mutation.
- `cw-session`: orchestration of sim, AI, replay, reports and pause policy.
- `cw-ffi`: owned DTOs, bounded session access and typed errors.
- `cw-cli`: headless orchestration, replay, diagnostics, sweeps and benchmark tools.

Dependencies point toward core/data. Sim must not depend on FFI, filesystem, networking
or AI. Session calls sim and AI. Swift imports bindings only and owns lifecycle/file I/O.
Do not split every economic rule into another crate or expose a function for each UI tap.

## State and determinism

Use integer/fixed-point authoritative state. Checked arithmetic is enabled in release.
Pin RNG algorithm and stream schema; domain streams isolate events, AI and world generation.
Persist stream state, entity allocator, AI cursors, queues and unresolved offers.
Tie rules and random draw order are part of the rules version.

Use stable typed IDs with generation validation and an indirection map to dense rows.
Swap-removing rows updates indirection atomically. Save/replay references use logical IDs,
not row positions. Avoid whole-world rescans for deletion. A subsidiary forest should use
iterative traversal and cached group membership invalidated on ownership transitions.

Start with simple typed tables. Adopt SoA where measured iteration cost warrants it.
A blanket HashMap or allocation ban is not a correctness contract. Deterministic
iteration always uses stable sorted IDs or a declared schedule. Structural additions may
allocate within measured limits; steady tick scratch buffers are reused.

A canonical state hash uses explicit field order, fixed endian encoding and a pinned
hash algorithm. Exclude wall time, UI cache and profiling metrics. Include pending commands,
RNG, AI scheduling and all future-affecting state. Serialisation bytes are not implicitly
canonical. Every replay records engine/rules/data/schema versions and initial state or
scenario hash. Incompatible rules fail clearly or use a shipped compatibility path.

## Commands and observations

Commands carry sequence, expected state revision, actor, intent and parameters. Session
serialises all mutation. Validation returns typed success/rejection and a preview may
return changed costs before confirmation. Duplicate command IDs are idempotent. Failed
compound commands leave no partial cash reservation or state change.

AI gets the same public observations and command permissions as an equivalent player.
Private firm data is available only to authorised management. Due diligence costs time
and money. AI decision work has bounded operations, deterministic scheduling and maximum
reaction latency. Urgent insolvency and offer deadlines cannot wait behind every firm.
Pause/start UI or device scheduling must not change the economic outcome for a given log.

## Rust/Swift boundary

Proposed session API: create scenario, submit command batch, advance a bounded number of
weeks, obtain summary/detail observations, export state, import state and inspect error.
Do not freeze an arbitrary eight-call count. Design around capabilities and payload sizes.
Catch domain errors explicitly; panic is a bug, never a normal rejection path.

A Swift actor or dedicated serial executor owns the Rust handle. Never run ticks,
save encoding or expensive observation conversion on the main actor. Publish small
immutable view models on the main actor. UniFFI DTOs are copied/serialized; zero-copy
is not assumed. Separate tick compute, snapshot encoding, FFI lift, view update and
end-to-end latency measurements. Support cancellation between ticks, never half a tick.

Lifecycle uses stopped, ready, advancing, awaiting-decision, saving and failed states.
Save only committed tick boundaries. Backgrounding requests bounded stop-and-save; if
OS suspension wins, retain the last complete save. Foreground never replays an accepted
command twice. Autosave UI and manual save slots cannot race on file replacement.

## Save envelope and recovery

Envelope fields: magic, schema version, rules version, data identity, engine build,
career ID, tick/revision, payload length, checksum and compressed payload. Validate size
bounds before allocating, then verify checksum before parsing. Reject future or corrupt
formats with an actionable error. Never execute or trust imported paths/names.

Write a new temporary file in the save directory, flush, atomically replace, retain
last-known-good backup. Provide manual save slots, export/import and a recovery picker.
Bundle migration fixtures for each released schema. Migrations preserve an audit record
and never overwrite the only original. Saved dataset version is embedded or recoverable;
an app upgrade cannot silently remap facilities by new region row positions.

Separate bounded historical metrics from full optional command log. Initial save goals
are measured budgets, not premature two MB hard rejection. Define maximum import bytes,
entity counts and career history retention from stress fixtures. Disk full, interrupted
writes and memory pressure must fail safely. Never claim device recovery from a Linux
round-trip test alone.
