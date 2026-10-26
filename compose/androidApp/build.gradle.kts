plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "net.lakesidegames.corporatewarfare"
    compileSdk = libs.versions.compileSdk.get().toInt()

    defaultConfig {
        applicationId = "net.lakesidegames.corporatewarfare"
        minSdk = libs.versions.minSdk.get().toInt()
        targetSdk = libs.versions.compileSdk.get().toInt()
        versionCode = 1
        versionName = "1.3.1"
        // The engine is built for these two ABIs only (scripts/build-engine.sh).
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    // The engine's shared libraries, one per ABI, built by scripts/build-engine.sh.
    sourceSets["main"].jniLibs.srcDirs(rootProject.layout.projectDirectory.dir("build/engine/jniLibs"))

    buildTypes {
        release {
            isMinifyEnabled = false
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
