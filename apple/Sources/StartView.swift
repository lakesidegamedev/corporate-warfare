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

struct SavesView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                if store.saves.isEmpty {
                    Text("No saves yet. A career autosaves every four weeks and when you stop.")
                        .foregroundStyle(.secondary)
                }
                ForEach(store.saves) { save in
                    Button {
                        Task {
                            await store.resume(save)
                            dismiss()
                        }
                    } label: {
                        VStack(alignment: .leading) {
                            Text(save.slot).font(.body)
                            Text("Week \(save.week) · \(save.savedAt.formatted(date: .abbreviated, time: .shortened))")
                                .font(.caption)
                                .foregroundStyle(.secondary)
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
            }
        }
    }
}
