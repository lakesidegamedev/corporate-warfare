import SwiftUI

/// Edit a built-in scenario's numbers and keep the result as a custom scenario. Everything is
/// optional: a scenario left alone plays exactly like the built-in one.
struct ScenarioEditorView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var scheme
    @Environment(\.horizontalSizeClass) private var sizeClass

    @State private var draft: CustomScenarioDto?
    @State private var saved: [CustomScenarioDto] = []
    @State private var message: String?
    @State private var guided = true
    @State private var confirmDelete = false
    @State private var openRival: String?
    @State private var openRegion: String?

    /// Open straight into a built-in scenario, as from its Customize button.
    var baseId: String?

    private let ranges = customScenarioRanges()
    private let builtIns = scenarios().filter { $0.available }

    private static let difficulties = [("easy", "Easy"), ("standard", "Standard"), ("hard", "Hard")]
    private static let economies = [("historic", "Historic"), ("calm", "Calm"), ("boom", "Boom"), ("downturn", "Downturn")]
    private static let unions = [("weak", "Weak"), ("standard", "Standard"), ("strong", "Strong")]
    private static let paths = [("historical", "Historical"), ("historical_variance", "Historical with variance"), ("random", "Random")]

    private var directory: String {
        SaveManager.shared.directory.appendingPathComponent("custom-scenarios", isDirectory: true).path
    }

    var body: some View {
        NavigationStack {
            Group {
                if let d = draft {
                    editor(d)
                } else {
                    picker
                }
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Scenario editor")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(draft == nil ? "Close" : "Back") {
                        if draft == nil { dismiss() } else { draft = nil; message = nil }
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    if let d = draft {
                        Button("Save") { save(d, asNew: false) }
                            .disabled(!problems(d).isEmpty)
                    }
                }
            }
            .onAppear {
                saved = customScenarioList(dir: directory)
                if draft == nil, let baseId, let scenario = builtIns.first(where: { $0.id == baseId }) {
                    open(baseId, name: "My \(scenario.name)")
                }
            }
            .confirmationDialog("Delete this scenario?", isPresented: $confirmDelete, titleVisibility: .visible) {
                Button("Delete", role: .destructive) { deleteDraft() }
                Button("Keep it", role: .cancel) {}
            } message: {
                Text("The saved scenario is removed. Games already started from it are not affected.")
            }
        }
    }

    // MARK: Picker

    private var picker: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                SectionCard(title: "Start from a built-in scenario") {
                    ForEach(builtIns, id: \.id) { scenario in
                        Button {
                            open(scenario.id, name: "My \(scenario.name)")
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(scenario.name).font(.headline)
                                Text("\(scenario.region) · \(String(scenario.startYear))")
                                    .font(.caption)
                                    .foregroundStyle(Theme.secondary)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .buttonStyle(.plain)
                    }
                }
                if !saved.isEmpty {
                    SectionCard(title: "Your scenarios") {
                        ForEach(saved, id: \.id) { scenario in
                            Button {
                                draft = scenario
                            } label: {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(scenario.name).font(.headline)
                                    Text("Custom · \(scenario.difficulty)")
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondary)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                if let message {
                    Text(message).font(.footnote).foregroundStyle(Theme.negative)
                }
            }
            .padding(16)
        }
    }

    // MARK: Editor

    private func editor(_ d: CustomScenarioDto) -> some View {
        let summary = try? customScenarioSummary(scenario: d)
        let issues = summary?.problems ?? []
        return VStack(spacing: 0) {
            ScrollView {
                Group {
                    if sizeClass == .regular {
                        HStack(alignment: .top, spacing: 16) {
                            VStack(spacing: 12) { leftColumn(d, summary) }
                            VStack(spacing: 12) { rightColumn(d) }
                        }
                    } else {
                        VStack(spacing: 12) {
                            leftColumn(d, summary)
                            rightColumn(d)
                        }
                    }
                }
                .padding(16)
            }
            footer(d, summary, issues)
        }
    }

    @ViewBuilder
    private func leftColumn(_ d: CustomScenarioDto, _ summary: CustomSummaryDto?) -> some View {
        SectionCard(title: "Name") {
            TextField("Scenario name", text: field(d, \.name))
                .textFieldStyle(.roundedBorder)
            Text("Based on \(builtIns.first { $0.id == d.baseScenarioId }?.name ?? d.baseScenarioId)")
                .font(.caption)
                .foregroundStyle(Theme.secondary)
        }
        SectionCard(title: "The start") {
            Text("Difficulty").font(.caption).foregroundStyle(Theme.secondary)
            segmented(Self.difficulties, field(d, \.difficulty))
            Text("The economy on day one").font(.caption).foregroundStyle(Theme.secondary)
            segmented(Self.economies, field(d, \.economy))
            Text("World price path").font(.caption).foregroundStyle(Theme.secondary)
            Picker("World price path", selection: field(d, \.pricePath)) {
                ForEach(Self.paths, id: \.0) { Text($0.1).tag($0.0) }
            }
            .pickerStyle(.menu)
            Text("The price path is saved now and takes effect in a later preview.")
                .font(.caption2)
                .foregroundStyle(Theme.secondary)
        }
        SectionCard(title: "Starting capital") {
            let standard = summary?.startingCapitalCents ?? 0
            Picker("Starting capital", selection: Binding(
                get: { d.startingCapitalCents == nil },
                set: { standardOn in
                    update { $0.startingCapitalCents = standardOn ? nil : min(max(standard, ranges.capitalMinCents), ranges.capitalMaxCents) }
                }
            )) {
                Text("Standard for difficulty").tag(true)
                Text("Set an amount").tag(false)
            }
            .pickerStyle(.segmented)
            if let cents = d.startingCapitalCents {
                let top = min(Double(ranges.capitalMaxCents) / 100, 50_000_000)
                ValueSlider(
                    value: Binding(
                        get: { min(Double(draft?.startingCapitalCents ?? cents) / 100, top) },
                        set: { value in update { $0.startingCapitalCents = Int64(value * 100) } }
                    ),
                    range: (Double(ranges.capitalMinCents) / 100)...top,
                    step: 250_000,
                    ticks: 20,
                    label: { Format.moneyCompact(Int64($0 * 100)) }
                )
            } else {
                Text(Format.moneyCompact(standard))
                    .font(.title3.weight(.bold).monospacedDigit())
            }
        }
        if let laws = d.laws {
            SectionCard(title: "Laws in force at the start") {
                Text("Corporate tax").font(.caption).foregroundStyle(Theme.secondary)
                ValueSlider(
                    value: lawBinding(laws, get: { Double($0.corporateTaxBps) }, set: { $0.corporateTaxBps = Int32($1) }),
                    range: Double(ranges.taxMinBps)...Double(ranges.taxMaxBps), step: 100, ticks: 30,
                    label: { String(format: "%.0f%%", $0 / 100) }
                )
                Text("Wage floor, a week").font(.caption).foregroundStyle(Theme.secondary)
                ValueSlider(
                    value: lawBinding(laws, get: { Double($0.wageFloorCents) }, set: { $0.wageFloorCents = Int64($1) }),
                    range: Double(ranges.wageFloorMinCents)...Double(ranges.wageFloorMaxCents), step: 1_000, ticks: 20,
                    label: { Format.money(Int64($0)) }
                )
                Text("Merger review starts at this combined regional share").font(.caption).foregroundStyle(Theme.secondary)
                ValueSlider(
                    value: lawBinding(laws, get: { Double($0.mergerReviewBps) }, set: { $0.mergerReviewBps = Int64($1) }),
                    range: Double(ranges.mergerMinBps)...Double(ranges.mergerMaxBps), step: 100, ticks: 18,
                    label: { String(format: "%.0f%%", $0 / 100) }
                )
                Text("Unions").font(.caption).foregroundStyle(Theme.secondary)
                Picker("Unions", selection: Binding(
                    get: { draft?.laws?.unions ?? laws.unions },
                    set: { value in update { $0.laws?.unions = value } }
                )) {
                    ForEach(Self.unions, id: \.0) { Text($0.1).tag($0.0) }
                }
                .pickerStyle(.segmented)
            }
        }
    }

    @ViewBuilder
    private func rightColumn(_ d: CustomScenarioDto) -> some View {
        SectionCard(title: "Rivals (\(d.rivals.filter { $0.enabled }.count) of \(d.rivals.count) start)") {
            ForEach(d.rivals, id: \.firmId) { rival in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(rival.name)
                                .font(.subheadline.weight(.semibold))
                                .foregroundStyle(rival.enabled ? Theme.primaryText : Theme.secondary)
                            Text(rivalNote(rival))
                                .font(.caption2)
                                .foregroundStyle(Theme.secondary)
                        }
                        .contentShape(Rectangle())
                        .onTapGesture { openRival = openRival == rival.firmId ? nil : rival.firmId }
                        Spacer()
                        Button(rival.enabled ? "Remove" : "Add back") {
                            updateRival(rival.firmId) { $0.enabled.toggle() }
                        }
                        .font(.caption.weight(.semibold))
                    }
                    if openRival == rival.firmId && rival.enabled {
                        Text("Size").font(.caption).foregroundStyle(Theme.secondary)
                        ValueSlider(
                            value: Binding(
                                get: { Double(currentRival(rival).sizePct) },
                                set: { value in updateRival(rival.firmId) { $0.sizePct = UInt16(value) } }
                            ),
                            range: Double(ranges.rivalSizeMin)...Double(ranges.rivalSizeMax), step: 5, origin: 100, ticks: 22,
                            label: { "\(Int($0))% of standard" }
                        )
                        Text("Opening cash").font(.caption).foregroundStyle(Theme.secondary)
                        ValueSlider(
                            value: Binding(
                                get: { Double(currentRival(rival).cashPct) },
                                set: { value in updateRival(rival.firmId) { $0.cashPct = UInt16(value) } }
                            ),
                            range: Double(ranges.rivalCashMin)...Double(ranges.rivalCashMax), step: 5, origin: 100, ticks: 25,
                            label: { "\(Int($0))% of standard" }
                        )
                    }
                }
            }
        }
        SectionCard(title: "Regions") {
            ForEach(d.regions, id: \.code) { region in
                VStack(alignment: .leading, spacing: 6) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(region.name).font(.subheadline.weight(.semibold))
                        Text("Wages \(region.wagePct)% · demand \(region.demandPct)%")
                            .font(.caption2)
                            .foregroundStyle(Theme.secondary)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
                    .onTapGesture { openRegion = openRegion == region.code ? nil : region.code }
                    if openRegion == region.code {
                        Text("Wages").font(.caption).foregroundStyle(Theme.secondary)
                        ValueSlider(
                            value: Binding(
                                get: { Double(currentRegion(region).wagePct) },
                                set: { value in updateRegion(region.code) { $0.wagePct = UInt16(value) } }
                            ),
                            range: Double(ranges.regionPctMin)...Double(ranges.regionPctMax), step: 5, origin: 100, ticks: 30,
                            label: { "\(Int($0))% of standard" }
                        )
                        Text("Household demand").font(.caption).foregroundStyle(Theme.secondary)
                        ValueSlider(
                            value: Binding(
                                get: { Double(currentRegion(region).demandPct) },
                                set: { value in updateRegion(region.code) { $0.demandPct = UInt16(value) } }
                            ),
                            range: Double(ranges.regionPctMin)...Double(ranges.regionPctMax), step: 5, origin: 100, ticks: 30,
                            label: { "\(Int($0))% of standard" }
                        )
                    }
                }
            }
        }
    }

    private func footer(_ d: CustomScenarioDto, _ summary: CustomSummaryDto?, _ issues: [String]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            if let summary {
                Text("\(d.name.isEmpty ? "Unnamed" : d.name) · \(d.difficulty) · \(d.economy) economy · \(Format.moneyCompact(summary.startingCapitalCents)) · \(summary.rivalsStarted) of \(summary.rivalsTotal) rivals · wages \(summary.averageWagePct)% · demand \(summary.averageDemandPct)%")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
            }
            if let first = issues.first {
                Text(first).font(.caption).foregroundStyle(Theme.negative)
            } else if let message {
                Text(message).font(.caption).foregroundStyle(Theme.positive)
            }
            Picker("How you start", selection: $guided) {
                Text("Run a company").tag(true)
                Text("Found your own").tag(false)
            }
            .pickerStyle(.segmented)
            HStack {
                if !d.id.isEmpty {
                    Button("Delete", role: .destructive) { confirmDelete = true }
                    Button("Save as") { save(d, asNew: true) }
                        .disabled(!issues.isEmpty)
                }
                Spacer()
                Button {
                    let seed = UInt64.random(in: 1...9_999_999)
                    Task {
                        await store.startCustom(d, guided: guided, seed: seed)
                        if store.errorMessage == nil { dismiss() }
                    }
                } label: {
                    PixelLabel("Start game", icon: "play")
                }
                .buttonStyle(.borderedProminent)
                .disabled(!issues.isEmpty || store.isBusy)
            }
        }
        .padding(16)
        .background(Theme.panel)
    }

    // MARK: Editing helpers

    private func problems(_ d: CustomScenarioDto) -> [String] {
        (try? customScenarioSummary(scenario: d))?.problems ?? []
    }

    private func update(_ change: (inout CustomScenarioDto) -> Void) {
        guard var current = draft else { return }
        change(&current)
        draft = current
    }

    private func field<T>(_ d: CustomScenarioDto, _ path: WritableKeyPath<CustomScenarioDto, T>) -> Binding<T> {
        Binding(
            get: { (draft ?? d)[keyPath: path] },
            set: { value in update { $0[keyPath: path] = value } }
        )
    }

    private func lawBinding(
        _ laws: CustomLawsDto,
        get: @escaping (CustomLawsDto) -> Double,
        set: @escaping (inout CustomLawsDto, Double) -> Void
    ) -> Binding<Double> {
        Binding(
            get: { get(draft?.laws ?? laws) },
            set: { value in update { if var next = $0.laws { set(&next, value); $0.laws = next } } }
        )
    }

    private func segmented(_ items: [(String, String)], _ binding: Binding<String>) -> some View {
        Picker("", selection: binding) {
            ForEach(items, id: \.0) { Text($0.1).tag($0.0) }
        }
        .pickerStyle(.segmented)
    }

    private func currentRival(_ rival: RivalDto) -> RivalDto {
        draft?.rivals.first { $0.firmId == rival.firmId } ?? rival
    }

    private func currentRegion(_ region: RegionAdjustDto) -> RegionAdjustDto {
        draft?.regions.first { $0.code == region.code } ?? region
    }

    private func updateRival(_ id: String, _ change: (inout RivalDto) -> Void) {
        update { scenario in
            if let index = scenario.rivals.firstIndex(where: { $0.firmId == id }) {
                change(&scenario.rivals[index])
            }
        }
    }

    private func updateRegion(_ code: String, _ change: (inout RegionAdjustDto) -> Void) {
        update { scenario in
            if let index = scenario.regions.firstIndex(where: { $0.code == code }) {
                change(&scenario.regions[index])
            }
        }
    }

    private func rivalNote(_ rival: RivalDto) -> String {
        let sector = rival.industry.replacingOccurrences(of: "_", with: " ").capitalized
        return rival.enabled ? "\(sector) · size \(rival.sizePct)% · cash \(rival.cashPct)%" : "\(sector) · does not start"
    }

    // MARK: Actions

    private func open(_ base: String, name: String) {
        do {
            draft = try customScenarioDefault(baseScenarioId: base, name: name)
            message = nil
        } catch {
            message = plain(error)
        }
    }

    private func save(_ d: CustomScenarioDto, asNew: Bool) {
        var toSave = d
        if asNew {
            toSave.id = ""
            if !toSave.name.isEmpty { toSave.name += " copy" }
        }
        do {
            let stored = try customScenarioSave(dir: directory, scenario: toSave)
            draft = stored
            saved = customScenarioList(dir: directory)
            message = "Saved as \(stored.name)."
        } catch {
            message = plain(error)
        }
    }

    private func deleteDraft() {
        guard let d = draft else { return }
        do {
            try customScenarioDelete(dir: directory, id: d.id)
            saved = customScenarioList(dir: directory)
            draft = nil
            message = nil
        } catch {
            message = plain(error)
        }
    }

    private func plain(_ error: Error) -> String {
        if let ffi = error as? FfiError, case .Domain(_, let detail) = ffi, !detail.isEmpty {
            return detail
        }
        return error.localizedDescription
    }
}
