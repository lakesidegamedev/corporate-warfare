import SwiftUI

/// Ways to play a scenario.
enum GameMode: String, CaseIterable, Identifiable {
    case guided, sandbox, scored
    var id: String { rawValue }
    var title: String {
        switch self {
        case .guided: return "Guided"
        case .sandbox: return "Sandbox"
        case .scored: return "Five-year"
        }
    }
    var note: String {
        switch self {
        case .guided: return "Take over 60% of a running company, with the walkthrough."
        case .sandbox: return "Start with cash and found your own company. No end date."
        case .scored: return "Guided start, scored at the end of year five."
        }
    }
    var guided: Bool { self != .sandbox }
    var fiveYear: Bool { self == .scored }
}

/// One scenario on the start screen: where and when it starts, and what the player is up against.
struct ScenarioCard: View {
    let scenario: ScenarioDto
    let selected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(scenario.name)
                    .font(.headline)
                    .foregroundStyle(Theme.primaryText)
                Spacer()
                if selected {
                    PixelIcon("check_circle")
                        .foregroundStyle(Theme.gold)
                }
            }
            Text("\(scenario.country) · starts \(String(scenario.startYear)) · \(scenario.regionCount) regions")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Theme.secondary)
            Text(scenario.summary)
                .font(.footnote)
                .foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.ink.opacity(0.5), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(selected ? Theme.gold : Theme.stroke, lineWidth: selected ? 2 : 1)
        )
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? [.isSelected, .isButton] : .isButton)
    }
}

/// Names the slot that failed, its week and timestamp, and offers the last-good copy. The damaged
/// file stays on the device: recovery never overwrites or deletes it.
struct RecoveryCard: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    let issue: SaveRecoveryIssue
    var onExportDiagnostics: () -> Void

    private var weekText: String {
        issue.week.map { "week \($0)" } ?? "unknown week"
    }

    private var dateText: String {
        guard let savedAt = issue.savedAt, savedAt != Date.distantPast else { return "unknown time" }
        return savedAt.formatted(date: .abbreviated, time: .shortened)
    }

    var body: some View {
        SectionCard(title: "Save needs recovery") {
            Text("\(issue.slot) · \(weekText) · saved \(dateText)")
                .font(.callout.weight(.medium))
            Text(SaveCopy.message(forKey: issue.errorKey))
                .font(.footnote)
                .foregroundStyle(Theme.secondaryText(for: scheme))
            if issue.hasLastGood {
                Button {
                    Task { await store.recoverLastGood() }
                } label: {
                    PixelLabel("Recover last good save", icon: "history")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .accessibilityHint("Loads the previous copy of this save after the engine checks it.")
            } else {
                Text("There is no last good copy of this slot.")
                    .font(.footnote)
                    .foregroundStyle(Theme.negative)
            }
            Button("Export diagnostics", action: onExportDiagnostics)
                .font(.footnote)
            Text("A copy of the damaged file is kept for support. Nothing is deleted or uploaded.")
                .font(.caption2)
                .foregroundStyle(Theme.secondaryText(for: scheme))
        }
    }
}

/// Local diagnostics review. The file is written on device and shared only by explicit action.
struct DiagnosticExportView: View {
    let export: DiagnosticExport
    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text("Review before sharing")
                        .font(.headline)
                    Text(export.summary)
                        .font(.caption.monospaced())
                        .foregroundStyle(Theme.secondaryText(for: scheme))
                        .textSelection(.enabled)
                    ShareLink(item: export.url) {
                        PixelLabel("Share diagnostics file", icon: "share")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    Text("This file is written on this device. Corporate Warfare never uploads it; sharing happens only through the button above.")
                        .font(.footnote)
                        .foregroundStyle(Theme.secondaryText(for: scheme))
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(20)
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Diagnostics")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
    }
}

struct SavesView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @State private var export: DiagnosticExport?
    @State private var confirmDelete: String?

    var body: some View {
        NavigationStack {
            List {
                if let issue = store.recoveryIssue {
                    RecoveryCard(issue: issue) {
                        export = store.makeDiagnostics()
                    }
                    .environmentObject(store)
                    .listRowInsets(EdgeInsets())
                    .listRowBackground(Color.clear)
                }
                if store.saves.isEmpty {
                    Text("No saves yet. A career autosaves every four weeks and when you stop.")
                        .foregroundStyle(.secondary)
                }
                ForEach(store.saves) { save in
                    SaveRow(
                        save: save,
                        confirming: confirmDelete == save.id,
                        onLoad: {
                            Task {
                                await store.resume(save)
                                if store.recoveryIssue == nil { dismiss() }
                            }
                        },
                        onAskDelete: { confirmDelete = save.id },
                        onCancelDelete: { confirmDelete = nil },
                        onDelete: {
                            SaveManager.shared.delete(save)
                            store.saves = SaveManager.shared.list()
                            confirmDelete = nil
                        },
                        onRecover: save.hasLastGood ? { Task { await store.recover(slot: save.slot) } } : nil
                    )
                    .listRowBackground(Theme.panel)
                }
            }
            .scrollContentBackground(.hidden)
            .background(Theme.ink)
            .navigationTitle("Load game")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button("Diagnostics") { export = store.makeDiagnostics() }
                }
            }
        }
        .sheet(item: $export) { item in
            DiagnosticExportView(export: item)
        }
    }
}

/// One save: what it is, when it was written, and Load or Delete with a confirmation.
struct SaveRow: View {
    let save: StoredSave
    let confirming: Bool
    var onLoad: () -> Void
    var onAskDelete: () -> Void
    var onCancelDelete: () -> Void
    var onDelete: () -> Void
    var onRecover: (() -> Void)?

    private var isAutosave: Bool { save.slot.hasPrefix("autosave") }

    var body: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(isAutosave ? "Autosave" : "Saved game")
                    .font(.body.weight(.semibold))
                    .foregroundStyle(Theme.primaryText)
                Text("\(ResultBuckets.weekLabel(save.week)) · week \(save.week) · \(save.savedAt.formatted(date: .abbreviated, time: .shortened))")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                if save.hasLastGood {
                    Text("A last good copy is kept")
                        .font(.caption2)
                        .foregroundStyle(Theme.secondary)
                }
            }
            Spacer(minLength: 4)
            if confirming {
                Button("Delete", role: .destructive, action: onDelete)
                    .buttonStyle(.borderless)
                Button("Keep", action: onCancelDelete)
                    .buttonStyle(.borderless)
                    .tint(Theme.secondary)
            } else {
                Button("Delete", action: onAskDelete)
                    .buttonStyle(.borderless)
                    .tint(Theme.secondary)
                Button("Load", action: onLoad)
                    .buttonStyle(.borderedProminent)
                    .tint(Theme.gold)
                    .foregroundStyle(Theme.ink)
            }
        }
        .contextMenu {
            if let onRecover {
                Button {
                    onRecover()
                } label: {
                    PixelLabel("Recover last good copy", icon: "history")
                }
            }
        }
    }
}
