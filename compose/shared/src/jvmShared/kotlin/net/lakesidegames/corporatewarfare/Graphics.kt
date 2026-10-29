package net.lakesidegames.corporatewarfare

import kotlin.math.PI

/**
 * Every graphics option in one value. The map and the menu read it once per frame; the settings
 * page edits it through [AppSettings.chooseGraphics]. Same keys and presets as the iPhone app.
 */
data class GraphicsConfig(
    val detail: MapDetail = MapDetail.High,
    /** 3D tilt: 0 off, 1 subtle, 2 full. */
    val tilt: Int = 1,
    /** Regions stand up as tiles, taller where the active layer is stronger. */
    val extrude: Boolean = true,
    /** 0 flat colours, 1 standard sunlight and atmosphere, 2 cinematic. */
    val lighting: Int = 2,
    /** The sun moves round the world; off keeps a fixed afternoon light. */
    val dayNight: Boolean = true,
    val cityLights: Boolean = true,
    /** The real Earth from NASA's Blue Marble under the map, with the seasons; off paints it flat. */
    val earth: Boolean = true,
    /** Clouds from the satellite cloud map: 0 off, 1 thin, 2 light, 3 heavy. */
    val clouds: Int = 1,
    /** Freight along the lanes and cars on the highways: 0 off, 1 light, 2 busy. */
    val traffic: Int = 1,
    /** 0 flat badges, 1 divisions as buildings, 2 also city skylines. */
    val buildings: Int = 2,
    /** Frames a second for moving effects; 0 is as fast as the display. */
    val frameRate: Int = 60,
    val animations: Boolean = true,
    val showFps: Boolean = false,
    /** Trees, fields, barns and houses on the ground up close. */
    val countryside: Boolean = true,
    /** Rivals' plants as buildings up close. */
    val rivals: Boolean = true,
    /** The drawn interstates. */
    val interstates: Boolean = true,
    /** Snow, autumn and winter following the game's calendar; off keeps midsummer. */
    val seasons: Boolean = true,
    /** Names on the map: 0 none, 1 cities, 2 cities and regions. */
    val labels: Int = 2,
    /** The new GPU map (desktop preview): stylized terrain, roads and buildings drawn on the graphics card. Off by default. */
    val newMap: Boolean = false,
) {
    /** The tilt in radians once the map is flat. */
    val pitch: Double get() = when (tilt) { 0 -> 0.0; 1 -> 28 * PI / 180; else -> 48 * PI / 180 }

    /** Whether anything on the map moves on its own, so the frame clock needs to run. */
    val moves: Boolean get() = animations && (traffic > 0 || clouds > 0 || buildings > 0 || dayNight || lighting == 2)

    /** The preset this matches, or null when the player has mixed their own. */
    val preset: GraphicsPreset? get() = GraphicsPreset.entries.firstOrNull { it.config(animations, showFps) == copy(newMap = false) }

    companion object {
        /** What the map draws with no settings file, as in tests and previews. */
        val Default = GraphicsPreset.High.config(animations = true, showFps = false)
    }
}

enum class GraphicsPreset(val title: String) {
    Low("Low"), Medium("Medium"), High("High"), Ultra("Ultra");

    fun config(animations: Boolean, showFps: Boolean): GraphicsConfig = when (this) {
        Low -> GraphicsConfig(
            MapDetail.Low, tilt = 0, extrude = false, lighting = 0, dayNight = false, cityLights = false, earth = false, clouds = 0,
            traffic = 0, buildings = 0, frameRate = 30, countryside = false, rivals = false, seasons = false,
        )
        Medium -> GraphicsConfig(
            MapDetail.Medium, tilt = 1, extrude = true, lighting = 1, dayNight = false, cityLights = true, earth = true, clouds = 1,
            traffic = 1, buildings = 1, frameRate = 30, countryside = false,
        )
        High -> GraphicsConfig(MapDetail.High, tilt = 1, extrude = true, lighting = 2, dayNight = true, cityLights = true, earth = true, clouds = 2, traffic = 1, buildings = 2, frameRate = 60)
        Ultra -> GraphicsConfig(MapDetail.High, tilt = 2, extrude = true, lighting = 2, dayNight = true, cityLights = true, earth = true, clouds = 3, traffic = 2, buildings = 2, frameRate = 0)
    }.copy(animations = animations, showFps = showFps)
}

/** One row on the graphics settings page: its title, choices and the plain-language note. */
internal class GraphicsOption(
    val title: String,
    val note: (GraphicsConfig) -> String,
    val choices: List<String>,
    val get: (GraphicsConfig) -> Int,
    val set: (GraphicsConfig, Int) -> GraphicsConfig,
    /** Only shown where the desktop shell offers the GPU map. */
    val gpuOnly: Boolean = false,
)

private fun Boolean.int() = if (this) 1 else 0

internal val graphicsOptions = listOf(
    GraphicsOption("Map detail", { it.detail.note }, MapDetail.entries.map { it.title }, { it.detail.ordinal }, { c, v -> c.copy(detail = MapDetail.entries[v]) }),
    GraphicsOption("3D tilt", { "Tilts the map as you zoom in, so the ground recedes into the distance." }, listOf("Off", "Subtle", "Full"), { it.tilt }, { c, v -> c.copy(tilt = v) }),
    GraphicsOption("Raised regions", { "Regions stand up as tiles, taller where the active layer is stronger." }, listOf("Off", "On"), { it.extrude.int() }, { c, v -> c.copy(extrude = v == 1) }),
    GraphicsOption("Lighting", { "Sunlight, shadows and the atmosphere around the globe." }, listOf("Off", "Standard", "Cinematic"), { it.lighting }, { c, v -> c.copy(lighting = v) }),
    GraphicsOption("Day and night", { "The sun moves across the world and night falls on the map." }, listOf("Off", "On"), { it.dayNight.int() }, { c, v -> c.copy(dayNight = v == 1) }),
    GraphicsOption("City lights", { "Cities glow on the night side." }, listOf("Off", "On"), { it.cityLights.int() }, { c, v -> c.copy(cityLights = v == 1) }),
    GraphicsOption("Satellite Earth", { "The real Earth from NASA imagery under the map, with snow, autumn and winter following the game's calendar." }, listOf("Off", "On"), { it.earth.int() }, { c, v -> c.copy(earth = v == 1) }),
    GraphicsOption("Clouds", { "Real satellite clouds that drift with the trade winds and westerlies. Heavy clouds cast shadows. The cloud button on the map steps through these." }, listOf("Off", "Thin", "Light", "Heavy"), { it.clouds }, { c, v -> c.copy(clouds = v) }),
    GraphicsOption("Traffic", { "Freight moves along the lanes and cars along the highways." }, listOf("Off", "Light", "Busy"), { it.traffic }, { c, v -> c.copy(traffic = v) }),
    GraphicsOption("Buildings", { "Shows your divisions as buildings, and city skylines." }, listOf("Flat", "Your divisions", "Divisions and cities"), { it.buildings }, { c, v -> c.copy(buildings = v) }),
    GraphicsOption("Countryside", { "Trees, fields, barns and houses on the ground when zoomed in." }, listOf("Off", "On"), { it.countryside.int() }, { c, v -> c.copy(countryside = v == 1) }),
    GraphicsOption("Rivals' buildings", { "Rivals' plants stand on the map, greyed, when zoomed in." }, listOf("Off", "On"), { it.rivals.int() }, { c, v -> c.copy(rivals = v == 1) }),
    GraphicsOption("Interstates", { "The highways drawn over the map." }, listOf("Off", "On"), { it.interstates.int() }, { c, v -> c.copy(interstates = v == 1) }),
    GraphicsOption("Seasons", { "Snow in winter and autumn colours, following the game's calendar." }, listOf("Off", "On"), { it.seasons.int() }, { c, v -> c.copy(seasons = v == 1) }),
    GraphicsOption("Labels", { "Names on the map." }, listOf("Off", "Cities", "All"), { it.labels }, { c, v -> c.copy(labels = v) }),
    GraphicsOption("Frame rate", { "Lower saves battery." }, listOf("30", "60", "Unlimited"), { when (it.frameRate) { 30 -> 0; 60 -> 1; else -> 2 } }, { c, v -> c.copy(frameRate = listOf(30, 60, 0)[v]) }),
    GraphicsOption("Animations", { "Camera flights, the menu and moving effects." }, listOf("Off", "On"), { it.animations.int() }, { c, v -> c.copy(animations = v == 1) }),
    GraphicsOption(
        "New map (preview)",
        { "A new look for the map, drawn on your graphics card: terrain, roads, rails and buildings. A preview for testers; the frame rate counter shows what it costs. If your computer has no usable graphics card the game keeps the current map." },
        listOf("Off", "On"), { it.newMap.int() }, { c, v -> c.copy(newMap = v == 1) }, gpuOnly = true,
    ),
    GraphicsOption("Show frame rate", { "A frame counter in the corner of the map." }, listOf("Off", "On"), { it.showFps.int() }, { c, v -> c.copy(showFps = v == 1) }),
)
