package net.lakesidegames.corporatewarfare

/**
 * A timestamped list of the steps between launch and the main menu, printed to stderr when the
 * environment has CW_STARTUP_TIMELINE set. Off by default, so a normal launch pays one lookup.
 * Times are milliseconds since the JVM started.
 */
object StartupTimeline {
    private val enabled = System.getenv("CW_STARTUP_TIMELINE") != null
    private val marks = mutableListOf<Pair<String, Long>>()

    /** Milliseconds since the JVM started, so the time before main() counts too. */
    private fun sinceJvmStart(): Long =
        System.currentTimeMillis() - java.lang.management.ManagementFactory.getRuntimeMXBean().startTime

    @Synchronized
    fun mark(step: String) {
        if (!enabled) return
        marks += step to sinceJvmStart()
        System.err.println("startup %6d ms  %s".format(marks.last().second, step))
    }

    /** The first time the main menu is on screen: the number to compare between builds. */
    @Synchronized
    fun menuShown() {
        if (!enabled || marks.any { it.first == "main menu shown" }) return
        mark("main menu shown")
        if (System.getenv("CW_STARTUP_TIMELINE") == "exit") Runtime.getRuntime().halt(0)
    }
}
