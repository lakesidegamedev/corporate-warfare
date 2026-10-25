import SwiftUI

struct StartView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    @AppStorage(OnboardingKeys.completed) private var onboardingCompleted = false
    @State private var guided = true
    @State private var fiveYear = true
    @State private var seedText = ""
    @State private var showingAbout = false
    @State private var showingSaves = false
    @State private var export: DiagnosticExport?

    private var info: EngineInfo { engineInfo() }
    private var walkthroughTitle: String {
        onboardingCompleted ? "Replay the guided walkthrough" : "Guided walkthrough"
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Corporate Warfare")
                            .font(.largeTitle.weight(.semibold))
                            .foregroundStyle(Theme.navy)
                        Text("A Great Lakes business simulation. Offline, single player, one investor across every company you own or lose.")
                            .font(.body)
                            .foregroundStyle(Theme.secondary)
                    }

                    SectionCard(title: "New career") {
                        Toggle("Guided start (60% of a running firm)", isOn: $guided)
                        Toggle("Five-year scored scenario", isOn: $fiveYear)
                        TextField("Optional seed", text: $seedText)
                            .textFieldStyle(.roundedBorder)
                            .keyboardType(.numberPad)
                        Button {
                            let seed = UInt64(seedText) ?? UInt64.random(in: 1...9_999_999)
                            Task { await store.newCareer(guided: guided, fiveYear: fiveYear, seed: seed) }
                        } label: {
                            Label("Start", systemImage: "play.fill")
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.large)
                        .disabled(store.isBusy)
                    }

                    if let issue = store.recoveryIssue {
                        RecoveryCard(issue: issue) {
                            export = store.makeDiagnostics()
                        }
                        .environmentObject(store)
                    }

                    if let latest = store.saves.first {
                        SectionCard(title: "Continue") {
                            Button {
                                Task { await store.resume(latest) }
                            } label: {
                                Label("Resume week \(latest.week)", systemImage: "arrow.clockwise")
                                    .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.large)
                        }
                    }

                    SectionCard(title: "About this build") {
                        Text("Engine \(info.version) · interface \(info.interfaceVersion) · rules \(info.rulesVersion)")
                            .font(.caption.monospaced())
                            .foregroundStyle(Theme.secondary)
                        Text("Premium offline game. No account, no ads, no network calls. Institutions are simplified game rules, not legal, tax or investment advice.")
                            .font(.footnote)
                            .foregroundStyle(Theme.secondary)
                        Button(walkthroughTitle) {
                            store.showOnboarding = true
                        }
                        .accessibilityHint("Explains the first seven steps and the real action behind each one.")
                        Button("Saves and recovery") { showingSaves = true }
                    }

                    if let error = store.errorMessage {
                        Text(error)
                            .font(.footnote)
                            .foregroundStyle(Theme.negative)
                    }
                }
                .padding(20)
                .sheet(item: $export) { item in
                    DiagnosticExportView(export: item)
                }
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Start")
            .sheet(isPresented: $showingSaves) {
                SavesView()
                    .environmentObject(store)
            }
        }
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
                    Label("Recover last good save", systemImage: "clock.arrow.circlepath")
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
                        Label("Share diagnostics file", systemImage: "square.and.arrow.up")
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
                    Button {
                        Task {
                            await store.resume(save)
                            if store.recoveryIssue == nil {
                                dismiss()
                            }
                        }
                    } label: {
                        VStack(alignment: .leading) {
                            HStack {
                                Text(save.slot).font(.body)
                                Spacer()
                                if save.hasLastGood {
                                    Text("last good")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            Text("Week \(save.week) · \(save.savedAt.formatted(date: .abbreviated, time: .shortened))")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .contextMenu {
                        if save.hasLastGood {
                            Button {
                                Task { await store.recover(slot: save.slot) }
                            } label: {
                                Label("Recover last good copy", systemImage: "clock.arrow.circlepath")
                            }
                        }
                        Button(role: .destructive) {
                            SaveManager.shared.delete(save)
                            store.saves = SaveManager.shared.list()
                        } label: {
                            Label("Delete this slot", systemImage: "trash")
                        }
                    }
                }
                .onDelete { offsets in
                    for index in offsets {
                        SaveManager.shared.delete(store.saves[index])
                    }
                    store.saves = SaveManager.shared.list()
                }
            }
            .navigationTitle("Saves")
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
