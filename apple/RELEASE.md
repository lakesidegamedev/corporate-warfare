# Release and TestFlight runbook

States are deliberately separate. A green job means only the state it printed, never a later one.

| State | How it is established |
|---|---|
| compiled | Apple smoke workflow: simulator compile |
| tested | EngineBridgeTests in an iPhone simulator |
| archived | Release workflow archive step |
| signed/exported | `verify-archive.sh` passes on a signed archive |
| uploaded | `xcrun altool --upload-app` exits 0 |
| processing | `asc_status.py` prints PROCESSING (incomplete) |
| available | `asc_status.py` prints VALID with a build id |
| installed | a real iPhone installed the build through TestFlight (device evidence) |
| acceptance-passed | the offline acceptance route finished on that device |

## Required owner actions (currently blocking)

As of 2026-10-25 (1.0.0 branch) Verify and Apple smoke run on every PR and the `appstore`
environment exists. Release run 40209622603 failed closed because the four signing secrets are
missing. The intended key is the studio App Store Connect API key already used for the other
Lakeside iOS apps; its issuer ID and team ID are stored only as secrets elsewhere, and GitHub
never returns secret values, so they must be supplied again for this repository.

1. Create the App Store Connect app record for bundle ID `net.lakesidegames.corporatewarfare`
   on the same team as Ask and Ops, and name the internal tester group. The API cannot create
   app records.
2. Supply that key's issuer ID and team ID. The operator then sets
   `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_CONTENT` and `APPLE_DEVELOPMENT_TEAM` on
   the `appstore` environment, restricted to `main` deployments.
3. Confirm agreements are accepted and the team can create provisioning profiles.

Until those exist, run the workflow anyway: it fails closed with
`blocked: missing signing secrets` and never claims a release.

## Running a release

Use **Actions -> Release -> Run workflow** on a tested commit with:

- `version`: marketing version, for example `1.0.0`
- `build`: unique increasing build number
- `confirm`: exactly `UPLOAD`

The workflow regenerates the engine, runs Rust and simulator tests, archives with automatic
signing, verifies bundle/version/architectures/privacy manifest/icon/signature, exports, uploads,
then polls Apple for up to fifteen minutes. `PROCESSING_AT_DEADLINE` exits 3 and is **not**
success. `INVALID`/`FAILED`/`EXPIRED` exit 2.

## After upload

1. Read `processing.json`; record the build id and state.
2. Only after `VALID`, the intended internal group can install through TestFlight.
3. Installation and the acceptance route require a real iPhone. Capture build number, device,
   OS, profile session, crashes, memory/performance and known issues under `evidence/releases/`.
4. TestFlight builds expire after ninety days.

The privacy manifest declares no tracking, one collected data type (gameplay content for
analytics, not linked to the player: the anonymous market snapshot every five game weeks,
which the player can switch off), and the UserDefaults required-reason API (`CA92.1`).
`apple/scripts/privacy_audit.py` allows network use only in `Telemetry.swift` and only to the
documented endpoint. Re-audit it whenever a dependency
or API usage changes.
