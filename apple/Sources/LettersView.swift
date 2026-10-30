import SwiftUI

// The stationery: warm paper, dark ink, one red seal. Fixed colours so a letter reads the same
// in any appearance.
private enum Stationery {
    static let sheet = Color(red: 0.918, green: 0.875, blue: 0.753)      // #EADFC0
    static let edge = Color(red: 0.725, green: 0.647, blue: 0.478)       // #B9A57A
    static let rule = Color(red: 0.788, green: 0.722, blue: 0.561)       // #C9B88F
    static let ink = Color(red: 0.165, green: 0.129, blue: 0.086)        // #2A2116
    static let faded = Color(red: 0.420, green: 0.357, blue: 0.243)      // #6B5B3E
    static let seal = Color(red: 0.608, green: 0.231, blue: 0.180)       // #9B3B2E
}

extension LetterDto: Identifiable {}

/// Weeks the Inbox keeps before a letter moves to the archive.
let inboxWeeks: UInt32 = 8

/// A sender's face: the cast portrait for a person (looked up by id, so a larger atlas needs
/// nothing here), a plain tile for an office.
struct SenderPortrait: View {
    let sender: LetterSenderDto
    var size: CGFloat = 40

    var body: some View {
        if !sender.person.isEmpty, CastPortraits.all[sender.person] != nil {
            PixelPortrait(person: sender.person, size: size, mood: sender.mood)
        } else {
            PixelIcon("people", size: size / 2)
                .foregroundStyle(Theme.secondary)
                .frame(width: size, height: size)
                .background(Theme.panelRaised, in: Pixel.small)
                .overlay(Pixel.small.stroke(Theme.stroke, lineWidth: 1))
                .accessibilityHidden(true)
        }
    }
}

private func openLabel(_ link: PaperLinkDto) -> String {
    switch link.kind {
    case "company": return "Open company"
    case "deal": return "Review the bid"
    case "event": return "Read the notice"
    case "good": return "Open market"
    case "region": return "Open region"
    default: return ""
    }
}

/// One letter in the stack: portrait, sender, first line, date and an unread dot.
struct LetterRow: View {
    let letter: LetterDto

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            SenderPortrait(sender: letter.sender, size: 44)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(letter.sender.name)
                        .font(.callout.weight(letter.read ? .regular : .bold))
                        .foregroundStyle(Theme.primaryText)
                        .lineLimit(1)
                    if !letter.read {
                        Rectangle().fill(Theme.gold).frame(width: 8, height: 8).clipShape(Pixel.small)
                    }
                }
                let line = [letter.sender.role, letter.sender.organisation]
                    .filter { !$0.isEmpty && $0 != letter.sender.name }.joined(separator: " · ")
                Text(line).font(.caption2).foregroundStyle(Theme.secondary).lineLimit(1)
                Text(letter.summary)
                    .font(.caption)
                    .foregroundStyle(Theme.primaryText.opacity(letter.read ? 0.7 : 1))
                    .lineLimit(2)
            }
            Spacer(minLength: 4)
            VStack(alignment: .trailing, spacing: 4) {
                Text(ResultBuckets.weekLabel(letter.week))
                    .font(Pixel.font(10)).foregroundStyle(Theme.secondary)
                if letter.event != nil {
                    SignalPill(
                        text: letter.mandatory ? "Due \(ResultBuckets.weekLabel(letter.deadline))" : "Reply",
                        tint: letter.mandatory ? Theme.warning : Theme.gold
                    )
                }
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.panel, in: Pixel.shape)
        .overlay(Pixel.shape.stroke(Theme.stroke, lineWidth: 1))
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
    }
}

/// An open letter on a paper-like panel: letterhead with the organisation, the portrait, the
/// body, the signature, then the buttons the inbox always offered.
struct LetterPaperView: View {
    @EnvironmentObject private var store: AppStore
    let letter: LetterDto
    @State private var target: PaperTarget?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                SenderPortrait(sender: letter.sender, size: 56)
                VStack(alignment: .leading, spacing: 2) {
                    Text((letter.sender.organisation.isEmpty ? letter.sender.role : letter.sender.organisation).uppercased())
                        .font(Pixel.font(17)).tracking(0.6).foregroundStyle(Stationery.ink)
                    Text([letter.sender.role, ResultBuckets.weekLabel(letter.week)]
                        .filter { !$0.isEmpty }.joined(separator: " · "))
                        .font(.caption).foregroundStyle(Stationery.faded)
                }
                Spacer(minLength: 0)
                if letter.event != nil && letter.mandatory {
                    Text("DUE \(ResultBuckets.weekLabel(letter.deadline))")
                        .font(Pixel.font(11)).foregroundStyle(Stationery.seal)
                }
            }
            Rectangle().fill(Stationery.ink).frame(height: 2)
            Rectangle().fill(Stationery.rule).frame(height: 1)
            Text(letter.salutation).font(.callout.weight(.medium)).foregroundStyle(Stationery.ink)
            ForEach(Array(letter.paragraphs.enumerated()), id: \.offset) { _, paragraph in
                Text(paragraph).font(.callout).foregroundStyle(Stationery.ink)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Text(letter.signOff + ",").font(.callout).foregroundStyle(Stationery.ink).padding(.top, 2)
            Text(letter.sender.name).font(.headline.italic()).foregroundStyle(Stationery.ink)
            let label = openLabel(letter.link)
            if letter.event != nil || !label.isEmpty {
                Rectangle().fill(Stationery.rule).frame(height: 1)
                if let event = letter.event {
                    ForEach(letter.choices, id: \.id) { choice in
                        paperButton(choice.label, filled: true) {
                            Task { await store.respond(event: event, choice: choice.id) }
                        }
                        .disabled(store.isBusy)
                    }
                }
                if !label.isEmpty {
                    paperButton(label, filled: false) { target = PaperTarget(link: letter.link, story: nil) }
                }
            }
        }
        .padding(18)
        .frame(maxWidth: 620, alignment: .leading)
        .background(Stationery.sheet, in: Pixel.shape)
        .overlay(Pixel.shape.stroke(Stationery.edge, lineWidth: 2))
        .sheet(item: $target) { target in
            PaperTargetSheet(target: target).environmentObject(store)
        }
        .task(id: letter.id) {
            if !letter.read { await store.markLettersRead([letter.id]) }
        }
    }

    private func paperButton(_ text: String, filled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(text)
                .font(.callout.weight(.semibold))
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 12).padding(.vertical, 9)
                .foregroundStyle(filled ? Stationery.sheet : Stationery.ink)
                .background(filled ? Stationery.ink : Color.clear, in: Pixel.shape)
                .overlay(Pixel.shape.stroke(Stationery.ink, lineWidth: 1))
        }
        .buttonStyle(.plain)
    }
}

/// The letters of the last eight weeks, then older ones on request.
struct LetterStack: View {
    @EnvironmentObject private var store: AppStore
    var onOpen: (LetterDto) -> Void
    @State private var showArchive = false
    @State private var archive: [LetterDto] = []

    var body: some View {
        VStack(spacing: 6) {
            if store.letters.isEmpty {
                CalloutRow(icon: "check_circle", text: "Quiet week. Letters arrive when something happens.", tint: Theme.secondary)
            }
            ForEach(store.letters, id: \.id) { letter in
                Button { onOpen(letter) } label: { LetterRow(letter: letter) }.buttonStyle(.plain)
            }
            Button { showArchive.toggle() } label: {
                Text(showArchive ? "HIDE OLDER LETTERS" : "OLDER LETTERS")
                    .font(Pixel.font(11)).foregroundStyle(Theme.gold)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 6)
            }
            .buttonStyle(.plain)
            if showArchive {
                if archive.isEmpty { Text("Nothing older yet.").font(.caption).foregroundStyle(Theme.secondary) }
                ForEach(archive, id: \.id) { letter in
                    Button { onOpen(letter) } label: { LetterRow(letter: letter) }.buttonStyle(.plain)
                }
            }
        }
        // The archive is read when the player asks for it and time is not running, never at speed.
        .task(id: showArchive) {
            guard showArchive, store.runSpeed == 0 else { return }
            let now = store.overview?.tick ?? 0
            let all = await store.read { $0.letters(sinceWeek: 1) } ?? []
            archive = all.filter { $0.week + inboxWeeks <= now && $0.event == nil }
        }
    }
}
