import SwiftUI

/// A named set of graphics options. Picking one writes every option; changing any single option
/// afterwards makes the preset Custom.
enum GraphicsPreset: String, CaseIterable, Identifiable {
    case low, medium, high, ultra, custom

    var id: String { rawValue }
    var title: String { rawValue.prefix(1).uppercased() + rawValue.dropFirst() }

    /// The four presets offered in the picker; Custom is shown as a caption.
    static let choices: [GraphicsPreset] = [.low, .medium, .high, .ultra]

    /// The values this preset writes, or nil for Custom.
    var values: GraphicsSettings? {
        switch self {
        case .low:
            return GraphicsSettings(mapDetail: 0, tilt: 0, extrude: false, lighting: 0, dayNight: false, cityLights: false,
                                    earth: false, clouds: 0, traffic: 0, buildings: 0, frameRate: 30,
                                    countryside: false, rivals: false, seasons: false)
        case .medium:
            return GraphicsSettings(mapDetail: 1, tilt: 1, extrude: true, lighting: 1, dayNight: false, cityLights: true,
                                    earth: true, clouds: 1, traffic: 1, buildings: 1, frameRate: 30,
                                    countryside: false)
        case .high:
            return GraphicsSettings(mapDetail: 2, tilt: 1, extrude: true, lighting: 2, dayNight: true, cityLights: true,
                                    earth: true, clouds: 2, traffic: 1, buildings: 2, frameRate: 60)
        case .ultra:
            return GraphicsSettings(mapDetail: 2, tilt: 2, extrude: true, lighting: 2, dayNight: true, cityLights: true,
                                    earth: true, clouds: 3, traffic: 2, buildings: 2, frameRate: 0)
        case .custom:
            return nil
        }
    }
}

/// Every graphics option a preset sets. Animations and the frame counter sit outside presets.
struct GraphicsSettings: Equatable {
    var mapDetail: Int
    var tilt: Int
    var extrude: Bool
    var lighting: Int
    var dayNight: Bool
    var cityLights: Bool
    /// The real Earth from NASA's Blue Marble under the map, with the seasons; off paints it flat.
    var earth: Bool
    /// Clouds from the satellite cloud map: 0 off, 1 thin, 2 light, 3 heavy.
    var clouds: Int
    var traffic: Int
    var buildings: Int
    var frameRate: Int
    /// Trees, fields, barns and houses on the ground up close.
    var countryside = true
    /// Rivals' plants as buildings up close.
    var rivals = true
    /// The drawn interstates.
    var interstates = true
    /// Snow, autumn and winter following the game's calendar; off keeps midsummer.
    var seasons = true
    /// Names on the map: 0 none, 1 cities, 2 cities and regions.
    var labels = 2

    /// The tilt setting as a pitch in radians: off, subtle 28 degrees, full 48 degrees.
    static func pitch(forTilt tilt: Int) -> Double {
        switch tilt {
        case 1: return 28 * .pi / 180
        case 2: return 48 * .pi / 180
        default: return 0
        }
    }

    /// Seconds between overlay frames for a frame-rate setting, nil when unlimited.
    static func frameInterval(_ frameRate: Int) -> Double? {
        frameRate > 0 ? 1.0 / Double(frameRate) : nil
    }

    /// Writes every option to the defaults store.
    func write(to defaults: UserDefaults = .standard) {
        defaults.set(mapDetail, forKey: GraphicsKeys.mapDetail)
        defaults.set(tilt, forKey: GraphicsKeys.tilt)
        defaults.set(extrude, forKey: GraphicsKeys.extrude)
        defaults.set(lighting, forKey: GraphicsKeys.lighting)
        defaults.set(dayNight, forKey: GraphicsKeys.dayNight)
        defaults.set(cityLights, forKey: GraphicsKeys.cityLights)
        defaults.set(earth, forKey: GraphicsKeys.earth)
        defaults.set(clouds, forKey: GraphicsKeys.clouds)
        defaults.set(traffic, forKey: GraphicsKeys.traffic)
        defaults.set(buildings, forKey: GraphicsKeys.buildings)
        defaults.set(frameRate, forKey: GraphicsKeys.frameRate)
        defaults.set(countryside, forKey: GraphicsKeys.countryside)
        defaults.set(rivals, forKey: GraphicsKeys.rivals)
        defaults.set(interstates, forKey: GraphicsKeys.interstates)
        defaults.set(seasons, forKey: GraphicsKeys.seasons)
        defaults.set(labels, forKey: GraphicsKeys.labels)
    }

    /// Fills in a missing preset once: a fresh install gets High. An older install keeps its map
    /// detail and animations and takes High for the new options, shown as Custom when its map
    /// detail differs from High's.
    static func migrate(_ defaults: UserDefaults = .standard) {
        // Clouds came back after presets were stored: give them the stored preset's amount.
        if defaults.object(forKey: GraphicsKeys.clouds) == nil, let stored = defaults.string(forKey: GraphicsKeys.preset),
           let values = GraphicsPreset(rawValue: stored)?.values {
            defaults.set(values.clouds, forKey: GraphicsKeys.clouds)
        }
        // Clouds gained a thin level, so the top is now 3; anything stored above that is heavy.
        if defaults.integer(forKey: GraphicsKeys.clouds) > 3 { defaults.set(3, forKey: GraphicsKeys.clouds) }
        // The satellite Earth came after presets were stored: on for every preset but Low.
        if defaults.object(forKey: GraphicsKeys.earth) == nil, let stored = defaults.string(forKey: GraphicsKeys.preset) {
            defaults.set(GraphicsPreset(rawValue: stored)?.values?.earth ?? true, forKey: GraphicsKeys.earth)
        }
        // Countryside, rivals, interstates, seasons and labels came later still: the stored
        // preset's values, or all on for a custom mix.
        if let stored = defaults.string(forKey: GraphicsKeys.preset) {
            let values = GraphicsPreset(rawValue: stored)?.values
            if defaults.object(forKey: GraphicsKeys.countryside) == nil {
                defaults.set(values?.countryside ?? true, forKey: GraphicsKeys.countryside)
            }
            if defaults.object(forKey: GraphicsKeys.rivals) == nil { defaults.set(values?.rivals ?? true, forKey: GraphicsKeys.rivals) }
            if defaults.object(forKey: GraphicsKeys.interstates) == nil {
                defaults.set(values?.interstates ?? true, forKey: GraphicsKeys.interstates)
            }
            if defaults.object(forKey: GraphicsKeys.seasons) == nil { defaults.set(values?.seasons ?? true, forKey: GraphicsKeys.seasons) }
            if defaults.object(forKey: GraphicsKeys.labels) == nil { defaults.set(values?.labels ?? 2, forKey: GraphicsKeys.labels) }
        }
        guard defaults.string(forKey: GraphicsKeys.preset) == nil, var high = GraphicsPreset.high.values else { return }
        var preset = GraphicsPreset.high
        if defaults.object(forKey: GraphicsKeys.mapDetail) != nil {
            let stored = defaults.integer(forKey: GraphicsKeys.mapDetail)
            if stored != high.mapDetail { preset = .custom }
            high.mapDetail = stored
        }
        high.write(to: defaults)
        defaults.set(preset.rawValue, forKey: GraphicsKeys.preset)
    }
}
