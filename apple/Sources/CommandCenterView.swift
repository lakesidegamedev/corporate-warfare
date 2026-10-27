import SwiftUI

/// Every player action comes from the engine's catalogue, so a new command appears here without
/// new screens. Built once, read by the Command Center.
enum ActionCatalogue {
    static let all: [ActionDto] = actionCatalogue()
}

/// Loans, contracts, policies and votes of one company and every open deal, for form pickers.
struct ActionPickers {
    var loans: [LoanDto] = []
    var contracts: [ContractDto] = []
    var policies: [InsuranceDto] = []
    var votes: [BoardVoteDto] = []
    var deals: [DealDto] = []
    /// Options the engine lists itself: bonds, ventures and open activist campaigns.
    var extra: [String: [PickDto]] = [:]
}

/// Engine notes give money as "N cents"; show it in dollars.
func dollarsText(_ text: String) -> String {
    guard let regex = try? NSRegularExpression(pattern: "(-?[0-9]+) cents") else { return text }
    let source = text as NSString
    var result = source
    let matches = regex.matches(in: text, range: NSRange(location: 0, length: source.length))
    for match in matches.reversed() {
        guard let cents = Int64(source.substring(with: match.range(at: 1))) else { continue }
        result = result.replacingCharacters(in: match.range, with: Format.money(cents)) as NSString
    }
    return result as String
}

/// A simple wrapping row for chips.
struct ChipFlow: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let limit = proposal.width ?? CGFloat.infinity
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var widest: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > 0 && x + size.width > limit {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
            widest = max(widest, x - spacing)
        }
        return CGSize(width: widest, height: y + rowHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX
        var y = bounds.minY
        var rowHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > bounds.minX && x + size.width > bounds.maxX {
                x = bounds.minX
                y += rowHeight + spacing
                rowHeight = 0
            }
            subview.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
    }
}

/// A pill the player taps to choose.
struct ChoiceChip: View {
    let text: String
    let on: Bool
    var icon: String?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 5) {
                if let icon { PixelIcon(icon, size: 12) }
                Text(text)
            }
                .font(.caption.weight(.semibold))
                .foregroundStyle(on ? Theme.ink : Theme.primaryText)
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(on ? Theme.gold : Theme.panelRaised, in: Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// Every action in the game, searchable. Pick one to get its form: pickers, sliders and switches
/// generated from the engine's catalogue, a live preview of what it would do, and Submit.
struct CommandCenterView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.horizontalSizeClass) private var sizeClass
    var initialQuery = ""
    var onClose: (() -> Void)?

    @State private var query = ""
    @State private var category: String?
    @State private var chosen: String?
    @State private var started = false

    private var wide: Bool { sizeClass == .regular }
    private var all: [ActionDto] { ActionCatalogue.all }

    private var matches: [ActionDto] {
        let words = query.split(separator: " ").map(String.init)
        return all.filter { action in
            (category == nil || action.category == category)
                && words.allSatisfy { word in
                    [action.title, action.summary, action.keywords, action.category].contains {
                        $0.localizedCaseInsensitiveContains(word)
                    }
                }
        }
    }

    private var categories: [String] {
        var seen: [String] = []
        for action in all where !seen.contains(action.category) { seen.append(action.category) }
        return seen
    }

    private var chosenAction: ActionDto? {
        guard let chosen else { return nil }
        return all.first { $0.command == chosen }
    }

    var body: some View {
        Group {
            if !wide, let action = chosenAction {
                ActionFormView(action: action, onBack: { chosen = nil })
                    .id(action.command)
            } else {
                HStack(alignment: .top, spacing: 10) {
                    list
                        .frame(maxWidth: wide ? 420 : .infinity)
                    if wide {
                        if let action = chosenAction {
                            ActionFormView(action: action, onBack: nil)
                                .id(action.command)
                        } else {
                            Text("Pick an action on the left to see its form.")
                                .font(.callout)
                                .foregroundStyle(Theme.secondary)
                                .frame(maxWidth: .infinity, maxHeight: .infinity)
                        }
                    }
                }
                .padding(wide ? 10 : 0)
            }
        }
        .background(Theme.ink)
        .onAppear {
            guard !started else { return }
            started = true
            query = initialQuery
            if !initialQuery.isEmpty, wide { chosen = matches.first?.command }
            store.tutorial.visited(.actions)
        }
    }

    private var list: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Actions")
                    .font(.title3.weight(.bold))
                    .foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                Spacer()
                Text("\(matches.count) of \(all.count)")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                if let onClose {
                    Button("Done", action: onClose).tint(Theme.gold)
                }
            }
            .padding(12)
            TextField("Search: IPO, loan, build, price, research", text: $query)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .padding(.horizontal, 12)
            ChipFlow(spacing: 5) {
                ChoiceChip(text: "All", on: category == nil) { category = nil }
                ForEach(categories, id: \.self) { name in
                    ChoiceChip(text: name, on: category == name, icon: categoryIcon(name)) { category = category == name ? nil : name }
                }
            }
            .padding(12)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(categories, id: \.self) { group in
                        let rows = matches.filter { $0.category == group }
                        if !rows.isEmpty {
                            Text("\(group.uppercased())  \(rows.count)")
                                .font(.caption2.weight(.semibold))
                                .tracking(0.8)
                                .foregroundStyle(Theme.secondary)
                                .padding(.leading, 14)
                                .padding(.top, 10)
                                .padding(.bottom, 4)
                            ForEach(rows, id: \.command) { action in
                                row(action)
                            }
                        }
                    }
                    if matches.isEmpty {
                        Text("Nothing matches. Try another word.")
                            .font(.footnote)
                            .foregroundStyle(Theme.secondary)
                            .padding(14)
                    }
                }
            }
        }
        .background(Theme.panel, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
    }

    private func row(_ action: ActionDto) -> some View {
        let on = chosen == action.command
        return Button {
            chosen = action.command
        } label: {
            HStack(alignment: .top, spacing: 10) {
                PixelIcon(categoryIcon(action.category))
                    .foregroundStyle(on ? Theme.gold : Theme.secondary)
                VStack(alignment: .leading, spacing: 2) {
                    Text(action.title)
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(on ? Theme.gold : Theme.primaryText)
                    Text(action.summary)
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(on ? Theme.gold.opacity(0.14) : Color.clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// One option of a picker field: the value sent to the engine and what the player sees.
private struct Pick: Identifiable {
    let value: String
    let label: String
    var id: String { value }
}

/// The generated form for one action.
struct ActionFormView: View {
    @EnvironmentObject private var store: AppStore
    let action: ActionDto
    var onBack: (() -> Void)?

    @State private var values: [String: String] = [:]
    @State private var pickers = ActionPickers()
    @State private var preview: PreviewDto?
    @State private var result: (Bool, String)?

    // MARK: Values

    private func raw(_ field: FieldDto) -> String {
        values[field.key] ?? defaultOf(field)
    }

    private func defaultOf(_ field: FieldDto) -> String {
        if field.kind == "my_firm" { return store.selectedFirm.map { String($0) } ?? "" }
        if field.encoding == "optional" { return "" }
        switch field.kind {
        case "bool": return field.defaultValue != 0 ? "true" : "false"
        case "choice": return field.choices.first?.value ?? ""
        case "text": return ""
        case "money", "lots", "shares", "bps", "weeks", "count": return String(max(field.defaultValue, field.min))
        default: return ""
        }
    }

    private var firm: UInt64? {
        if let field = action.fields.first(where: { $0.kind == "my_firm" }) {
            return UInt64(raw(field))
        }
        return store.selectedFirm
    }

    /// The legal actor for the chosen company: the investor for a direct holding, the parent
    /// company for a subsidiary.
    private var actor: UInt64 {
        guard let firm, action.fields.contains(where: { $0.kind == "my_firm" }),
              let node = store.groupTree.first(where: { $0.handle == firm }) else { return 0 }
        return node.directlyControlled ? 0 : (node.parent ?? 0)
    }

    /// JSON for the action from the raw field values; nil while a required field is empty.
    private var json: String? {
        var parts: [String] = []
        for field in action.fields {
            let value = raw(field)
            let encoded: String
            switch field.encoding {
            case "bool":
                encoded = value == "true" ? "true" : "false"
            case "string":
                if value.isEmpty { return nil }
                let escaped = value.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"")
                encoded = "\"" + escaped + "\""
            case "optional":
                encoded = value.isEmpty ? "null" : value
            default:
                guard let number = Int64(value) else { return nil }
                encoded = String(number)
            }
            parts.append("\"\(field.key)\":\(encoded)")
        }
        if parts.isEmpty { return CommandJSON.unit(action.command) }
        return "{\"\(action.command)\":{\(parts.joined(separator: ","))}}"
    }

    // MARK: Options

    private func picks(for field: FieldDto) -> [Pick] {
        var list: [Pick]
        switch field.kind {
        case "my_firm":
            list = store.firms.filter { $0.controlled }.map { Pick(value: String($0.handle), label: $0.name) }
        case "any_firm":
            list = store.firms.sorted { $0.controlled && !$1.controlled }
                .map { Pick(value: String($0.handle), label: $0.name + ($0.controlled ? " (yours)" : "")) }
        case "bank", "insurer":
            list = store.firms.filter { $0.industry == "financial" }.map { Pick(value: String($0.handle), label: $0.name) }
        case "region":
            list = store.boards.sorted { $0.index < $1.index }
                .map { Pick(value: String($0.index), label: "\($0.name), \($0.stateCode)") }
        case "product":
            list = store.productCatalogue.filter { $0.referencePriceCents > 0 }
                .map { Pick(value: String($0.index), label: $0.name) }
        case "recipe":
            list = store.strategyCatalogue.map { Pick(value: String($0.index), label: "\($0.name) (\(Sector.name($0.industry)))") }
        case "sector":
            list = Sector.engineOrder.enumerated().map { Pick(value: String($0.offset), label: Sector.name($0.element)) }
        case "my_facility":
            list = store.facilities.map { plant in
                let name = store.strategyCatalogue.first { $0.id == plant.recipe }?.name ?? plant.recipe
                return Pick(value: String(plant.handle), label: "\(name) · \(plant.region) · \(plant.blocks) plants")
            }
        case "my_loan":
            list = pickers.loans.map {
                Pick(value: String($0.handle),
                     label: "\(Format.moneyCompact($0.principalCents)) from \($0.lender) at \(String(format: "%.2f", Double($0.rateBps) / 100))%")
            }
        case "my_contract":
            list = pickers.contracts.map {
                Pick(value: String($0.handle), label: "\($0.product): \($0.seller) to \($0.buyer), \($0.quantityWeekly) a week")
            }
        case "my_policy":
            list = pickers.policies.map {
                Pick(value: String($0.handle), label: "\($0.insurer) cover, \(Format.moneyCompact($0.limitCents))")
            }
        case "vote":
            list = pickers.votes.filter { !$0.resolved }.map {
                Pick(value: String($0.id), label: "\($0.subject), closes week \($0.deadline)")
            }
        case "my_deal":
            list = pickers.deals.map { Pick(value: String($0.handle), label: "\($0.buyer) for \($0.target) (\($0.state))") }
        case "bond", "my_bond", "my_venture", "open_campaign":
            list = (pickers.extra[field.kind] ?? []).map { Pick(value: $0.value, label: $0.label) }
        default:
            list = []
        }
        if field.encoding == "optional" { list.insert(Pick(value: "", label: "None"), at: 0) }
        return list
    }

    private func ceiling(for field: FieldDto) -> Int64 {
        if field.max > 0 { return field.max }
        if field.kind == "money" && field.key.contains("price") { return 100_000 }
        switch field.kind {
        case "money":
            return max(store.selectedFirmDto?.cashCents ?? 0, store.overview?.investorCashCents ?? 0, 100_000_000)
        case "shares": return 10_000_000
        case "lots": return 10_000
        case "weeks": return 520
        default: return 100
        }
    }

    // MARK: Body

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack(alignment: .center) {
                    if let onBack {
                        Button {
                            onBack()
                        } label: {
                            PixelLabel("Actions", icon: "chevron_left")
                                .font(.callout)
                        }
                        .tint(Theme.gold)
                    }
                    Spacer()
                }
                VStack(alignment: .leading, spacing: 2) {
                    Eyebrow(text: action.category)
                    Text(action.title)
                        .font(.title2.weight(.bold))
                        .foregroundStyle(Theme.primaryText)
                        .accessibilityAddTraits(.isHeader)
                }
                Text(action.summary)
                    .font(.callout)
                    .foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                ForEach(action.fields, id: \.key) { field in
                    fieldControl(field)
                }
                previewBox
                submitRow
            }
            .padding(18)
        }
        .background(Theme.panel, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
        .task(id: firm ?? 0) {
            pickers = await store.pickers(firm: firm)
        }
        .task(id: json ?? "") {
            preview = nil
            guard let text = json else { return }
            try? await Task.sleep(nanoseconds: 300_000_000)
            if Task.isCancelled { return }
            preview = await store.preview(text)
        }
    }

    private var previewBox: some View {
        VStack(alignment: .leading, spacing: 4) {
            Eyebrow(text: "What it would do")
            if json == nil {
                Text("Fill in every field to see the effect.")
                    .font(.footnote).foregroundStyle(Theme.secondary)
            } else if let preview {
                if let key = preview.blockedKey {
                    Text("Not allowed: " + CommandRejection(key: key, detail: preview.blockedDetail ?? "").message)
                        .font(.footnote).foregroundStyle(Theme.negative)
                        .fixedSize(horizontal: false, vertical: true)
                } else {
                    if preview.cashChangeCents != 0 {
                        Text("Cash \(Format.signedMoney(preview.cashChangeCents))")
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(preview.cashChangeCents >= 0 ? Theme.positive : Theme.negative)
                    } else {
                        Text("No immediate cash change.")
                            .font(.footnote).foregroundStyle(Theme.primaryText)
                    }
                    ForEach(Array(preview.warnings.enumerated()), id: \.offset) { _, line in
                        PixelLabel(dollarsText(line), icon: "warning")
                            .font(.caption).foregroundStyle(Theme.warning)
                    }
                    ForEach(Array(preview.assumptions.enumerated()), id: \.offset) { _, line in
                        Text("· " + dollarsText(line))
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
            } else {
                Text("Working it out")
                    .font(.footnote).foregroundStyle(Theme.secondary)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.ink, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private var submitRow: some View {
        HStack(spacing: 12) {
            Button {
                guard let text = json else { return }
                result = nil
                let who = actor
                Task {
                    if let rejection = await store.submit(text, actor: who) {
                        result = (false, rejection.message)
                    } else {
                        result = (true, "Done.")
                    }
                }
            } label: {
                Text("Submit").frame(minWidth: 90)
            }
            .buttonStyle(.borderedProminent)
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
            .controlSize(.large)
            .disabled(json == nil || preview?.blockedKey != nil || store.isBusy)
            if let result {
                Text(result.1)
                    .font(.footnote)
                    .foregroundStyle(result.0 ? Theme.positive : Theme.negative)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    // MARK: Fields

    @ViewBuilder
    private func fieldControl(_ field: FieldDto) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(field.label)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Theme.primaryText)
            switch field.kind {
            case "bool":
                Toggle("", isOn: Binding(
                    get: { raw(field) == "true" },
                    set: { values[field.key] = $0 ? "true" : "false" }
                ))
                .labelsHidden()
                .tint(Theme.gold)
            case "choice":
                ChipFlow(spacing: 6) {
                    ForEach(field.choices, id: \.value) { option in
                        ChoiceChip(text: option.label, on: raw(field) == option.value) {
                            values[field.key] = option.value
                        }
                    }
                }
            case "text":
                TextField(field.key == "name" ? "Your company's name" : "", text: Binding(
                    get: { raw(field) },
                    set: { values[field.key] = $0 }
                ))
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
            case "money", "lots", "shares", "weeks", "count", "bps":
                ActionNumberControl(
                    field: field,
                    ceiling: ceiling(for: field),
                    raw: Binding(get: { raw(field) }, set: { values[field.key] = $0 })
                )
            default:
                picker(field)
            }
            if !field.help.isEmpty {
                Text(field.help)
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    @ViewBuilder
    private func picker(_ field: FieldDto) -> some View {
        let options = picks(for: field)
        let current = raw(field)
        if options.isEmpty {
            Text("Nothing to choose yet")
                .font(.callout)
                .foregroundStyle(Theme.secondary)
        } else {
            Picker(field.label, selection: Binding(
                get: { current },
                set: { values[field.key] = $0 }
            )) {
                if !options.contains(where: { $0.value == current }) {
                    Text("Choose").tag(current)
                }
                ForEach(options) { option in
                    Text(option.label).tag(option.value)
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .tint(Theme.gold)
            .padding(.horizontal, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.ink, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
        }
    }
}

/// A slider with an exact box under it. Money is shown in dollars, percentages in percent.
struct ActionNumberControl: View {
    let field: FieldDto
    let ceiling: Int64
    @Binding var raw: String
    @State private var text = ""

    private var scale: Double { field.kind == "money" || field.kind == "bps" ? 100 : 1 }
    private var value: Int64 { Int64(raw) ?? field.defaultValue }
    private var low: Double { Double(field.min) / scale }
    private var high: Double { max(Double(ceiling) / scale, low + 1) }
    private var step: Double {
        let base = Double(max(field.step, 1)) / scale
        return max(base, field.kind == "money" ? 1.0 : 0.01)
    }

    private func label(_ dollars: Double) -> String {
        switch field.kind {
        case "money": return Format.money(Int64((dollars * 100).rounded()))
        case "bps": return String(format: "%.2f%%", dollars)
        case "weeks": return "\(Int64(dollars)) weeks"
        case "shares": return "\(Int64(dollars).formatted()) shares"
        case "lots": return "\(Int64(dollars).formatted()) lots"
        default: return Int64(dollars).formatted()
        }
    }

    private func display(_ stored: Int64) -> String {
        switch field.kind {
        case "money": return String(stored / 100)
        case "bps": return String(format: "%.2f", Double(stored) / 100)
        default: return String(stored)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ValueSlider(
                value: Binding(
                    get: { min(max(Double(value) / scale, low), high) },
                    set: { raw = String(Int64(($0 * scale).rounded())) }
                ),
                range: low...high, step: step, ticks: 10,
                label: label
            )
            HStack(spacing: 6) {
                if field.kind == "money" { Text("$").foregroundStyle(Theme.secondary) }
                TextField("Exact amount", text: $text)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.decimalPad)
                    .frame(maxWidth: 180)
                if field.kind == "bps" { Text("%").foregroundStyle(Theme.secondary) }
            }
        }
        .onAppear { text = display(value) }
        .onChange(of: text) { _, new in
            guard let number = Double(new.replacingOccurrences(of: ",", with: "")) else { return }
            let stored = Int64((number * scale).rounded())
            if String(stored) != raw { raw = String(stored) }
        }
        .onChange(of: raw) { _, _ in
            let typed = Double(text.replacingOccurrences(of: ",", with: "")).map { Int64(($0 * scale).rounded()) }
            if typed != value { text = display(value) }
        }
    }
}
