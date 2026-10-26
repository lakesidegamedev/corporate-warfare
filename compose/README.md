# Corporate Warfare for Android, Windows and Linux

One Compose Multiplatform interface over the same Rust engine as the iPhone app. The engine is
built as a shared library and called through uniffi's Kotlin bindings, which load it with JNA.

- `shared/`: the app (`src/jvmShared`), compiled into both the Android and the desktop targets.
  Each target brings its own JNA (an AAR on Android, a JAR on the desktop), which is why the app
  is one source directory shared by both rather than an intermediate source set.
- `androidApp/`: the Android application (`net.lakesidegames.corporatewarfare`).
- Desktop app (Windows, Linux): next, reusing `shared` with a desktop entry point.

Build:

```
compose/scripts/build-engine.sh desktop   # host engine and bindings, for the smoke test
compose/scripts/build-engine.sh android   # arm64-v8a and x86_64 engines for the APK
cd compose && ./gradlew :shared:desktopTest :androidApp:assembleDebug
```

Generated bindings and libraries live in `compose/build/engine` and are never committed. CI
(`.github/workflows/android.yml`) runs the smoke test and uploads the debug APK.
