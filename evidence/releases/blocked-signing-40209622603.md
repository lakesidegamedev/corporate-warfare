# Release fail-closed evidence

- Commit: `319fe31` (main at dispatch)
- Workflow: Release run 40209622603
- Inputs: version 0.1.0, build 1, confirm UPLOAD
- Result: failure at `Require signing configuration`
- Exact message: `blocked: missing signing secrets: APPLE_API_ISSUER
  APPLE_API_KEY APPLE_API_KEY_CONTENT APPLE_DEVELOPMENT_TEAM`

No archive, export, upload or TestFlight claim follows from this run. It is the intended
fail-closed outcome. Add the four secrets to the existing `appstore` environment and confirm the
App Store Connect app record, then rerun with a tested commit.
