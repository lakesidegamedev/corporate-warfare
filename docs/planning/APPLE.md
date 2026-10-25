# Apple build and TestFlight delivery

Official requirements and citations: `../RESEARCH.md`. Recheck before release.
Current verified upload floor is Xcode 26 with iOS 26 SDK. Minimum app runtime is a
separate decision, provisionally iOS 17. The owner says Apple resources are available;
that is not proof this repo's app identity, secrets or signing path are configured.

## Bootstrap in this repository

`apple/project.yml` is the generated project source. Proposed bundle ID:
`net.lakesidegames.corporatewarfare`; verify ownership and final spelling before release.
The app currently calls one Rust engine-info function. It visibly says gameplay is
unimplemented. UniFFI runtime and generator share the exact pinned version in Cargo.lock.

`bash apple/scripts/build-engine.sh` on a supported Mac builds ARM64 device and simulator
static libraries, generates Swift/header/modulemap files and assembles an XCFramework.
XcodeGen creates the application project. `Apple smoke` CI compiles the simulator app, runs a native XCTest bridge probe,
and archives an unsigned device app. Both must actually pass before claiming packaging
works. Generated outputs are ignored; they must be regenerated from the tested commit.

The packaging probe is deliberately small. The bootstrap XCTest calls Rust engine-info through generated bindings. The next
packet extends it to create a real session, submit a command and save/load. Add iPhone UI tests and
lifecycle tests as those features arrive. Compile success alone is not runtime proof.

## Project-specific resources to verify

1. App Store owning team, active membership and accepted agreements.
2. Final explicit App ID and App Store Connect app record; no accidental reuse of Ops IDs.
3. Mac runner label, supported Xcode build, SDK and Rust target/deployment compatibility.
4. Least-privileged App Store Connect API credential or approved signing identity path.
5. Repo/environment secret names only: APPLE_API_ISSUER, APPLE_API_KEY,
   APPLE_API_KEY_CONTENT, APPLE_DEVELOPMENT_TEAM, matching the existing studio pattern.
6. Release environment controls and intended internal tester group, test device/OS.
7. App icon, beta description, What to Test, feedback/review contacts, privacy URL,
   age rating and export-compliance determination based on actual shipped code.

Inspect secret names/availability, never print secret values. Do not export Ops signing
credentials from its runtime environment or copy another app's entitlements. Reuse an
approved studio credential through the secret manager or owner configuration path.
Mac CI for PRs must have no signing secrets. Release workflow is separate, explicit,
restricted to a tested commit, with temporary keychain/key material removed on exit.

## Release work packet

Implement a manually dispatched release workflow after unsigned smoke passes. Build
from a clean pinned commit; run required tests; generate fresh bindings, icons and data;
archive; inspect embedded frameworks and Info.plist; validate privacy manifest and
required-reason APIs; sign/export; verify bundle, version, entitlements, architectures,
profile and symbols; upload; retain symbol files, archive provenance and checksum.
Provisioning or missing app record errors must fail or emit an explicit blocked outcome,
never a green 'release complete' while skipping the upload.

Do not hardcode a claim of no collection or encryption exemption without checking the
actual archive and dependencies. The offline default should avoid analytics SDKs and
mandatory network calls. Crash reporting remains opt-in only if separately approved.
Prepare an exportable local diagnostic bundle with user review.

Record release as separate states: compiled, archived, signed/exported, uploaded,
processing, available-to-group, installed, acceptance-passed. Poll Apple with deadlines;
handle invalid/rejected/failed/expired states as terminal failures. A TestFlight build
expires after 90 days. Internal beta and external Beta App Review are distinct tracks.
Do not select random staff/testers or send invitations without owner direction.

## What the owner may still need to do

If API permissions cannot create the app record or accept agreements, provide exact
screens/values and the tested artifact/commit. Ask only for that missing action. Continue
all independent work. A credential blocker does not justify shipping placeholder screens
or omitting tests. Final software-ready handoff lists each unsatisfied Apple gate with
responsible person and next step. 'TestFlight installed' requires device evidence.

## Acceptance flight

On an actual iPhone: install via TestFlight, launch in airplane mode, complete onboarding,
make and resume a save, execute corporate/control and recovery routes, reach five-year
score and continue sandbox. Capture build number, device/OS, profile session, observed
crashes, memory/performance and known issues. A simulator recording cannot replace this.
