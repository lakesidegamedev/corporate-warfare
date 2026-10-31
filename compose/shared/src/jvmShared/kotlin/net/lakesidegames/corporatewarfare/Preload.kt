package net.lakesidegames.corporatewarfare

/** Reads the game's data files ahead of their first screen. */
object Preload {
    /**
     * Parses the logo, icons, world map, building models and cast portraits now. Meant for a
     * background thread at launch, so the first map frame does not stop to read them. Each file
     * loads once behind a lock, so a screen that needs one while this runs waits only for the rest.
     */
    fun run() {
        runCatching {
            PixelLogo.file
            SectorIcons.all
            WorldMap.preload()
            VoxelModels.has("city")
            PixelIcons.all
            CastPortraits.all
        }
    }
}
