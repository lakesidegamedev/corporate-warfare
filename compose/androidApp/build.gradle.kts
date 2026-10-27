plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

// Release builds are signed with the upload key when CI provides it; local builds stay debug.
val uploadKeystore = providers.environmentVariable("CW_ANDROID_KEYSTORE_FILE").orNull
val uploadPassword = providers.environmentVariable("CW_ANDROID_KEY_PASSWORD").orNull
// One version for every platform: the workspace version in the root Cargo.toml.
val cargoVersion = rootProject.file("../Cargo.toml").readLines()
    .first { it.startsWith("version = ") }.substringAfter('"').substringBefore('"')
// Preview builds (-Pcw.channel=preview -Pcw.preview=N) sit just below the release they lead to,
// as in shared/build.gradle.kts.
val previewNumber = if (findProperty("cw.channel") == "preview") (findProperty("cw.preview") as String?)?.toInt() ?: 0 else 0
val baseVersion = (findProperty("cw.version") as String?) ?: cargoVersion
val gameVersion = if (previewNumber > 0) "$baseVersion-preview.$previewNumber" else baseVersion
val gameVersionCode = baseVersion.split('.').map(String::toInt).let { (major, minor, patch) ->
    if (previewNumber > 0) major * 10_000 + (minor - 1) * 100 + 90 + previewNumber else major * 10_000 + minor * 100 + patch
}

android {
    namespace = "net.lakesidegames.corporatewarfare"
    compileSdk = libs.versions.compileSdk.get().toInt()

    defaultConfig {
        applicationId = "net.lakesidegames.corporatewarfare"
        minSdk = libs.versions.minSdk.get().toInt()
        targetSdk = libs.versions.compileSdk.get().toInt()
        versionCode = gameVersionCode
        versionName = gameVersion
        // The engine is built for these two ABIs only (scripts/build-engine.sh).
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    // The engine's shared libraries, one per ABI, built by scripts/build-engine.sh.
    sourceSets["main"].jniLibs.srcDirs(rootProject.layout.projectDirectory.dir("build/engine/jniLibs"))

    signingConfigs {
        if (uploadKeystore != null && uploadPassword != null) {
            create("upload") {
                storeFile = file(uploadKeystore)
                storePassword = uploadPassword
                keyAlias = "upload"
                keyPassword = uploadPassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfigs.findByName("upload")?.let { signingConfig = it }
        }
    }
    buildFeatures {
        compose = true
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    implementation(project(":shared"))
    implementation(libs.androidx.activity.compose)
}
