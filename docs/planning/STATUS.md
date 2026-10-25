# Current checkpoint

Date: 2026-10-25. Owner: DeepSeek goal session. Viability branch based on `9d7339b`.
Kanban: 254781. Milestone issues: 1-6. Merged PRs: #8 foundation, #9 release track,
#10 onboarding, #11 adversarial reliability, #12 industry scenarios.

## What now exists

- `cw-core`: cents/milli-lots/BPS, checked arithmetic and half-even rounding, largest-remainder
  allocation, balanced double-entry ledger with a 13-week journal and chained compaction, stable
  generation-checked handles, SplitMix64 streams, canonical BLAKE3 hashing, 52-week calendar.
- `cw-data`: validated pinned dataset (25 products, 17 operating templates, 11 corridor regions,
  24 event cards, 30 named firms exactly two per industry), region fixture with recorded source
  attempts, farm worked example.
- `cw-sim`: weekly settlement order, labour clearing, utilities and production, one-pass market
  allocation with the four worked examples, freight/contracts/shipments, credit, banking,
  insurance, governance, ownership/consolidation/cycles, valuation and exchange,
  issuance/dividends/buybacks/IPO, insolvency, deals with antitrust, unions, policy, research,
  brand, events and the five-year score. Full command catalogue with authority/solvency/rollback
  checks. Eleven production-accounting defects were found and fixed by the industry pass.
- `cw-ai`: bounded rival proposals (urgent weekly checks, four stable strategic cohorts).
- `cw-session`: scenario creation, submit/preview/advance/overview/query, versioned save envelope
  with checksum and bounds, replay log.
- `cw-ffi` + SwiftUI app: serial engine actor, owned DTOs and typed rejection keys; Start,
  Portfolio, Company, Regions, Markets, Inbox; cash bridge; saves list with atomic write and
  last-good copy; seven-step guided onboarding; original app icon and privacy manifest.
- `cw-cli`: doctor, run, sweep, replay. Release workflow: archive, verify, sign/export, upload,
  bounded App Store Connect processing check, fail-closed when secrets are absent.

## Verified

- 126 workspace tests pass locally, with `cargo fmt --check` and
  `cargo clippy --workspace --all-targets -D warnings` clean.
- Hosted Verify and Apple smoke passed on PRs #8 (40163354063 / 40163354065), #9, #10 and #11.
- iPhone simulator XCTest verified the canonical hash `7b781159...` at PR #11.
- After the industry fix merge the fixture hash is `cb14891544da9a770548d832c8a54d787d7926614c840fbd309fba85312ff21d`,
  computed on Linux. The matching iOS run is **not yet verified**; R05 is therefore `active`,
  not done.
- Requirements done: R03, R04, R31. R05 is active pending a Mac-runner pass on the new pin.

## Blockers (honest, newest first)

1. **GitHub Actions billing blocked.** From PR #12 onward every workflow fails in ~2 seconds
   before starting: "recent account payments have failed or your spending limit needs to be
   increased". Local checks still run; hosted gates and the iOS hash verification cannot. Owner
   action: restore Actions billing/spending limit. Do not merge check-dependent work silently;
   this checkpoint records the exception for the locally verified #12 squash.
2. **P01/R01 corridor crosswalk blocked.** BEA econlist now redirects to a generic data-tools
   page, the www URL is 404, BEAREA.zip is not an archive, archive.org returned 429. Region codes
   and names are the verified official roster; county membership is absent, not invented. P05/R02
   stay blocked behind this.
3. **Apple signing/TestFlight blocked on owner action.** No repository or organisation secrets;
   no App Store Connect key provisioned for this app. Needed: confirm team and the app record for
   `net.lakesidegames.corporatewarfare`, then set `APPLE_API_ISSUER`, `APPLE_API_KEY`,
   `APPLE_API_KEY_CONTENT`, `APPLE_DEVELOPMENT_TEAM` (repository or `appstore` environment), or
   name an approved existing studio key. Keys on this host belong to other products and were not
   read, copied or used. `apple/RELEASE.md` holds the runbook and state table.
4. **Corridor solvency is not solved.** Spot books are same-region, gateways quote a finite buy
   side that clearing does not use, seeded capacity exceeds local household demand in several
   regions, media advertising demand from marketer budgets is unused, and construction projects do
   not consume contractor service/equipment. Producers without a local buyer earn nothing. This is
   the next product-quality blocker, not a test artefact.
5. **Device, human playtest and balance gates not started** (P33-P35, R36-R38).

## Exact next moves

1. Restore GitHub Actions billing, then rerun PR checks on the new hash and re-record R05 with the
   iOS XCTest evidence.
2. Fix corridor solvency: use the gateway buy side for finite exports, allow cross-region spot
   offers with freight and lead time, and wire marketing budgets to media demand; add a corridor
   run that reaches 260 weeks with explainable firm failures rather than silent starvation.
3. Extend phone flows to every command family (subsidiaries, boards, takeover, banking, insurance,
   labour, policy, recovery) and complete the P31 accessibility audit at 320pt and AX text.
4. P32 remaining: filesystem interrupted-write/suspend recovery in Swift and migration fixtures.
5. P33 balance sweeps, P34 device profiling, P35 three independent playtests.
6. Apple: resolve blocker 3, dispatch the release workflow, poll processing, install on a real
   iPhone and run the acceptance route. Only then can the goal close.

Resume with:
`cd /root/projects/corporate-warfare && git pull && cargo test --locked --workspace`.
