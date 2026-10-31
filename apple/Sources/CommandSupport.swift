import Foundation
import SwiftUI

/// Builds the exact externally-tagged serde shape of `CommandIntent` without hand-quoted strings.
/// Swift never invents a command field: every key here matches `crates/cw-sim/src/commands.rs`.
enum CommandJSON {
    static func command(_ variant: String, _ fields: [String: Any] = [:]) -> String {
        // JSONSerialization takes NSNumber; keep every integer field an `Int` so the bridge is
        // unambiguous for UInt32 ids, UInt64 handles and Int64 cent amounts alike.
        let normalized: [String: Any] = fields.mapValues { value in
            switch value {
            case let number as Int32: return Int(number)
            case let number as UInt32: return Int(number)
            case let number as Int64: return Int(number)
            case let number as UInt64: return Int(number)
            default: return value
            }
        }
        guard let data = try? JSONSerialization.data(withJSONObject: [variant: normalized], options: []),
              let text = String(data: data, encoding: .utf8) else {
            return "{}"
        }
        return text
    }

    /// The command a JSON intent carries: the bare string of a unit variant, or the single key of
    /// a struct variant.
    static func name(of json: String) -> String {
        let trimmed = json.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("\"") {
            return trimmed.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
        }
        guard let open = trimmed.range(of: "{\"") else { return "" }
        let rest = trimmed[open.upperBound...]
        guard let close = rest.firstIndex(of: "\"") else { return "" }
        return String(rest[rest.startIndex..<close])
    }

    /// A unit variant is a bare JSON string, for example `AcceptManagementContract`.
    static func unit(_ variant: String) -> String {
        "\"" + variant + "\""
    }
}

/// One typed domain rejection. The key is stable; the copy and remedy live here, not in Rust.
struct CommandRejection: Identifiable, Equatable {
    let key: String
    let detail: String
    var id: String { key + detail }

    var title: String { Remedy.phase(detail)?.title ?? Remedy.title(for: key) }
    var remedy: String { Remedy.phase(detail)?.remedy ?? Remedy.text(for: key) }
    /// One line for inline display: the title and what to do about it.
    var message: String { "\(title). \(remedy)" }
}

enum Remedy {
    /// Wrong-phase rejections that name a specific division or raid rule.
    static func phase(_ detail: String) -> (title: String, remedy: String)? {
        if detail.contains("division_strategy") {
            return ("Your division runs another strategy", "Every plant in a division runs its strategy. Build that strategy here, or retool the division from the CEO Office first.")
        }
        if detail.contains("own_division") {
            return ("No division in this market", "You can only raid a market you serve. Build a plant of this sector type in the region first.")
        }
        if detail.contains("rival_division") {
            return ("Nothing to take", "The rival needs at least two plants here: a raid never takes its last one.")
        }
        if detail.contains("raid_cooldown") {
            return ("Campaign still running", "You can raid the same market once every thirteen weeks.")
        }
        if detail.contains("rival_firm") {
            return ("That is your own group", "Raids target rivals outside the group you control.")
        }
        return nil
    }

    static func title(for key: String) -> String {
        switch key {
        case "error.invalid_actor": return "Unknown actor"
        case "error.no_authority": return "No authority for this action"
        case "error.stale_revision": return "The world moved on"
        case "error.invalid_entity": return "That entity is gone"
        case "error.insufficient_cash": return "Not enough free cash"
        case "error.insufficient_stock": return "Not enough shares or stock"
        case "error.capacity_unavailable": return "Capacity is unavailable"
        case "error.solvency_restriction": return "A solvency rule blocks this"
        case "error.ownership_cycle": return "That would create an ownership cycle"
        case "error.quote_expired": return "The quote expired"
        case "error.legal_review_required": return "Competition review is pending"
        case "error.duplicate_command": return "Already submitted"
        case "error.arithmetic": return "A number is out of range"
        case "error.wrong_phase": return "Not available in this state"
        case "error.malformed_command": return "The form could not build a command"
        case "error.resource_limit": return "The world limit was reached"
        case "error.internal_fault": return "The engine could not complete this"
        default: return "Rejected"
        }
    }

    static func text(for key: String) -> String {
        switch key {
        case "error.no_authority":
            return "You need control of the legal firm acting here: more than half the votes, or yourself as CEO with three board seats. Open Ownership to raise your stake or call a meeting."
        case "error.insufficient_cash":
            return "This spends the firm's own uncommitted cash, not your personal cash. Lower the amount, or fund the firm first with a loan, a share issuance or receipts from sales."
        case "error.insufficient_stock":
            return "You hold fewer shares or lots than this order needs. Reduce the quantity, or buy the shares on the exchange first."
        case "error.solvency_restriction":
            return "A payout, credit or capital rule blocks this. Check the four-week cash buffer, positive distributable earnings, credit grade and payout freeze on the Finance section."
        case "error.capacity_unavailable":
            return "Physical capacity is short. Add workers, inputs or another facility before retrying."
        case "error.ownership_cycle":
            return "This purchase would make a firm own part of itself, directly or through another holding. Choose a target outside your controlled group."
        case "error.quote_expired":
            return "The quote expired at a week boundary. Your input is kept; confirm again to revalidate it against the current week."
        case "error.legal_review_required":
            return "The competition review is still running. Open Deals and wait for the review deadline, or submit a divestment remedy if one is open."
        case "error.wrong_phase":
            return "The deal is in a different state than this action allows. Open Deals and use the action shown for its current state."
        case "error.stale_revision":
            return "The world changed while this form was open. Your input is kept; confirm again to submit against the current revision."
        case "error.invalid_entity":
            return "That firm, facility or policy no longer exists. Go back and choose a current row."
        case "error.arithmetic":
            return "One field is outside the accepted range. Check the units, minimums and that quantities are positive."
        case "error.malformed_command":
            return "The app could not encode a valid command. Report this build; no state changed."
        case "error.duplicate_command":
            return "This command was already accepted. Refresh before submitting again."
        default:
            return "No state changed. Review the actor, amount and authority, then try again."
        }
    }
}

/// Parses a dollar entry into integer cents. Returns nil rather than guessing a bad value.
func parseDollarsToCents(_ text: String) -> Int64? {
    let cleaned = text
        .replacingOccurrences(of: "$", with: "")
        .replacingOccurrences(of: ",", with: "")
        .trimmingCharacters(in: .whitespaces)
    guard !cleaned.isEmpty, let value = Double(cleaned), value.isFinite, value >= 0 else {
        return nil
    }
    let cents = (value * 100).rounded()
    guard cents >= 0, cents < 9_000_000_000_000_000 else { return nil }
    return Int64(cents)
}

/// Parses a whole-number entry such as shares or lots.
func parseCount(_ text: String) -> Int64? {
    let cleaned = text
        .replacingOccurrences(of: ",", with: "")
        .replacingOccurrences(of: " ", with: "")
    guard !cleaned.isEmpty, let value = Int64(cleaned), value > 0 else { return nil }
    return value
}

/// Shown for every command form: who acts, what the session previews, and how a refusal is fixed.
struct PreviewCard: View {
    let preview: PreviewDto
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let blocked = preview.blockedKey {
                PixelLabel(Remedy.title(for: blocked), icon: "blocked")
                    .font(.callout.weight(.medium))
                    .foregroundStyle(Theme.negative)
                Text(Remedy.text(for: blocked))
                    .font(.footnote)
                    .foregroundStyle(Theme.secondaryText(for: scheme))
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text("Cash change \(Format.signedMoney(preview.cashChangeCents))")
                    .font(.callout.monospacedDigit())
                Text("A labelled session estimate. Settlement uses the rules at the next boundary.")
                    .font(.caption2)
                    .foregroundStyle(Theme.secondaryText(for: scheme))
            }
            ForEach(preview.assumptions.indices, id: \.self) { index in
                Text("• \(preview.assumptions[index])")
                    .font(.caption)
                    .foregroundStyle(Theme.secondaryText(for: scheme))
                    .fixedSize(horizontal: false, vertical: true)
            }
            ForEach(preview.warnings.indices, id: \.self) { index in
                Text("• \(preview.warnings[index])")
                    .font(.caption)
                    .foregroundStyle(Theme.warning)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Preview. Cash change \(Format.signedMoney(preview.cashChangeCents)).")
    }
}

struct RejectionNotice: View {
    let rejection: CommandRejection
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            PixelLabel(rejection.title, icon: "warning")
                .font(.callout.weight(.medium))
                .foregroundStyle(Theme.negative)
            Text(rejection.remedy)
                .font(.footnote)
                .foregroundStyle(Theme.text(for: scheme))
                .fixedSize(horizontal: false, vertical: true)
            Text("Nothing was changed. Your entries are kept.")
                .font(.caption2)
                .foregroundStyle(Theme.secondaryText(for: scheme))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

/// Actor header + fields + preview + inline rejection + sticky confirm, shared by every sheet.
struct CommandSheet<Fields: View>: View {
    let title: String
    let actorName: String
    let actorDetail: String
    let submitTitle: String
    let preview: PreviewDto?
    let rejection: CommandRejection?
    let isBusy: Bool
    let canSubmit: Bool
    let onSubmit: () async -> Bool
    @ViewBuilder var fields: Fields

    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var scheme
    @Environment(AppStore.self) private var store
    @State private var submitting = false

    var body: some View {
        NavigationStack {
            Form {
                Section("Legal actor") {
                    Text(actorName)
                        .font(.body.weight(.medium))
                        .fixedSize(horizontal: false, vertical: true)
                    Text(actorDetail)
                        .font(.caption)
                        .foregroundStyle(Theme.secondaryText(for: scheme))
                        .fixedSize(horizontal: false, vertical: true)
                    if let parent = store.actingThroughParent {
                        Text("Authority: \(parent.name) operates this subsidiary.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                    } else {
                        Text("Authority: you, the investor, through your share of the votes.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                fields
                Section("Preview") {
                    if let preview {
                        PreviewCard(preview: preview)
                    } else {
                        Text("Enter the required values to see the session preview.")
                            .font(.footnote)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                    }
                }
                if let rejection {
                    Section("Rejected") {
                        RejectionNotice(rejection: rejection)
                    }
                }
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
            .safeAreaInset(edge: .bottom) {
                confirmBar
            }
        }
    }

    private var confirmBar: some View {
        VStack(spacing: 0) {
            Divider()
            Button {
                guard !submitting else { return }
                submitting = true
                Task {
                    let accepted = await onSubmit()
                    submitting = false
                    if accepted { dismiss() }
                }
            } label: {
                PixelLabel(submitTitle, icon: "check")
                    .frame(maxWidth: .infinity)
                    .frame(minHeight: 44)
            }
            .buttonStyle(PixelButtonStyle(prominent: true))
            .controlSize(.large)
            .disabled(!canSubmit || isBusy || submitting)
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
        }
        .background(Theme.panel)
    }
}

/// A tappable row in the Company hub. Keeps a single 44pt target and wraps at large text.
struct CompanyLink<Destination: View>: View {
    let title: String
    let detail: String
    let icon: String
    @ViewBuilder var destination: Destination
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        NavigationLink {
            destination
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                PixelIcon(icon)
                    .frame(width: 24, height: 24, alignment: .center)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.headline)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(Theme.secondaryText(for: scheme))
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 8)
                PixelIcon("chevron_right", size: 12)
                    .foregroundStyle(Theme.secondaryText(for: scheme))
                    .accessibilityHidden(true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(minHeight: 44)
            .padding(16)
            .background(Theme.surface(for: scheme))
            .clipShape(Pixel.shape)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(title). \(detail)")
    }
}

/// A zero-field command that still deserves a preview and one explicit confirmation.
struct ConfirmRequest: Identifiable {
    let id = UUID()
    let title: String
    let actorName: String
    let detail: String
    let submitTitle: String
    let json: String
    /// Investor-only intents such as a tender response or the primary-group switch.
    var asInvestor: Bool = false
}

struct ConfirmCommandSheet: View {
    let request: ConfirmRequest
    @Environment(AppStore.self) private var store
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    var body: some View {
        CommandSheet(
            title: request.title,
            actorName: request.actorName,
            actorDetail: request.detail,
            submitTitle: request.submitTitle,
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: true,
            onSubmit: {
                let rejection: CommandRejection?
                if request.asInvestor {
                    rejection = await store.submitAsInvestor(request.json)
                } else {
                    rejection = await store.submit(request.json)
                }
                if let rejection {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            EmptyView()
        }
        .task {
            preview = await store.preview(request.json)
        }
    }
}

/// A full-width 44pt action row used inside section cards.
struct ActionButton: View {
    let title: String
    let icon: String
    var role: ButtonRole? = nil
    let action: () -> Void

    var body: some View {
        Button(role: role, action: action) {
            PixelLabel(title, icon: icon)
                .frame(maxWidth: .infinity)
                .frame(minHeight: 44)
        }
        .buttonStyle(PixelButtonStyle())
        .accessibilityHint(title)
    }
}

extension BoardVoteDto: Identifiable {}

extension FacilityDto: Identifiable {
    public var id: UInt64 { handle }
}

extension DealDto: Identifiable {
    public var id: UInt64 { handle }
}

/// Deal states that no longer accept an action.
func dealIsTerminal(_ state: String) -> Bool {
    ["settled", "withdrawn", "expired", "rejected"].contains(state)
}

/// A small label/value row that wraps instead of truncating at large Dynamic Type sizes.
struct DetailRow: View {
    let title: String
    let value: String
    var tone: Int = 0
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(title)
                .font(.callout)
                .foregroundStyle(Theme.secondaryText(for: scheme))
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 8)
            Text(value)
                .font(.callout.monospacedDigit())
                .foregroundStyle(toneColor)
                .multilineTextAlignment(.trailing)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }

    private var toneColor: Color {
        switch tone {
        case 1: return Theme.positive
        case -1: return Theme.negative
        default: return Theme.text(for: scheme)
        }
    }
}

extension RaidOptionDto: Identifiable {
    public var id: String { "\(target)-\(region)-\(industry)" }
}

extension DivisionDto: Identifiable {
    public var id: String { region + "/" + industry }
}
