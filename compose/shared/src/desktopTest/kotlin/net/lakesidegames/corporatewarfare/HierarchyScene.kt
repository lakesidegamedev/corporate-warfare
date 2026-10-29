package net.lakesidegames.corporatewarfare

import uniffi.cw_ffi.LaneDto
import uniffi.cw_ffi.LaneOwnerDto
import uniffi.cw_ffi.MapDivisionDto
import uniffi.cw_ffi.MapPlantDto

/** A busy Great Lakes scene for the map tests: the player's plants, several rivals, and lanes owned by the player, a rival and outside carriers. */
internal object HierarchyScene {
    val divisions = listOf(
        MapDivisionDto(1uL, "OH-CEN", "manufacturing", 6u, 6u), MapDivisionDto(1uL, "MI-DET", "energy", 9u, 9u),
        MapDivisionDto(1uL, "PA-WES", "technology", 2u, 2u), MapDivisionDto(1uL, "IL-CHI", "logistics", 4u, 4u),
    )
    val shading = mapOf("OH-CEN" to 1.0, "MI-DET" to 0.8, "PA-WES" to 0.3, "IL-CHI" to 0.5)

    private fun plant(handle: Int, firm: ULong, name: String, player: Boolean, region: String, industry: String, blocks: Int, status: String = "Running") =
        MapPlantDto(handle.toULong(), firm, name, player, region, industry, "balanced", blocks.toUInt(), status, 8000)

    val plants = listOf(
        plant(1, 1uL, "Player", true, "OH-CEN", "manufacturing", 6), plant(2, 1uL, "Player", true, "MI-DET", "energy", 9),
        plant(3, 1uL, "Player", true, "PA-WES", "technology", 2), plant(4, 1uL, "Player", true, "IL-CHI", "logistics", 4),
        plant(10, 2uL, "Rival A", false, "OH-CEN", "chemical_industries", 5), plant(11, 3uL, "Rival B", false, "OH-CEN", "retail", 3),
        plant(12, 2uL, "Rival A", false, "OH-CEN", "healthcare", 2), plant(13, 4uL, "Rival C", false, "OH-CEN", "financial", 6, "Mothballed"),
        plant(14, 3uL, "Rival B", false, "MI-DET", "technology", 4), plant(15, 4uL, "Rival C", false, "MI-DET", "media", 2),
        plant(16, 2uL, "Rival A", false, "MI-DET", "manufacturing", 7), plant(17, 5uL, "Rival D", false, "PA-WES", "extraction", 6),
        plant(18, 5uL, "Rival D", false, "PA-WES", "energy", 3, "Building"), plant(19, 3uL, "Rival B", false, "IL-CHI", "financial", 8),
        plant(20, 4uL, "Rival C", false, "IL-CHI", "real_estate", 3), plant(21, 2uL, "Rival A", false, "IL-CHI", "telecommunications", 2),
    )

    private fun lane(a: String, b: String, mode: String, owners: List<LaneOwnerDto>, mine: Boolean, outside: Int): LaneDto =
        LaneDto(
            a, b, a, b, 0u, mode, mode, 400u, 40, 30, 0, outside, 7500, 1000L, 1200L, 0u, owners, 0u,
            if (mine) 20 else 0, if (mine) 15 else 0, 0, 0L,
        )

    val lanes = listOf(
        lane("MI-DET", "OH-CEN", "road", listOf(LaneOwnerDto(1uL, "Player", 30, true)), true, 0),
        lane("IL-CHI", "MI-DET", "rail", listOf(LaneOwnerDto(2uL, "Rival A", 30, false)), false, 2),
        lane("OH-CEN", "PA-WES", "road", listOf(LaneOwnerDto(3uL, "Rival B", 25, false)), false, 4),
        lane("IL-CHI", "OH-CEN", "road", listOf(LaneOwnerDto(4uL, "Rival C", 5, false)), false, 40),
    )
}
