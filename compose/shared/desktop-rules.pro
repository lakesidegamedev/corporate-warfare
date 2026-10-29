# Keep rules for the desktop release build (ProGuard, shrink only: no obfuscation, no rewriting).
# Compose, coroutines and the Kotlin standard library bring their own rules; these are the parts that
# are reached by name instead of by a call.

# JNA finds the engine's functions and structure fields by reflection and by name.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-dontwarn com.sun.jna.**

# The engine bindings (uniffi) are loaded through JNA the same way: interface methods, structure
# fields and callback classes are looked up by name.
-keep class uniffi.** { *; }

# Skia and its native library call back into these classes through JNI.
-keep class org.jetbrains.skia.** { *; }
-keep class org.jetbrains.skiko.** { *; }
-dontwarn org.jetbrains.skiko.**

# kotlinx.serialization: generated serializers and companions are found by name.
-keepattributes *Annotation*, InnerClasses, Signature, RuntimeVisibleAnnotations
-keepclassmembers class **$$serializer { *; }
-keepclassmembers @kotlinx.serialization.Serializable class ** {
    static **$* *;
    *** Companion;
    *** INSTANCE;
    kotlinx.serialization.KSerializer serializer(...);
}
-keep,includedescriptorclasses class net.lakesidegames.corporatewarfare.**$$serializer { *; }

# The game's own entry points that reflection or the OS reach: the main class, and the Windows
# single-instance code that goes through JNA.
-keep class net.lakesidegames.corporatewarfare.MainKt { public static void main(java.lang.String[]); }
-keep class net.lakesidegames.corporatewarfare.SingleInstance** { *; }

# Warnings from optional libraries that the desktop build does not ship.
-dontwarn kotlinx.coroutines.debug.**
-dontwarn java.lang.management.**
-dontwarn org.jspecify.**
-dontwarn javax.annotation.**
