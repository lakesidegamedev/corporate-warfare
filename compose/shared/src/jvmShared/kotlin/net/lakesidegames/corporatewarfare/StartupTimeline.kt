package net.lakesidegames.corporatewarfare

/**
 * A timestamped list of the steps between launch and the main menu, printed to stderr when the
 * environment has CW_STARTUP_TIMELINE set. Off by default, so a normal launch pays one lookup.
 * Times are milliseconds since [origin]: the desktop app sets it to the JVM's start, so the time
 * before main() counts too; otherwise it is the first use.
 */
object StartupTimeline {
    private val enabled = System.getenv("CW_STARTUP_TIMELINE") != null
    private val marks = mutableListOf<Pair<String, Long>>()

    @Volatile
    var origin: Long = System.currentTimeMillis()

    private fun sinceJvmStart(): Long = System.currentTimeMillis() - origin

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
