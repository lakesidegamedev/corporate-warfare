// Corporate Warfare for Android, Windows and Linux: one Compose Multiplatform interface over
// the Rust engine (uniffi Kotlin bindings, loaded through JNA). See compose/README.md.
plugins {
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.compose.multiplatform) apply false
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.library) apply false
}
