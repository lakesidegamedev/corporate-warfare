# Owner actions required to finish the release

Nothing below can be done from this Linux host or by an agent without the named access. Each
item lists the exact action and what becomes possible afterwards.

## 1. Restore GitHub Actions billing

Symptom: every workflow fails in 2-4 seconds before any step runs, with
"The job was not started because recent account payments have failed or your spending limit needs
to be increased."

Action: restore the account payment method or raise the Actions spending limit in GitHub billing
settings, or use the owner-approved public-repository path in `evidence/handoff-muse.md`.

Then immediately possible:

- `Verify` runs fmt, clippy, workspace tests, doctor and the plan/design validators on `main`.
- `Apple smoke` regenerates the XCFramework, compiles the SwiftUI app (uncompiled since PR #11),
  runs the eight save-lifecycle tests and the session hash test, and archives an unsigned device
  app.
- Expected canonical hash in `EngineBridgeTests` and the Rust pin:
  `6731ed7cf42d0a410e94b55f3aefad6fbe51453cbe3a6803548fa7edbfb633ad`.
  If the simulator disagrees, that is a real cross-platform finding, not a reason to change the
  pin without investigating.

## 2. Apple identity, signing and TestFlight

Missing from the repository and organisation: `APPLE_API_ISSUER`, `APPLE_API_KEY`,
`APPLE_API_KEY_CONTENT`, `APPLE_DEVELOPMENT_TEAM`. No App Store Connect app record has been
verified for `net.lakesidegames.corporatewarfare`.

Actions:

1. Confirm the owning team, active agreements and the final bundle ID.
2. Create or confirm the App Store Connect app record for that bundle ID.
3. Add the four secrets to the repository or the `appstore` environment (least-privileged App
   Store Connect key), or name an approved existing studio key path.
4. Confirm the intended internal tester group.

Then possible: dispatch `Release` with version, build and `UPLOAD`. The workflow fails closed on
missing secrets, archives with automatic signing, verifies bundle/version/architectures/privacy
manifest/icon/signature, exports, uploads, and polls processing for fifteen minutes. `VALID` is
required before a build exists for testers; `PROCESSING_AT_DEADLINE` exits 3 and is not success.

Keys present on this host belong to other products and were deliberately not read or reused.

## 3. Geography decision (blocks the ledger, not the engine)

The official 2004 BEA county-to-economic-area crosswalk could not be recovered (BEA URLs redirect
or 404, `BEAREA.zip` is not an archive, archive.org rate-limits). Region codes and names come from
the verified official roster; county membership rows are absent.

Choose one:

- Supply the official crosswalk file (publisher, download date, checksum), or
- Approve a named custom corridor scheme as the product's geography, recorded in DECISIONS.

Until then R01/R02 and every requirement and packet that depends on the historical dataset stay
blocked even though the systems are implemented against labelled fixtures.

## 4. Device and human acceptance

Needed for R37, R38 and R42:

- An identified A15-class iPhone and OS version for measurements.
- Three independent testers recruited by the owner, following `apple/metadata/what-to-test.txt`.
- A signed TestFlight build (item 2) before installation.

No device measurement or playtest report will be written before those exist.
