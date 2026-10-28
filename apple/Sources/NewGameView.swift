import SwiftUI
import UIKit

/// The scenarios, read once.
enum ScenarioCatalogue {
    static let all: [ScenarioDto] = scenarios()
}

/// Scenario art from the bundle, cached. A missing file shows a plain panel.
struct ScenarioArt: View {
    let name: String
    private static let cache = NSCache<NSString, UIImage>()

    private static func load(_ name: String) -> UIImage? {
        if let hit = cache.object(forKey: name as NSString) { return hit }
        guard let url = Bundle.main.url(forResource: name, withExtension: "png"),
              let image = UIImage(contentsOfFile: url.path) else { return nil }
        cache.setObject(image, forKey: name as NSString)
        return image
    }

    var body: some View {
        if let image = Self.load(name) {
            Image(uiImage: image)
                .resizable()
                .scaledToFill()
        } else {
            Theme.panelRaised
        }
    }
}

private protocol WizardOption: Identifiable, Hashable {
    var title: String { get }
    var note: String { get }
}

private enum StartMode: String, CaseIterable, WizardOption {
    case guided, founder, scored
    var id: String { rawValue }
    var title: String {
        switch self {
        case .guided: return "Take over a company"
        case .founder: return "Found your own"
        case .scored: return "Five-year score"
        }
    }
    var note: String {
        switch self {
        case .guided: return "Start running 60 percent of an established company, with the tutorial."
        case .founder: return "Start with cash and no company. Pick a region and a strategy and build from nothing."
        case .scored: return "Take over a company and be scored at the end of year five. You can play on after."
        }
    }
    var guided: Bool { self != .founder }
    var fiveYear: Bool { self == .scored }
}

private enum StartLevel: String, CaseIterable, WizardOption {
    case easy, standard, hard
    var id: String { rawValue }
    var title: String { rawValue.prefix(1).uppercased() + rawValue.dropFirst() }
    var note: String {
        switch self {
        case .easy: return "$8 million to start. Rivals defend but never attack your markets."
        case .standard: return "$5 million to start. Cost cutters, consolidators and vultures fight for share."
        case .hard: return "$3 million to start. Every rival undercuts, invades and raids."
        }
    }
    var capital: Double {
        switch self {
        case .easy: return 8_000_000
        case .standard: return 5_000_000
        case .hard: return 3_000_000
        }
    }
}

private enum StartEconomy: String, CaseIterable, WizardOption {
    case historic, calm, boom, downturn
    var id: String { rawValue }
    var title: String {
        switch self {
        case .historic: return "As it was"
        case .calm: return "Calm"
        case .boom: return "Boom"
        case .downturn: return "Downturn"
        }
    }
    var note: String {
        switch self {
        case .historic: return "A late boom on stretched credit, rates at 6 percent: a bust is likely within a few years."
        case .calm: return "On trend, inflation at target, little debt built up."
        case .boom: return "Running hot with credit piling up: strong demand now, a hard landing later."
        case .downturn: return "Already in recession with cheap money: weak demand now, a recovery ahead."
        }
    }
}

private enum StartPricePath: String, CaseIterable, WizardOption {
    case historical
    case historicalVariance = "historical_variance"
    case random
    var id: String { rawValue }
    var title: String {
        switch self {
        case .historical: return "Historical"
        case .historicalVariance: return "Historical with variance"
        case .random: return "Random"
        }
    }
    var note: String {
        switch self {
        case .historical: return "Oil, steel, grain and the rest follow the real path of the period, on the real dates."
        case .historicalVariance: return "The same shape as history, with the timing and depth of each swing shifted by your game's seed."
        case .random: return "Neutral prices that drift around where they began. Busts come only from the business cycle."
        }
    }
}

/// Scenario, then the career's terms, then start.
struct NewGameView: View {
    @EnvironmentObject private var store: AppStore
    var onBack: () -> Void

    private let catalogue = ScenarioCatalogue.all
    @State private var chosenID: String?
    @State private var customizing: String?
    @State private var editingCustom: CustomScenarioDto?
    @State private var customs: [CustomScenarioDto] = []
    @State private var chosenCustom: CustomScenarioDto?
    @State private var step = 0
    @State private var mode = StartMode.guided
    @State private var level = StartLevel.standard
    @State private var economy = StartEconomy.historic
    /// Nil means the scenario's own default.
    @State private var pricePathPick: StartPricePath?
    @State private var capital: Double = StartLevel.standard.capital

    private var chosen: ScenarioDto? {
        if let chosenID, let hit = catalogue.first(where: { $0.id == chosenID }) { return hit }
        return catalogue.first { $0.available }
    }

    private var pricePath: StartPricePath {
        pricePathPick ?? StartPricePath(rawValue: chosen?.defaultPricePath ?? "") ?? .historical
    }

    var body: some View {
        VStack(spacing: 0) {
            topBar
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if step == 0 { scenarioStep } else { setupStep }
                }
                .padding(16)
                .frame(maxWidth: 1000)
                .frame(maxWidth: .infinity)
            }
            bottomBar
        }
        .background(Theme.ink)
        .onAppear { customs = customScenarioList(dir: ScenarioEditorView.directory) }
        .sheet(
            isPresented: Binding(
                get: { customizing != nil || editingCustom != nil },
                set: { if !$0 { customizing = nil; editingCustom = nil } }
            ),
            onDismiss: { customs = customScenarioList(dir: ScenarioEditorView.directory) }
        ) {
            ScenarioEditorView(baseId: customizing, existing: editingCustom).environmentObject(store)
        }
    }

    private var topBar: some View {
        HStack(spacing: 10) {
            Button {
                if step == 0 { onBack() } else { step = 0 }
            } label: {
                PixelLabel("Back", icon: "chevron_left").font(.callout)
            }
            .tint(Theme.secondary)
            Text("New game")
                .font(.title3.weight(.bold))
                .foregroundStyle(Theme.primaryText)
                .accessibilityAddTraits(.isHeader)
            Spacer()
            stepDot(1, "Scenario", step == 0)
            stepDot(2, "Setup", step == 1)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Theme.panel)
    }

    private func stepDot(_ number: Int, _ label: String, _ active: Bool) -> some View {
        HStack(spacing: 6) {
            Text("\(number)")
                .font(.caption2.weight(.bold))
                .foregroundStyle(active ? Theme.ink : Theme.secondary)
                .frame(width: 22, height: 22)
                .background(active ? Theme.gold : Theme.panelRaised, in: Pixel.small)
            Text(label)
                .font(.footnote)
                .foregroundStyle(active ? Theme.primaryText : Theme.secondary)
        }
    }

    // MARK: Step one

    @ViewBuilder
    private var scenarioStep: some View {
        Text("Choose a scenario")
            .font(.title2.weight(.bold))
            .foregroundStyle(Theme.primaryText)
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 320), spacing: 16, alignment: .top)], spacing: 16) {
            ForEach(catalogue, id: \.id) { scenario in
                ZStack(alignment: .topLeading) {
                    Button {
                        if scenario.available {
                            chosenID = scenario.id
                            pricePathPick = nil
                            chosenCustom = nil
                        }
                    } label: {
                        ScenarioTile(scenario: scenario, selected: chosenCustom == nil && chosen?.id == scenario.id)
                    }
                    .buttonStyle(.plain)
                    .disabled(!scenario.available)
                    if scenario.available {
                        Button {
                            customizing = scenario.id
                        } label: {
                            PixelLabel("Customize", icon: "sliders")
                                .font(.caption2.weight(.bold))
                                .foregroundStyle(Theme.primaryText)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 5)
                                .background(Theme.panelRaised, in: Pixel.small)
                        }
                        .buttonStyle(.plain)
                        .padding(12)
                    }
                }
            }
        }
        if !customs.isEmpty {
            Text("Your scenarios")
                .font(.title2.weight(.bold))
                .foregroundStyle(Theme.primaryText)
                .padding(.top, 8)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 320), spacing: 16, alignment: .top)], spacing: 16) {
                ForEach(customs, id: \.id) { custom in
                    let on = chosenCustom?.id == custom.id
                    ZStack(alignment: .topTrailing) {
                        Button {
                            chosenCustom = custom
                        } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(custom.name).font(.headline).foregroundStyle(Theme.primaryText)
                                Text("Custom · based on \(catalogue.first { $0.id == custom.baseScenarioId }?.name ?? custom.baseScenarioId) · \(custom.difficulty)")
                                    .font(.caption)
                                    .foregroundStyle(Theme.secondary)
                            }
                            .padding(14)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(on ? Theme.gold.opacity(0.14) : Theme.panel, in: Pixel.shape)
                            .overlay(
                                Pixel.shape
                                    .stroke(on ? Theme.gold : Theme.stroke, lineWidth: on ? 2 : 1)
                            )
                        }
                        .buttonStyle(.plain)
                        Button {
                            editingCustom = custom
                        } label: {
                            PixelLabel("Edit", icon: "pen")
                                .font(.caption2.weight(.bold))
                                .foregroundStyle(Theme.primaryText)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 5)
                                .background(Theme.panelRaised, in: Pixel.small)
                        }
                        .buttonStyle(.plain)
                        .padding(10)
                    }
                }
            }
        }
    }

    // MARK: Step two

    @ViewBuilder
    private var setupStep: some View {
        if let custom = chosenCustom {
            VStack(alignment: .leading, spacing: 4) {
                Text(custom.name).font(.title3.weight(.bold)).foregroundStyle(Theme.primaryText)
                Text("Difficulty, economy, capital, rivals and laws come from this scenario. Open the editor to change them.")
                    .font(.footnote)
                    .foregroundStyle(Theme.secondary)
            }
            section("How you start") {
                OptionGrid(items: StartMode.allCases, selected: mode) { mode = $0 }
            }
        } else if let scenario = chosen {
            HStack(spacing: 14) {
                ScenarioArt(name: scenario.image)
                    .frame(width: 150, height: 150 * 640 / 1200)
                    .clipShape(Pixel.shape)
                VStack(alignment: .leading, spacing: 2) {
                    Text(scenario.name).font(.title3.weight(.bold)).foregroundStyle(Theme.primaryText)
                    Text("\(scenario.region) · starts \(String(scenario.startYear)) · \(scenario.currency)")
                        .font(.footnote)
                        .foregroundStyle(Theme.secondary)
                }
            }
        }
        section("How you start") {
            OptionGrid(items: StartMode.allCases, selected: mode) { mode = $0 }
        }
        if chosenCustom == nil {
        section("Difficulty") {
            OptionGrid(items: StartLevel.allCases, selected: level) {
                level = $0
                capital = $0.capital
            }
        }
        section("The economy on day one") {
            OptionGrid(items: StartEconomy.allCases, selected: economy) { economy = $0 }
        }
        section("World commodity prices") {
            OptionGrid(items: StartPricePath.allCases, selected: pricePath) { pricePathPick = $0 }
            if let fallback = StartPricePath(rawValue: chosen?.defaultPricePath ?? "") {
                Text("This scenario's default is \(fallback.title.lowercased()).")
                    .font(.caption2)
                    .foregroundStyle(Theme.secondary)
            }
        }
        section("Starting capital") {
            ValueSlider(
                value: $capital,
                range: 1_000_000...20_000_000, step: 250_000, ticks: 19,
                label: { Format.moneyCompact(Int64($0 * 100)) },
                detail: { _ in
                    mode.guided
                        ? "Your own cash. Taking over the company uses some of it; the rest is yours to invest."
                        : "Your own cash to found and fund companies."
                }
            )
            .frame(maxWidth: 760)
        }
        }
    }

    private func section<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Eyebrow(text: title)
            content()
        }
    }

    // MARK: Bottom bar

    private var bottomBar: some View {
        HStack(spacing: 12) {
            Text(summary)
                .font(.footnote)
                .foregroundStyle(Theme.secondary)
                .lineLimit(2)
            Spacer(minLength: 8)
            if step == 0 {
                primaryButton("Next: setup") { step = 1 }
                    .disabled(chosen == nil && chosenCustom == nil)
            } else {
                primaryButton("Start career") { start() }
                    .disabled((chosen == nil && chosenCustom == nil) || store.isBusy)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Theme.panel)
    }

    private var summary: String {
        if let custom = chosenCustom {
            return step == 0 ? "\(custom.name): your scenario" : "\(mode.title) · \(custom.name)"
        }
        if step == 0 {
            if let scenario = chosen { return "\(scenario.name): \(scenario.tagline)" }
            return "Pick a scenario"
        }
        return "\(mode.title) · \(level.title) · \(economy.title) economy · \(Format.moneyCompact(Int64(capital * 100)))"
    }

    private func primaryButton(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).font(.callout.weight(.bold)).padding(.horizontal, 10)
        }
        .buttonStyle(PixelButtonStyle(prominent: true))
        .tint(Theme.gold)
        .foregroundStyle(Theme.ink)
        .controlSize(.large)
    }

    private func start() {
        if let custom = chosenCustom {
            let seed = UInt64.random(in: 1...9_999_999)
            Task { await store.startCustom(custom, guided: mode.guided, seed: seed, fiveYear: mode.fiveYear) }
            return
        }
        guard let scenario = chosen else { return }
        let options = NewGameDto(
            seed: UInt64.random(in: 1...9_999_999),
            scenarioId: scenario.id,
            guided: mode.guided,
            fiveYear: mode.fiveYear,
            difficulty: level.rawValue,
            economy: economy.rawValue,
            startingCapitalCents: Int64((capital * 100).rounded()),
            pricePath: pricePathPick?.rawValue ?? ""
        )
        Task { await store.startCareer(options) }
    }
}

private struct OptionGrid<T: WizardOption>: View {
    let items: [T]
    let selected: T
    let onPick: (T) -> Void

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 240), spacing: 10, alignment: .top)], spacing: 10) {
            ForEach(items) { item in
                let on = item == selected
                Button {
                    onPick(item)
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(item.title)
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(Theme.primaryText)
                        Text(item.note)
                            .font(.caption)
                            .foregroundStyle(Theme.secondary)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(14)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(on ? Theme.gold.opacity(0.14) : Theme.panel, in: Pixel.shape)
                    .overlay(Pixel.shape.stroke(on ? Theme.gold : Theme.stroke, lineWidth: on ? 2 : 1))
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
    }
}

/// One scenario: its art, name, region, year and currency, or a coming-soon card.
struct ScenarioTile: View {
    let scenario: ScenarioDto
    let selected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ZStack(alignment: .bottomLeading) {
                Color.clear
                    .aspectRatio(1200.0 / 640.0, contentMode: .fit)
                    .overlay(
                        ScenarioArt(name: scenario.image)
                            .opacity(scenario.available ? 1 : 0.45)
                    )
                    .clipped()
                LinearGradient(
                    colors: [Color.clear, Theme.ink.opacity(0.9)],
                    startPoint: UnitPoint(x: 0.5, y: 0.55), endPoint: .bottom
                )
                VStack(alignment: .leading, spacing: 2) {
                    Text(scenario.name)
                        .font(.title3.weight(.bold))
                        .foregroundStyle(Theme.primaryText)
                    Text("\(scenario.region) · \(String(scenario.startYear))")
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                }
                .padding(14)
            }
            .overlay(alignment: .topTrailing) {
                Text(scenario.available ? "Play" : "Coming soon")
                    .font(.caption2.weight(.bold))
                    .foregroundStyle(scenario.available ? Theme.ink : Theme.primaryText)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 4)
                    .background(scenario.available ? Theme.gold : Theme.panelRaised, in: Pixel.small)
                    .padding(12)
            }
            VStack(alignment: .leading, spacing: 6) {
                Text(scenario.tagline)
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Theme.primaryText)
                Text(scenario.summary)
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)
                Text(scenario.country + " · " + scenario.currency + (scenario.available ? " · \(scenario.regionCount) regions" : ""))
                    .font(.caption2)
                    .foregroundStyle(Theme.secondary.opacity(0.8))
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Theme.panel)
        .clipShape(Pixel.shape)
        .overlay(Pixel.shape.stroke(selected ? Theme.gold : Theme.stroke, lineWidth: selected ? 3 : 1))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? [.isSelected, .isButton] : .isButton)
    }
}
