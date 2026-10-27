package net.lakesidegames.corporatewarfare

import java.net.HttpURLConnection
import java.net.URL

/**
 * Anonymous market-balance analytics, the same as the iPhone build sends. Every five game weeks
 * the engine's snapshot JSON is posted to the Lakeside ops ingest with a random install id. No
 * names, account or device data. On by default; the player can switch it off in Settings.
 * Uploads never block play and a failed upload is dropped, not retried.
 */
object Telemetry {
    private const val ENDPOINT = "https://ops.lakesidegames.net/api/cw/telemetry"

    /** True on the weeks a snapshot is due. */
    fun due(week: UInt): Boolean = week > 0u && week % 5u == 0u

    /** The request body: the install, the app, and the engine's snapshot verbatim. */
    fun envelope(installId: String, version: String, build: String, snapshotJson: String): String =
        "{\"install_id\":\"$installId\",\"app_version\":\"$version\",\"build\":\"$build\",\"snapshot\":$snapshotJson}"

    /** Post one snapshot. Blocking; call off the UI thread. Every failure is swallowed. */
    fun post(body: String) {
        runCatching {
            val connection = URL(ENDPOINT).openConnection() as HttpURLConnection
            try {
                connection.requestMethod = "POST"
                connection.connectTimeout = 10_000
                connection.readTimeout = 10_000
                connection.doOutput = true
                connection.setRequestProperty("Content-Type", "application/json")
                connection.outputStream.use { it.write(body.toByteArray()) }
                connection.responseCode
            } finally {
                connection.disconnect()
            }
        }
    }
}
