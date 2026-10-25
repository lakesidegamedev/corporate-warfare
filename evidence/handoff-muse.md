# Muse handoff: unblock CI, Apple identity and TestFlight

Repository: `https://github.com/lakesidegamedev/corporate-warfare` (now public after a clean
working-tree and git-history secret scan).
Current main: `981bf915b7f21d4b3dcecd92ced5200f695ef084`.
Bundle: `net.lakesidegames.corporatewarfare`.
Current engine hash pin: `6731ed7cf42d0a410e94b55f3aefad6fbe51453cbe3a6803548fa7edbfb633ad`.

Already done on 2026-10-25:

- repository made public; Actions started normally;
- Verify run 40206065817 passed on main;
- Apple smoke run 40206065805 passed: simulator compile, Rust bridge, 12 XCTests and device
  archive all succeeded;
- the `appstore` environment exists (id 25281148110).

## 1. Repository and CI status

The repository is already public, the working tree and history scan was clean, and both hosted
workflows are green on main `981bf91`. Do not repeat the visibility change. Use
`gh run list -R lakesidegamedev/corporate-warfare -L 5` to confirm the latest state.

## 2. Apple identity and signing

1. Confirm the owning Apple Developer team, active agreements, and the App Store Connect app
   record for `net.lakesidegames.corporatewarfare`.
2. Create a least-privileged App Store Connect API key that can upload builds for that app.
3. Add these secrets to the existing `appstore` environment (or the repository). Never commit
   them, paste them into a workflow log, or write them under the repository:
   - `APPLE_API_ISSUER`
   - `APPLE_API_KEY`
   - `APPLE_API_KEY_CONTENT` (the `.p8` contents)
   - `APPLE_DEVELOPMENT_TEAM`
4. Confirm the intended internal TestFlight group. Do not invite testers until the owner says so.
5. Confirm the key can create/refresh provisioning profiles for the bundle.

## 3. Compile and test before release

Already green on main `981bf91`. Re-run only after a new code commit:

```
gh workflow run Verify -R lakesidegamedev/corporate-warfare --ref main
gh workflow run "Apple smoke" -R lakesidegamedev/corporate-warfare --ref main
```

Expected canonical hash is `6731ed7c...`; a simulator mismatch must be investigated, not
papered over.

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
