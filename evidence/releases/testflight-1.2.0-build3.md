# TestFlight: 1.2.0 build 3

Date: 2026-10-26. Release workflow run 40275850031, dispatched on `main` at `a50c623`, the
commit tagged `v1.2.0`. Every workflow step passed: Rust checks, simulator tests, archive with
automatic signing, archive and IPA verification, upload, and the bounded processing check.

```
archive_ok bundle=net.lakesidegames.corporatewarfare version=1.2.0 build=3
ipa_ok bundle=net.lakesidegames.corporatewarfare version=1.2.0 build=3
{"state": "VALID", "build": "3", "internal_build_state": "IN_BETA_TESTING", "external_build_state": "READY_FOR_BETA_SUBMISSION"}
```

Not shown here: installed acceptance on a device (R42) and external beta review. Those
remain human gates.
