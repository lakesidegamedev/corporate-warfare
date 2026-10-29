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

// -Pcw.smoke=true builds the shrunk release for the CI emulator check only: signed with the debug
// key so it installs, with extra keep rules for the classes the instrumented test calls, and the
// test runs against the release build type. It is never shipped.
val smoke = findProperty("cw.smoke") == "true"

android {
    namespace = "net.lakesidegames.corporatewarfare"
    compileSdk = libs.versions.compileSdk.get().toInt()

    defaultConfig {
        applicationId = "net.lakesidegames.corporatewarfare"
        minSdk = libs.versions.minSdk.get().toInt()
        targetSdk = libs.versions.compileSdk.get().toInt()
        versionCode = gameVersionCode
        versionName = gameVersion
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        testProguardFiles("proguard-smoke-test.pro")
        // The engine is built for these two ABIs only (scripts/build-engine.sh).
        // (With per-ABI APKs the splits block below chooses them instead; AGP rejects both.)
        if (findProperty("cw.splitAbi") != "true") {
            ndk {
                abiFilters += listOf("arm64-v8a", "x86_64")
            }
        }
    }

    // -Pcw.splitAbi=true makes assembleRelease produce one APK per ABI instead of one holding both.
    // The direct download is the arm64-v8a one (every current phone); the x86_64 one is for
    // emulators and stays a CI artifact. The Play bundle splits by ABI on its own and ignores this.
    splits {
        abi {
            isEnable = findProperty("cw.splitAbi") == "true"
            reset()
            include("arm64-v8a", "x86_64")
            isUniversalApk = false
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
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (smoke) {
                proguardFiles("proguard-smoke.pro")
                signingConfig = signingConfigs.getByName("debug")
            } else {
                signingConfigs.findByName("upload")?.let { signingConfig = it }
            }
        }
    }
    if (smoke) testBuildType = "release"
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
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
}
