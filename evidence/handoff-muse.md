# Muse handoff: unblock CI, Apple identity and TestFlight

Repository: `https://github.com/lakesidegamedev/corporate-warfare` (currently private).
Current main: `a93a9d3a5f356dbf4012bc6b7244cd8b33ab25f3`.
Bundle: `net.lakesidegames.corporatewarfare`.
Current engine hash pin: `6731ed7cf42d0a410e94b55f3aefad6fbe51453cbe3a6803548fa7edbfb633ad`.

## 1. Make the repository public, safely

The owner has asked for a public repository. Before changing visibility:

1. Confirm the owner accepts that the full source and history become public.
2. Run a secret scan over the working tree **and git history** (gitleaks/trufflehog or an
   equivalent reviewed scan). Stop on any credential, signing key, personal data or production
   database snapshot. Do not print secret values in logs.
3. Confirm there is no licensed third-party source or restricted game data in the tree.
4. Only then:
   `gh repo edit lakesidegamedev/corporate-warfare --visibility public --accept-visibility-change-consequences`
5. Confirm Actions is enabled and that `Verify` can start on `main`:
   `gh workflow run Verify -R lakesidegamedev/corporate-warfare --ref main`
   `gh run list -R lakesidegamedev/corporate-warfare -L 5`

If the account remains billing-limited, restore the payment method or spending limit instead;
public visibility is not a substitute for accepted Apple Developer agreements or macOS minutes.

## 2. Apple identity and signing

1. Confirm the owning Apple Developer team, active agreements, and the App Store Connect app
   record for `net.lakesidegames.corporatewarfare`.
2. Create a least-privileged App Store Connect API key that can upload builds for that app.
3. Add these secrets to the repository or to the `appstore` environment. Never commit them,
   paste them into a workflow log, or write them under the repository:
   - `APPLE_API_ISSUER`
   - `APPLE_API_KEY`
   - `APPLE_API_KEY_CONTENT` (the `.p8` contents)
   - `APPLE_DEVELOPMENT_TEAM`
4. Confirm the intended internal TestFlight group. Do not invite testers until the owner says so.
5. Confirm the key can create/refresh provisioning profiles for the bundle.

## 3. Compile and test before release

Run both on current `main` and report the exact run URLs and conclusions:

```
gh workflow run Verify -R lakesidegamedev/corporate-warfare --ref main
gh workflow run "Apple smoke" -R lakesidegamedev/corporate-warfare --ref main
```

Apple smoke is the first Xcode compile and simulator XCTest run since PR #11. If it fails, the
failure is real software work; do not dispatch Release. Fix Swift compile/test failures on a
branch, keep the Rust hash pin honest, and rerun. Expected canonical hash is
`6731ed7c...`; a simulator mismatch must be investigated, not papered over.

## 4. Dispatch a signed TestFlight build only after both are green

Use the tested commit, not an untested `main`:

```
gh workflow run Release -R lakesidegamedev/corporate-warfare --ref main \
  -f version=0.1.0 -f build=1 -f confirm=UPLOAD
```

`Release` fails closed if any secret is missing. It archives with automatic signing, verifies
the bundle/version/architectures/privacy manifest/icon/signature, exports, uploads and polls
Apple for fifteen minutes. A `PROCESSING_AT_DEADLINE` result is not success.

## 5. Report back

Report:

- repository visibility and whether Actions actually started;
- Verify and Apple smoke run URLs and results;
- App Store Connect app id, team id, key id (never the key contents);
- Release run URL, build id, Apple processing state;
- whether the build is visible to the intended internal group;
- any real compile/test failure that needs software work.

Do not claim installed or acceptance-passed. Installation on a real iPhone and the offline
acceptance route remain separate owner/device gates (R37/R38/R42).
