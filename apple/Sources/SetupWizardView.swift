import SwiftUI
import UIKit

/// Where first-run setup stands, in UserDefaults.
enum SetupKeys {
    static let state = "cw.setup.state"
    static let fresh = "fresh"
    static let upgrade = "upgrade"
    static let done = "done"
}

enum SetupGate {
    /// Called first at launch, before `GraphicsSettings.migrate()` writes a preset for new installs.
    /// A stored graphics preset or finished walkthrough means a player from an earlier version, who
    /// gets "What's new" instead of the full wizard. Costs two reads of UserDefaults.
    static func prepare(_ defaults: UserDefaults = .standard, uiTesting: Bool = false) {
        guard defaults.string(forKey: SetupKeys.state) == nil else { return }
        let existing = defaults.string(forKey: GraphicsKeys.preset) != nil || defaults.object(forKey: OnboardingKeys.completed) != nil
        defaults.set(uiTesting ? SetupKeys.done : (existing ? SetupKeys.upgrade : SetupKeys.fresh), forKey: SetupKeys.state)
    }
}

/// Turns a measured frame time and the screen into a graphics preset; the same thresholds as the
/// Windows, Linux and Android builds. Ultra is never advised: it uncaps the frame rate.
enum SetupAdvice {
    static let benchWidth = 480
    static let benchHeight = 320
    static let highMs = 50.0
    static let mediumMs = 130.0

    static func recommend(frameMs: Double, screenPixels: Double) -> GraphicsPreset {
        let ratio = max(screenPixels / Double(benchWidth * benchHeight), 1)
        let projected = frameMs * ratio.squareRoot()
        if projected <= highMs { return .high }
        if projected <= mediumMs { return .medium }
        return .low
    }

    /// What Skip applies: the same High a fresh install already starts on.
    static let defaultPreset = GraphicsPreset.high
}

/// A short offscreen drawing of the world's outlines, timed. Runs on a background queue, never on the
/// main thread, and only when the Graphics step is shown.
enum SetupBenchmark {
    struct Result: Equatable {
        let frameMs: Double
        let frames: Int
    }

    static func measure(budget: TimeInterval = 1.0) -> Result? {
        let map = WorldMap.shared
        let w = SetupAdvice.benchWidth
        let h = SetupAdvice.benchHeight
        guard let context = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        let radius = Double(h) * 0.45
        func frame(_ index: Int) {
            let spin = Double(index) * 2 * .pi / 180
            context.setFillColor(CGColor(red: 0.02, green: 0.035, blue: 0.055, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: w, height: h))
            context.setFillColor(CGColor(red: 0.45, green: 0.38, blue: 0.20, alpha: 1))
            context.setStrokeColor(CGColor(red: 0.9, green: 0.9, blue: 0.9, alpha: 0.8))
            for country in map.countries {
                for ring in country.rings {
                    let path = CGMutablePath()
                    var count = 0
                    for point in ring.points {
                        let z = point.cosLat * cos(point.lon - spin)
                        if z <= 0 { continue }
                        let x = point.cosLat * sin(point.lon - spin)
                        let screen = CGPoint(x: Double(w) / 2 + radius * x, y: Double(h) / 2 - radius * point.sinLat)
                        if count == 0 { path.move(to: screen) } else { path.addLine(to: screen) }
                        count += 1
                    }
                    if count >= 3 {
                        context.addPath(path)
                        context.drawPath(using: .fillStroke)
                    }
                }
            }
        }
        frame(-1)
        var times: [Double] = []
        let start = Date()
        var index = 0
        while (times.count < 3 || Date().timeIntervalSince(start) < budget) && times.count < 200 {
            let begin = Date()
            frame(index)
            index += 1
            times.append(Date().timeIntervalSince(begin) * 1000)
        }
        return Result(frameMs: times.sorted()[times.count / 2], frames: times.count)
    }
}

@MainActor
final class SetupModel: ObservableObject {
    enum State: Equatable { case idle, running, done(SetupBenchmark.Result), unavailable }
    @Published var benchmark: State = .idle

    func run() {
        guard benchmark == .idle else { return }
        benchmark = .running
        Task {
            let result = await Task.detached(priority: .utility) { SetupBenchmark.measure() }.value
            benchmark = result.map { .done($0) } ?? .unavailable
        }
    }
}

/// The graphics keys, so Cancel in "Set up again" can put them back.
private let graphicsKeyList = [
    GraphicsKeys.preset, GraphicsKeys.mapDetail, GraphicsKeys.tilt, GraphicsKeys.extrude, GraphicsKeys.lighting, GraphicsKeys.dayNight,
    GraphicsKeys.cityLights, GraphicsKeys.earth, GraphicsKeys.clouds, GraphicsKeys.traffic, GraphicsKeys.buildings, GraphicsKeys.frameRate,
    GraphicsKeys.countryside, GraphicsKeys.rivals, GraphicsKeys.interstates, GraphicsKeys.seasons, GraphicsKeys.labels,
]

enum SetupStep: Int, CaseIterable {
    case welcome, graphics, privacy, start
}

/// First-run setup in the 8-bit panels: Welcome, Graphics, Privacy, Start. Updates are the store's job
/// on Apple platforms, so that step does not appear. The preset applies as it is picked, so the live
/// globe shows it; Cancel (set up again) puts the old graphics back, Skip (first launch) keeps the defaults.
struct SetupWizardView: View {
    let firstRun: Bool
    var startStep = SetupStep.welcome
    let onFinish: () -> Void
    @Environment(AppStore.self) private var store
    @StateObject private var model = SetupModel()
    @State private var step = SetupStep.welcome
    @State private var preset = GraphicsPreset.high
    @State private var touched = false
    @State private var analytics = Telemetry.enabled
    @State private var tutorial = true
    @State private var saved: [String: Any] = [:]

    var body: some View {
        GeometryReader { proxy in
            let scale = UIScreen.main.scale
            let pixels = Double(proxy.size.width * scale * proxy.size.height * scale)
            VStack(spacing: 0) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        dots
                        content(pixels: pixels)
                    }
                    .padding(16)
                    .frame(maxWidth: 640)
                    .frame(maxWidth: .infinity)
                }
                navigation
            }
            .background(Theme.ink)
            .onChange(of: model.benchmark) { _, result in
                if !touched, case .done(let measured) = result {
                    choose(SetupAdvice.recommend(frameMs: measured.frameMs, screenPixels: pixels), byPlayer: false)
                }
            }
        }
        .onAppear {
            step = startStep
            let defaults = UserDefaults.standard
            for key in graphicsKeyList { saved[key] = defaults.object(forKey: key) }
            preset = GraphicsPreset(rawValue: defaults.string(forKey: GraphicsKeys.preset) ?? "") .flatMap { $0 == .custom ? nil : $0 } ?? .high
            tutorial = !store.tutorial.hidden
        }
        .onChange(of: step) { _, now in if now == .graphics { model.run() } }
        .interactiveDismissDisabled()
    }

    private func choose(_ next: GraphicsPreset, byPlayer: Bool) {
        preset = next
        if byPlayer { touched = true }
        next.values?.write()
        UserDefaults.standard.set(next.rawValue, forKey: GraphicsKeys.preset)
    }

    private func finish(apply: Bool) {
        let defaults = UserDefaults.standard
        if apply {
            Telemetry.enabled = analytics
            store.tutorial.setHidden(!tutorial)
            choose(preset, byPlayer: false)
        } else if !firstRun {
            for key in graphicsKeyList {
                if let value = saved[key] { defaults.set(value, forKey: key) } else { defaults.removeObject(forKey: key) }
            }
        }
        onFinish()
    }

    private var dots: some View {
        HStack(spacing: 6) {
            ForEach(SetupStep.allCases, id: \.rawValue) { item in
                Rectangle()
                    .fill(item.rawValue <= step.rawValue ? Theme.gold : Theme.panelRaised)
                    .frame(width: item == step ? 14 : 10, height: item == step ? 14 : 10)
            }
            Text("STEP \(step.rawValue + 1) OF \(SetupStep.allCases.count)")
                .font(Pixel.font(11))
                .foregroundStyle(Theme.secondary)
                .padding(.leading, 6)
        }
    }

    private var navigation: some View {
        HStack(spacing: 8) {
            Button("Back") { if let previous = SetupStep(rawValue: step.rawValue - 1) { step = previous } }
                .buttonStyle(PixelButtonStyle())
                .disabled(step == .welcome)
            Button(firstRun ? "Skip" : "Cancel") { finish(apply: firstRun) }
                .buttonStyle(PixelButtonStyle())
            Spacer(minLength: 0)
            if let next = SetupStep(rawValue: step.rawValue + 1) {
                Button("Next") { step = next }
                    .buttonStyle(PixelButtonStyle(prominent: true)).tint(Theme.gold).foregroundStyle(Theme.ink)
            } else {
                Button("Play") { finish(apply: true) }
                    .buttonStyle(PixelButtonStyle(prominent: true)).tint(Theme.gold).foregroundStyle(Theme.ink)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .frame(maxWidth: 640)
        .frame(maxWidth: .infinity)
        .background(Theme.ink)
    }

    private func heading(_ text: String) -> some View {
        Text(text.uppercased()).font(Pixel.font(22, bold: true)).foregroundStyle(Theme.primaryText)
    }

    @ViewBuilder
    private func content(pixels: Double) -> some View {
        switch step {
        case .welcome:
            Panel {
                VStack(alignment: .leading, spacing: 10) {
                    heading("Welcome")
                    GlobeBackdrop(offsetX: 0)
                        .frame(height: 220)
                        .clipShape(Pixel.shape)
                    Text("Build companies, run them through real booms and busts, and out-earn your rivals. A few quick questions first, so the game suits this device. Skip any time to take the defaults.")
                        .font(.callout).foregroundStyle(Theme.primaryText)
                }
            }
        case .graphics:
            Panel {
                VStack(alignment: .leading, spacing: 10) {
                    heading("Graphics")
                    switch model.benchmark {
                    case .idle, .running:
                        Text("Testing how fast this device draws the map. It takes about a second.").font(.callout).foregroundStyle(Theme.gold)
                    case .done(let result):
                        Text("The test drew \(result.frames) map frames at \(Int(result.frameMs.rounded())) ms each. Recommended: \(SetupAdvice.recommend(frameMs: result.frameMs, screenPixels: pixels).title).")
                            .font(.callout).foregroundStyle(Theme.primaryText)
                    case .unavailable:
                        Text("The test could not run here, so the choice below starts on High.").font(.callout).foregroundStyle(Theme.secondary)
                    }
                    Text("Drawing with: Metal").font(.callout).foregroundStyle(Theme.primaryText)
                    PixelChoices(options: GraphicsPreset.choices.map { ($0, $0.title) },
                                 selection: Binding(get: { preset }, set: { choose($0, byPlayer: true) }), label: "Quality")
                    Text("You can change every graphics option later in Settings.").font(.caption).foregroundStyle(Theme.secondary)
                    GlobeBackdrop(offsetX: 0)
                        .frame(height: 200)
                        .clipShape(Pixel.shape)
                    Text("PREVIEW AT \(preset.title.uppercased())").font(Pixel.font(11)).foregroundStyle(Theme.secondary)
                }
            }
        case .privacy:
            Panel {
                VStack(alignment: .leading, spacing: 10) {
                    heading("Privacy")
                    Toggle(isOn: $analytics) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Share anonymous market data").font(.callout).foregroundStyle(Theme.primaryText)
                            Text("Every five weeks, the goods made, sold and priced, and how your company is doing, are sent with a random id. No names or account details.")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                    }
                    .tint(Theme.gold)
                    Eyebrow(text: "What that means")
                    ForEach(SetupCopy.privacyPoints, id: \.self) { point in
                        Text("- \(point)").font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
            }
        case .start:
            Panel {
                VStack(alignment: .leading, spacing: 10) {
                    heading("Start")
                    Toggle(isOn: $tutorial) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Guided tutorial").font(.callout).foregroundStyle(Theme.primaryText)
                            Text("Eight short steps that tick off as you play. Turn it off if you know the game; it is under Settings if you change your mind.")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                    }
                    .tint(Theme.gold)
                    if ProcessInfo.processInfo.isMacCatalystApp {
                        Eyebrow(text: "Where your saves are kept")
                        Text(SaveManager.shared.directory.path).font(.caption.monospaced()).foregroundStyle(Theme.primaryText)
                    }
                    Text("Graphics: \(preset.title). Anonymous data: \(analytics ? "on" : "off"). Tutorial: \(tutorial ? "on" : "off").")
                        .font(.callout).foregroundStyle(Theme.primaryText)
                }
            }
        }
    }
}

enum SetupCopy {
    static let privacyPoints = [
        "No account is needed and nothing that names you, contacts you or identifies your device is sent.",
        "The snapshot is used only to balance the game.",
        "Saves and settings stay on this device.",
        "You can switch this off here, or later in Settings.",
    ]
}

/// The one screen an upgrading player sees instead of the full wizard.
struct WhatsNewView: View {
    let onRunSetup: () -> Void
    let onContinue: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Panel {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("WHAT'S NEW IN 1.9").font(Pixel.font(22, bold: true)).foregroundStyle(Theme.primaryText)
                        ForEach(WhatsNewView.points, id: \.self) { point in
                            Text("- \(point)").font(.callout).foregroundStyle(Theme.primaryText)
                        }
                    }
                }
                Button("Run setup", action: onRunSetup)
                    .buttonStyle(PixelButtonStyle(prominent: true)).tint(Theme.gold).foregroundStyle(Theme.ink)
                Button("Continue to the game", action: onContinue)
                    .buttonStyle(PixelButtonStyle())
            }
            .padding(16)
            .frame(maxWidth: 640)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
    }

    static let points = [
        "Every scenario has dated events from its own period that play out in your career.",
        "The game does less work at the fastest speed, so tabs and division sheets no longer freeze.",
        "A first-run setup tests your graphics and sets privacy and the tutorial. Your current settings are kept unless you run it.",
        "Careers you save now will not open in older versions of the game. Your existing saves open as normal.",
    ]
}
