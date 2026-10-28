# TestFlight builds 5 to 15

Every `Release` workflow run that produced a build accepted by App Store Connect, read back from
each run's `release-artifacts` → `processing.json` and from the archived app's `Info.plist`.
Build 6 does not exist: the runs between build 5 and build 7 failed closed before upload.

| Build | Version | Run | Date | State | Internal | External |
|---|---|---|---|---|---|---|
| 5 | 1.3.1 | 40372655587 | 2026-10-26 | VALID | IN_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 7 | 1.4.0 | 40416138429 | 2026-10-26 | VALID | IN_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 8 | 1.5.0 | 40433031668 | 2026-10-27 | VALID | IN_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 9 | 1.6.0 | 40450808232 | 2026-10-27 | VALID | READY_FOR_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 10 | 1.7.0 | 40522874792 | 2026-10-27 | VALID | READY_FOR_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 11 | 1.7.0 | 40542945991 | 2026-10-27 | VALID | READY_FOR_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 12 | 1.7.0 | 40558318217 | 2026-10-27 | VALID | READY_FOR_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 13 | 1.7.0 | 40563333764 | 2026-10-27 | VALID | IN_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 14 | 1.7.0 | 40571885650 | 2026-10-27 | VALID | IN_BETA_TESTING | READY_FOR_BETA_SUBMISSION |
| 15 | 1.7.1 | 40615251714 | 2026-10-28 | VALID | READY_FOR_BETA_TESTING | READY_FOR_BETA_SUBMISSION |

All builds share app id `6826923645` and bundle id `net.lakesidegames.corporatewarfare`.

Two runs failed closed and uploaded nothing: 40413832155 (2026-10-26) and 40570398090
(2026-10-27, v1.7.0). Build 15 was archived from `685b1ada8955f5e76f3652811b242d7712e9dc70`
with `SUPPORTS_MACCATALYST=NO`.

Not shown here: installation and acceptance on a physical device (R42), external Beta App
Review, and App Store submission. Those remain human gates.
