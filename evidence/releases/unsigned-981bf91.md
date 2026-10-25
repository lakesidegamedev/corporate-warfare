# Unsigned Apple archive evidence

- Commit: `981bf915b7f21d4b3dcecd92ced5200f695ef084`
- Apple smoke run: 40206065805 (macos-26), conclusion success
- Artifact: `unsigned-apple-smoke`, artifact id 12603654732
- Simulator tests: 12 executed, 0 failures (EngineBridgeTests and SaveLifecycleTests)
- Canonical hash: `6731ed7cf42d0a410e94b55f3aefad6fbe51453cbe3a6803548fa7edbfb633ad`

Portable inspection of the downloaded `.xcarchive`:

- `CFBundleIdentifier = net.lakesidegames.corporatewarfare`
- `CFBundleShortVersionString = 0.1.0`
- `CFBundleVersion = 1`
- `MinimumOSVersion = 17.0`
- `PrivacyInfo.xcprivacy` present
- compiled `Assets.car` and both compiled app icons present
- arm64 device binary present (3,811,856 bytes)
- device archive step succeeded

This is an unsigned archive. It is **not** a signed, uploaded, processed or installed
TestFlight build. The App Store Connect app record and the four signing secrets are still
required before `Release` can run.
