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

As of 2026-10-25 the repository is public, Verify and Apple smoke are green on main `319fe31`,
and the `appstore` environment exists. Release run 40209622603 failed closed with the exact
missing-secret list below. Only Apple signing remains before an upload attempt.

1. Confirm the App Store Connect team, the app record for bundle ID
   `net.lakesidegames.corporatewarfare`, and the intended internal tester group.
2. Provide the least-privileged App Store Connect API key to the repository or the `appstore`
   environment as secrets:
   - `APPLE_API_ISSUER`
   - `APPLE_API_KEY` (the key identifier)
   - `APPLE_API_KEY_CONTENT` (the `.p8` contents)
   - `APPLE_DEVELOPMENT_TEAM`
   Alternatively name an approved existing studio key path to reuse. Keys found on this host
   belong to other products and were not read, copied or used.
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

The privacy manifest declares no tracking and no collected data, and the UserDefaults
required-reason API (`CA92.1`) used by onboarding completion. Re-audit it whenever a dependency
or API usage changes.
