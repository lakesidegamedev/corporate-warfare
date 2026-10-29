# AGP shrinks the test APK with its own rules (testProguardFiles): leave the test code and its runner alone.
-dontwarn com.google.errorprone.annotations.**
-keep class net.lakesidegames.corporatewarfare.ShrunkEngineTest { *; }
-keep class androidx.test.** { *; }
-dontwarn androidx.test.**
