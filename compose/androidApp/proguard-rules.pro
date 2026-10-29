# The engine is reached through uniffi bindings, which call the Rust library with JNA. JNA reads
# Structure fields, Callback methods and Library interfaces by reflection, and the generated
# bindings (package uniffi.cw_ffi) are named by the native side, so none of it can be renamed or
# removed.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class uniffi.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure { <fields>; }
-dontwarn java.awt.**
-dontwarn javax.swing.**
-dontwarn java.lang.invoke.StringConcatFactory
# Desktop-only Compose and Skia code is compiled into the shared module but never loaded on Android.
-dontwarn org.jetbrains.skia.**
-dontwarn androidx.compose.desktop.**
