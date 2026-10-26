# TestFlight: 1.1.0 build 2

Date: 2026-10-26. Release workflow run 40258925439, dispatched on `main` at `4a01c65`,
the commit tagged `v1.1.0`. The `appstore` environment only admits `main`; a dispatch on the
tag itself (run 40258874964) was rejected by the environment rule before any step ran.

Every workflow step passed: Rust checks, simulator tests, archive with automatic signing,
archive and IPA verification, upload, and the bounded processing check.

```
archive_ok bundle=net.lakesidegames.corporatewarfare version=1.1.0 build=2
ipa_ok bundle=net.lakesidegames.corporatewarfare version=1.1.0 build=2
{"state": "VALID", "build": "2", "internal_build_state": "IN_BETA_TESTING", "external_build_state": "READY_FOR_BETA_SUBMISSION"}
```

Not shown here: installed acceptance on a device (R42) and external beta review. Those
remain human gates.
