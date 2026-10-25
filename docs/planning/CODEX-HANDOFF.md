# Codex handoff — Corporate Warfare

Paused 2026-10-25 at the owner's request. This file is the restart source of truth together with
`STATUS.md`, `requirements.json`, `execution.json` and `evidence/`.

## Snapshot

- Repository: `https://github.com/lakesidegamedev/corporate-warfare` (public; Actions runs normally).
- Main code commit: `3814ecad7917542a17bf8fd4e92b91724529cec4` (`test(equity): cover treasury buybacks`).
- Pause/docs head: `12cee4f`.
- Working tree: clean; no registered worktrees at pause.
- Size: 209 tracked files, 67,141 text lines; Rust 29,381 lines, Swift 7,034 lines; 3.1 MB excluding
  `.git` and `target`.
- Tests: 222 Rust tests; 14 iOS simulator XCTests. Latest green Apple smoke on PR #69:
  simulator compile, Rust bridge, XCTests and unsigned device archive.
- Canonical hash: `3cef19c75778b8ac956c65b010a2eeda2a3ecae4a4f981a8b90a054aca8c1dab`
  (Rust pin and `apple/Tests/EngineBridgeTests.swift`).

## Completion state

| Ledger | Done | Active | Pending | Blocked |
|---|---:|---:|---:|---:|
| Requirements (42) | 26 | 8 | 8 | 0 |
| Execution packets (39) | 8 | 28 | 3 | 0 |
| Checklist steps (117) | 35 | — | — | — |
| Pull requests merged | #8 through #69 | | | |

Requirements done (26): R01-R20, R23, R25-R27, R30 and R31. Remaining:

- Active: R21 mergers/takeovers, R22 antitrust, R24 lobbying, R28 rival AI, R29 events,
  R35 onboarding/accessibility, R39 release assets/privacy, R40 reproducible archive.
- Pending: R32 core phone acceptance (human), R33 corporate phone acceptance (human), R34 score,
  R36 balance/adversarial, R37 device performance, R38 independent playtests, R41 processed
  TestFlight, R42 installed acceptance.

Packet status is behind the code. Most implemented systems still have `active` packets and
unchecked steps; do not trust packet status alone, and do not mark a packet done without
checklist evidence.

## Verified

- Latest code CI on main: Verify and Apple smoke green for PR #69 at `3814eca`.
- Green evidence for the earlier `981bf91`/`319fe31` runs is under `evidence/releases/` and
  `evidence/R05-cross-platform.json`, `evidence/R30-saves.json`.
- Unsigned archive inspection: `evidence/releases/unsigned-981bf91.md`; correct bundle
  `net.lakesidegames.corporatewarfare`, version 0.1.0 (1), iOS 17, privacy manifest and icons.
- Geography: owner approved an authored corridor. R01/R02 and P01/P05 are closed with an explicit
  “authored estimates, no county claim” limitation in `DECISIONS.md` and
  `content/geography/corridor-fixture.json`.
- Balance: 260-week seed 3 has four dissolved rivals and score 61; 1,000 five-year seeds score
  56/62/62 with dissolved median 5; 100 thirty-year seeds have dissolved median 10 and the
  passive policy loses control in the median seed.

## Blockers, in critical-path order

1. **Apple signing / TestFlight.** The `appstore` environment exists (id 25281148110), but no
   secrets do. Release run `40209622603` failed closed with:
   `blocked: missing signing secrets: APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_CONTENT APPLE_DEVELOPMENT_TEAM`.
   Need the App Store Connect app record for `net.lakesidegames.corporatewarfare`, the owning team,
   active agreements, least-privileged API key, and the intended internal tester group. Exact steps:
   `apple/RELEASE.md`, `evidence/handoff-muse.md`.
2. **Device and human acceptance.** No A15-class iPhone measurements (R37), no three independent
   testers (R38), no TestFlight installation (R42), and no human phone acceptance route
   (R32/R33/R35). These cannot be completed from the Linux host.
3. **Remaining automated requirements/evidence.** R21, R22, R24, R28, R29, R34, R39, R40 and the
   packet checklist/evidence ledger. Several systems are implemented; the work is mostly fixtures,
   review and honest completion notes.
4. **UI test target is parked.** `apple/UITests/AppFlowUITests.swift` exists but its first
   XCUITest hung the simulator runner and has been removed from the default scheme. Unit, bridge,
   save and archive CI are green. Fix separately or leave out of release gating.

## Restart

```
cd /root/projects/corporate-warfare
git pull --ff-only
cargo test --locked --workspace
python3 tools/plan_check.py
gh run list -R lakesidegamedev/corporate-warfare -L 5
gh secret list -R lakesidegamedev/corporate-warfare
```

Use a registered worktree and the shared scheduler for heavy checks. The owner has asked for no
long sleeps; inspect run state with `gh run view` rather than polling.

## Recommended next moves

1. Confirm the Apple app record/team/agreements and add the four secrets to `appstore`.
2. Dispatch `Release` on the tested commit with version/build/`UPLOAD`; poll processing; report
   build id and state. Do not claim a build exists until Apple says `VALID`.
3. Parallel software work: close R21/R22/R24/R28/R29/R34 and the packet checklist with real
   fixtures, then R39/R40.
4. Install the processed build on a real iPhone, run the offline acceptance route, record device
   timing/crash evidence (R37) and three independent playtests (R38).

## Honest completion estimate

Code volume and most systems are implemented; the release ledger is not. Requirements are 26/42
(62 percent), packets 8/39 (21 percent) and steps 35/117 (30 percent). The remaining automated
software/evidence work is plausibly 5-8 focused sessions. The Apple signing, processed TestFlight
build, physical-device checks and independent testers are external and dominate the calendar;
a completion date cannot be promised until the four secrets exist and a real iPhone is available.

No TestFlight or installed-build claim is made.
