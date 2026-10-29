# Added only to the CI emulator check (-Pcw.smoke=true). The instrumented test is compiled
# against the unshrunk classes, so the few it calls directly keep their names. Everything the
# game itself reaches is still shrunk, renamed and optimized as in the shipped build.
-keep class net.lakesidegames.corporatewarfare.GameStore { public *; }
-keepnames class kotlinx.coroutines.CoroutineScope
-keep class kotlinx.coroutines.CoroutineScopeKt { public *; }
-keep class kotlinx.coroutines.Dispatchers { public *; }
-keep class kotlinx.coroutines.CoroutineDispatcher
-keep class kotlin.coroutines.CoroutineContext
# The test runner's Kotlin code calls the compiler's null-check helpers, which R8 otherwise trims.
-keep,allowobfuscation class kotlin.jvm.internal.** { *; }
