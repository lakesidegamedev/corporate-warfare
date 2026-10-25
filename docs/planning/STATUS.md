# Current checkpoint

Date: 2026-10-25. Owner: DeepSeek goal session. Branch: `feat/foundation`.
Last tested commit: `753730f26636840e5c7709758a13936532acff09` (PR #8).
Kanban: 254781. Milestone issues: 1-6.

## What now exists

- `cw-core`: cents/milli-lots/BPS with checked arithmetic and half-even rounding, largest-remainder
  allocation, balanced double-entry ledger with a 13-week journal and chained compaction, stable
  generation-checked handles, SplitMix64 streams, canonical BLAKE3 hashing, 52-week calendar.
- `cw-data`: validated pinned dataset (25 products, 17 operating templates, 11 corridor regions,
  24 event cards, 30 named firms exactly two per industry), region fixture with recorded source
  attempts, farm worked example.
- `cw-sim`: the weekly settlement order, labour clearing, utilities and production, one-pass
  market allocation with the four worked examples, freight/contracts/shipments, credit grades and
  debt service, banking, insurance, governance and boards, ownership/consolidation/cycles, equity
  valuation and exchange, issuance/dividends/buybacks/IPO, insolvency/restructuring/liquidation,
  deals with antitrust review, unions, policy, research/brand, events and the five-year score.
  Full command catalogue with authority, solvency and rollback checks.
- `cw-ai`: bounded rival proposals (urgent weekly checks, four stable strategic cohorts).
- `cw-session`: scenario creation, submit/preview/advance/overview/query, versioned save envelope
  with checksum and size bounds, replay log.
- `cw-ffi` + SwiftUI app: real session behind a serial engine actor, owned DTOs, typed rejection
  keys, Start/Portfolio/Company/Regions/Markets/Inbox screens, cash bridge, saves list, atomic
  save manager.
- `cw-cli`: doctor, run, sweep, replay.

## Verified

- Hosted Verify (fmt, clippy -D warnings, workspace tests, doctor, plan/design validators):
  https://github.com/lakesidegamedev/corporate-warfare/actions/runs/40163354063
- Hosted Apple smoke: generated bindings, SwiftUI compile, four XCTest cases in an iPhone
  simulator (engine info, real session command/advance, save/load with matching canonical hash,
  rejected command, corrupt save) and unsigned device archive:
  https://github.com/lakesidegamedev/corporate-warfare/actions/runs/40163354065
- Cross-platform fixture hash `7b781159830184e8928f54e79949e343a1fd762ada357c235e42bd6f1b5fff4c`
  is now pinned in the Rust test and the Swift XCTest constant. It replaces
  `8d66f183804d61890005deb819cbc1b4587f51967fe69fe0eaf7d9c83d2bcef9`, which was the value
  verified on an iPhone simulator at commit `753730f`. The move is intentional: rival command
  ids now use the persisted sequence counter instead of a session-local counter that restarted
  after import. The iOS value for the new pin is **not re-verified yet**; see P32 below.
- Host scheduler job `20261025T043307Z-5ac62d48` passed.
- Requirements R03, R04, R31 done with evidence under `evidence/`. R05 is active again until the
  Mac runner reproduces the new pinned hash; its previous evidence describes the old pin.

## P32 adversarial reliability (in progress)

`crates/cw-session/tests/adversarial.rs` adds seven tests over the real session:

- 216 mixed valid/invalid commands across eight weeks; every rejection must keep the canonical
  hash, revision and accepted-command log unchanged, every acceptance must move all three, and
  ledgers stay balanced with non-negative stock.
- Duplicate ids return the first outcome (sequence 0, original acceptance tick) with no hash
  change, including across export/import, and are refused after the 52-week window.
- Stale revisions are refused before and after a real acceptance.
- Save corruption: 64 tail truncations, 22 interior cuts, payload/checksum/magic bit flips,
  four false declared lengths, future and unreleased schemas, and an over-cap buffer; each fails
  with a typed error and the untouched save still imports.
- Export/import/advance is idempotent: a resumed handle reaches the same hash as the
  uninterrupted run.
- A 110-week founder career keeps the journal inside its thirteen-week window, reconciles
  lifetime revenue and net income against the weekly closes, and round-trips its save.

Still open for P32: suspend/background/termination and interrupted write at the file-system layer
(Swift save manager, needs a real app run), per-schema migration fixtures, and the Linux/macOS/iOS
replay hash match for the new pin. No fuzz crate was added; command parsing bounds are covered by
typed-rejection cases.

## Defects found and fixed by P32

1. Rival command ids restarted at 1 after import, duplicating ids already in the accepted log and
   making a resumed career hash differently from an uninterrupted one. They now use the persisted
   `world.next_sequence`.
2. Command intents that carried an out-of-range product, recipe or region indexed arrays directly
   and panicked (`index out of bounds`) instead of rejecting. They now return `InvalidEntity`.
3. `Ledger::prune` skipped the first real prune at week 13, retaining fourteen weeks of postings.
4. Save schema 0 (never released) was parsed as schema 1; only `MIN_SCHEMA_VERSION..=SCHEMA_VERSION`
   is accepted now, and anything else is `UnsupportedVersion`.

## Blockers (honest)

1. **P01/R01 corridor crosswalk blocked.** BEA `econlist.cfm` now redirects to a generic data-tools
   page, the www URL is 404, `BEAREA.zip` is not an archive, and archive.org returned 429. Region
   codes/names are the verified official roster; county membership is absent, not invented. All
   calibrated values are labelled. P05/R02 remain blocked behind this.
2. **Apple signing and TestFlight blocked on owner action.** The repository has no secrets or
   variables; no org secrets are visible; no App Store Connect key is provisioned for this app;
   this Linux host has no Xcode and Mac work runs on GitHub macOS runners. Needed: confirm team
   and app record for `net.lakesidegames.corporatewarfare`, then set `APPLE_API_ISSUER`,
   `APPLE_API_KEY`, `APPLE_API_KEY_CONTENT`, `APPLE_DEVELOPMENT_TEAM` as repository or
   environment secrets, or name an approved existing studio key to reuse. Existing `.p8` files on
   this host belong to other products and were not read, copied or used.
3. **Device, human playtest and balance gates not started** (P33-P35, R36-R38).

## Exact next moves

1. Merge PR #8 after the recorded green checks.
2. P06-P09: deepen household/government/gateway accounting tests and the contract/freight fixtures;
   they exist and run but need independent oracles per packet.
3. P10/P30/P31: extend phone flows to every command family (subsidiaries, boards, takeover,
   banking, insurance, labour, policy, recovery) and add the seven-step guided onboarding.
4. P25: one worked scenario per industry; P26 research/brand fixtures.
5. P32 remaining: interrupted-write and lifecycle fault injection in the Swift save manager, the
   per-schema migration fixture set, and the Mac/iOS replay hash for the new pin; then P33 sweeps.
6. Apple: owner resolves blocker 2; then release workflow, signed upload, processing, install.

Resume with:
`cd /root/projects/corporate-warfare/worktrees/foundation && cargo test --locked --workspace`
and read `docs/planning/STATUS.md`, `execution.json`, `requirements.json`.
