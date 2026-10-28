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
                                    clouds: 0, traffic: 0, buildings: 0, frameRate: 30)
        case .medium:
            return GraphicsSettings(mapDetail: 1, tilt: 1, extrude: true, lighting: 1, dayNight: false, cityLights: true,
                                    clouds: 1, traffic: 1, buildings: 1, frameRate: 30)
        case .high:
            return GraphicsSettings(mapDetail: 2, tilt: 1, extrude: true, lighting: 2, dayNight: true, cityLights: true,
                                    clouds: 1, traffic: 1, buildings: 2, frameRate: 60)
        case .ultra:
            return GraphicsSettings(mapDetail: 2, tilt: 2, extrude: true, lighting: 2, dayNight: true, cityLights: true,
                                    clouds: 2, traffic: 2, buildings: 2, frameRate: 0)
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
    /// Clouds from the satellite cloud map: 0 off, 1 light, 2 heavy.
    var clouds: Int
    var traffic: Int
    var buildings: Int
    var frameRate: Int

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
        defaults.set(clouds, forKey: GraphicsKeys.clouds)
        defaults.set(traffic, forKey: GraphicsKeys.traffic)
        defaults.set(buildings, forKey: GraphicsKeys.buildings)
        defaults.set(frameRate, forKey: GraphicsKeys.frameRate)
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
