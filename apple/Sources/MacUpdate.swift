import CryptoKit
import Foundation
import Observation
import SwiftUI
import UIKit

/// Why an update step did not finish. Each one has plain words in `UpdateNotice`.
enum UpdateError: Error, Equatable {
    case unauthorized
    case notEntitled
    case noBuild
    case rateLimited
    case offline
    case noChecksum
    case mismatch
    case server(Int)
    case badReply
}

struct DownloadFile: Equatable {
    var platform: String
    var name: String
    var size: Int64?
    var sha256: String?
    var url: String
}

struct DownloadListing: Equatable {
    var version: String?
    var entitled: Bool
    var files: [DownloadFile]

    var macFile: DownloadFile? { files.first { $0.platform == "macos-universal" } }
}

/// Comparing dotted versions such as 2.0.2 with 2.0.1. A leading v and anything after a dash is ignored.
enum AppVersion {
    static func parts(_ text: String) -> [Int] {
        var t = text.trimmingCharacters(in: .whitespaces)
        if t.hasPrefix("v") || t.hasPrefix("V") { t.removeFirst() }
        t = String(t.split(separator: "-", maxSplits: 1).first ?? "")
        return t.split(separator: ".").map { Int($0) ?? 0 }
    }

    static func isNewer(_ candidate: String, than installed: String) -> Bool {
        let a = parts(candidate), b = parts(installed)
        guard !a.isEmpty else { return false }
        for i in 0..<max(a.count, b.count) {
            let x = i < a.count ? a[i] : 0
            let y = i < b.count ? b[i] : 0
            if x != y { return x > y }
        }
        return false
    }

    static var installed: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0"
    }
}

/// The Lakeside downloads API for the paid Mac build, plus the public GitHub release tag as a hint.
struct LakesideDownloads {
    var base = LakesideSignIn.accountBase
    var configuration: URLSessionConfiguration = .ephemeral
    var releaseURL = "https://api.github.com/repos/lakesidegamedev/corporate-warfare/releases/latest"

    static let platform = "macos-universal"
    static let storeURL = URL(string: "https://lakesidegames.net/store/corporate-warfare/")!

    static func error(forStatus status: Int) -> UpdateError? {
        switch status {
        case 200..<300: return nil
        case 401: return .unauthorized
        case 403: return .notEntitled
        case 404: return .noBuild
        case 429: return .rateLimited
        default: return .server(status)
        }
    }

    private func session() -> URLSession {
        URLSession(configuration: configuration, delegate: NoRedirects(), delegateQueue: nil)
    }

    private func get(_ url: URL, token: String?) async throws -> Data {
        var request = URLRequest(url: url)
        request.timeoutInterval = 15
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        let s = session()
        defer { s.finishTasksAndInvalidate() }
        do {
            let (data, response) = try await s.data(for: request)
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            if let problem = Self.error(forStatus: status) { throw problem }
            guard data.count <= 256 * 1024 else { throw UpdateError.badReply }
            return data
        } catch let e as UpdateError {
            throw e
        } catch {
            throw UpdateError.offline
        }
    }

    /// What Lakeside offers this token: the version, whether the account owns the game, and the files.
    func listing(token: String) async throws -> DownloadListing {
        guard let url = URL(string: base + "/account/api/downloads/corporate-warfare") else { throw UpdateError.badReply }
        let data = try await get(url, token: token)
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw UpdateError.badReply }
        let files: [DownloadFile] = (object["files"] as? [[String: Any]] ?? []).compactMap { f in
            guard let platform = f["platform"] as? String, let url = f["url"] as? String else { return nil }
            return DownloadFile(platform: platform, name: f["name"] as? String ?? "", size: (f["size"] as? NSNumber)?.int64Value,
                                sha256: f["sha256"] as? String, url: url)
        }
        return DownloadListing(version: object["version"] as? String, entitled: object["entitled"] as? Bool ?? false, files: files)
    }

    /// The free-access message to show on the sign-in prompt, or nil when there is none or it cannot be read.
    func freeAccessMessage() async -> String? {
        guard let url = URL(string: base + "/account/api/free-access"),
              let data = try? await get(url, token: nil),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              object["on"] as? Bool == true,
              let message = (object["message"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !message.isEmpty else { return nil }
        return String(message.filter { !$0.isNewline }.prefix(240))
    }

    /// The tag of the latest public release, used only as a hint that something newer exists.
    func latestPublicTag() async throws -> String? {
        guard let url = URL(string: releaseURL) else { return nil }
        do {
            let data = try await get(url, token: nil)
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            return object?["tag_name"] as? String
        } catch UpdateError.offline {
            throw UpdateError.offline
        } catch {
            // A refusal from the public release list (rate limit, say) is not about the player's account.
            return nil
        }
    }

    /// Download the file with the token to Lakeside only (the storage link gets no header), then check
    /// the size and SHA-256 before anything else. Returns the verified file inside `directory`.
    func download(_ file: DownloadFile, token: String, to directory: URL, progress: @escaping (Double) -> Void) async throws -> URL {
        guard let expected = file.sha256?.lowercased(), expected.count == 64, expected.allSatisfy({ $0.isHexDigit }) else {
            throw UpdateError.noChecksum
        }
        guard file.url.hasPrefix("/"), !file.url.hasPrefix("//"), let url = URL(string: base + file.url) else { throw UpdateError.badReply }
        var request = URLRequest(url: url)
        request.timeoutInterval = 60
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        let fm = FileManager.default
        let scratch = fm.temporaryDirectory.appendingPathComponent("cw-update-\(UUID().uuidString).part")
        defer { try? fm.removeItem(at: scratch) }
        let status = try await FileFetcher(configuration: configuration, scratch: scratch, progress: progress).fetch(request)
        if let problem = Self.error(forStatus: status) { throw problem }
        let attributes = try? fm.attributesOfItem(atPath: scratch.path)
        let actual = (attributes?[.size] as? NSNumber)?.int64Value ?? -1
        if let size = file.size, size != actual { throw UpdateError.mismatch }
        guard let digest = Self.sha256(of: scratch), digest == expected else { throw UpdateError.mismatch }
        let safeName = URL(fileURLWithPath: file.name).lastPathComponent
        let destination = directory.appendingPathComponent(safeName.isEmpty ? "CorporateWarfare.dmg" : safeName)
        do {
            try fm.createDirectory(at: directory, withIntermediateDirectories: true)
            try? fm.removeItem(at: destination)
            try fm.moveItem(at: scratch, to: destination)
        } catch {
            throw UpdateError.server(0)
        }
        return destination
    }

    static func sha256(of url: URL) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? handle.close() }
        var hasher = SHA256()
        do {
            while let chunk = try handle.read(upToCount: 1 << 20), !chunk.isEmpty {
                hasher.update(data: chunk)
            }
        } catch {
            return nil
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }
}

/// One file download. Redirects (the 302 to storage) are followed with the Authorization header removed.
final class FileFetcher: NSObject, URLSessionDownloadDelegate {
    private let configuration: URLSessionConfiguration
    private let scratch: URL
    private let progress: (Double) -> Void
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Int, Error>?
    private var status = 0
    private var moveFailed = false

    init(configuration: URLSessionConfiguration, scratch: URL, progress: @escaping (Double) -> Void) {
        self.configuration = configuration
        self.scratch = scratch
        self.progress = progress
    }

    func fetch(_ request: URLRequest) async throws -> Int {
        let session = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
        defer { session.finishTasksAndInvalidate() }
        return try await withCheckedThrowingContinuation { (resume: CheckedContinuation<Int, Error>) in
            lock.lock()
            continuation = resume
            lock.unlock()
            session.downloadTask(with: request).resume()
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        var next = request
        next.setValue(nil, forHTTPHeaderField: "Authorization")
        completionHandler(next)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64,
                    totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        if totalBytesExpectedToWrite > 0 { progress(min(1, Double(totalBytesWritten) / Double(totalBytesExpectedToWrite))) }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        let code = (downloadTask.response as? HTTPURLResponse)?.statusCode ?? 0
        lock.lock()
        status = code
        lock.unlock()
        guard (200..<300).contains(code) else { return }
        do {
            try? FileManager.default.removeItem(at: scratch)
            try FileManager.default.moveItem(at: location, to: scratch)
        } catch {
            lock.lock()
            moveFailed = true
            lock.unlock()
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        lock.lock()
        let waiting = continuation
        continuation = nil
        let code = status != 0 ? status : ((task.response as? HTTPURLResponse)?.statusCode ?? 0)
        let failedMove = moveFailed
        lock.unlock()
        if error != nil {
            waiting?.resume(throwing: UpdateError.offline)
        } else if failedMove {
            waiting?.resume(throwing: UpdateError.server(0))
        } else {
            waiting?.resume(returning: code)
        }
    }
}

/// What the player is told when an update step does not go through, and which buttons go with it.
struct UpdateNotice: Equatable {
    var message: String
    var offersBuy = false
    var offersSignIn = false

    static func make(_ error: UpdateError) -> UpdateNotice {
        switch error {
        case .unauthorized:
            return UpdateNotice(message: "Your Lakeside sign-in has ended. Sign in again to download.", offersSignIn: true)
        case .notEntitled:
            return UpdateNotice(message: "Your account does not include Corporate Warfare.", offersBuy: true)
        case .noBuild:
            return UpdateNotice(message: "There is no Mac build published yet. Try again later.")
        case .rateLimited:
            return UpdateNotice(message: "Lakeside is busy. Try again in a minute.")
        case .offline:
            return UpdateNotice(message: "Could not reach Lakeside. Keep playing and try again later.")
        case .noChecksum:
            return UpdateNotice(message: "Lakeside did not publish a checksum for this file, so it was not downloaded.")
        case .mismatch:
            return UpdateNotice(message: "The download did not match what Lakeside published, so it was thrown away and nothing was opened. Try again.")
        case .server, .badReply:
            return UpdateNotice(message: "Lakeside gave an unexpected answer. Try again later.")
        }
    }
}

/// The in-game update of the paid Mac build: check, sign in to Lakeside, download, verify, open.
/// Only Mac Catalyst shows it, and never when the Lakeside launcher started the game.
@MainActor
@Observable
final class MacUpdater {
    enum Phase: Equatable {
        case idle
        case checking
        case upToDate
        case available(version: String)
        case downloading(Double)
        case downloaded(String)
    }

    static let shared = MacUpdater()

    /// True only on the Mac, and only when the launcher did not start the game.
    static var enabled: Bool {
        #if targetEnvironment(macCatalyst)
        return !LauncherSession.current.managed
        #else
        return false
        #endif
    }

    private(set) var phase = Phase.idle
    private(set) var notice: UpdateNotice?
    private(set) var signedInName: String?
    private(set) var freeAccess: String?
    private(set) var signingIn = false
    private(set) var launcherAccount: LakesideAccount?
    /// True when the version came from the public release tag, so the download needs a sign-in.
    private(set) var isHint = false

    @ObservationIgnored private let auth: LakesideAuth
    @ObservationIgnored private let downloads: LakesideDownloads
    @ObservationIgnored private let directory: () -> URL
    @ObservationIgnored private let openFile: (URL) async -> Bool
    @ObservationIgnored private let installed: String
    @ObservationIgnored private let now: () -> Date
    @ObservationIgnored private var listing: DownloadListing?
    @ObservationIgnored private var coolDownUntil = Date.distantPast
    @ObservationIgnored private var started = false

    init(auth: LakesideAuth = LakesideSignIn(),
         downloads: LakesideDownloads = LakesideDownloads(),
         directory: @escaping () -> URL = { FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first ?? FileManager.default.temporaryDirectory },
         openFile: @escaping (URL) async -> Bool = { url in await UIApplication.shared.open(url) },
         installed: String = AppVersion.installed,
         now: @escaping () -> Date = Date.init) {
        self.auth = auth
        self.downloads = downloads
        self.directory = directory
        self.openFile = openFile
        self.installed = installed
        self.now = now
    }

    /// Launch: redeem a launcher handoff once, then (outside the launcher) check for an update.
    func start() async {
        guard !started else { return }
        started = true
        if LauncherSession.current.managed {
            launcherAccount = await LauncherSession.current.redeemOnce()
            return
        }
        guard Self.enabled else { return }
        auth.restore()
        signedInName = auth.name
        await check(manual: false)
    }

    /// Ask Lakeside (signed in) or the public release tag (signed out) whether a newer version is out.
    func check(manual: Bool) async {
        guard Self.enabled || manual else { return }
        if case .checking = phase { return }
        if case .downloading = phase { return }
        if blockedByCoolDown() { return }
        phase = .checking
        notice = nil
        signedInName = auth.name
        if freeAccess == nil { freeAccess = await downloads.freeAccessMessage() }
        do {
            if let token = auth.token() {
                let found = try await downloads.listing(token: token)
                listing = found
                guard found.entitled else {
                    phase = .idle
                    notice = UpdateNotice.make(.notEntitled)
                    return
                }
                isHint = false
                if let version = found.version, AppVersion.isNewer(version, than: installed) {
                    phase = .available(version: version)
                } else {
                    phase = .upToDate
                }
            } else {
                signedInName = nil
                isHint = true
                if let tag = try await downloads.latestPublicTag(), AppVersion.isNewer(tag, than: installed) {
                    phase = .available(version: tag.hasPrefix("v") ? String(tag.dropFirst()) : tag)
                } else {
                    phase = .upToDate
                }
            }
        } catch let e as UpdateError {
            fail(e, quiet: !manual && e == .offline)
        } catch {
            fail(.offline, quiet: !manual)
        }
    }

    func signIn(provider: String) async {
        guard !signingIn else { return }
        signingIn = true
        defer { signingIn = false }
        if freeAccess == nil { freeAccess = await downloads.freeAccessMessage() }
        switch await auth.signIn(provider: provider) {
        case .done(let name):
            signedInName = name
            notice = nil
            await check(manual: true)
        case .failed(let message):
            notice = UpdateNotice(message: message)
        case .cancelled:
            break
        }
    }

    func cancelSignIn() { auth.cancelSignIn() }

    func signOut() {
        auth.signOut()
        signedInName = nil
        listing = nil
        isHint = true
        notice = nil
    }

    /// Download the Mac build, check it, save it to Downloads and open the disk image.
    func download() async {
        if case .downloading = phase { return }
        guard let token = auth.token() else {
            signedInName = nil
            notice = UpdateNotice.make(.unauthorized)
            return
        }
        if blockedByCoolDown() { return }
        let before = phase
        notice = nil
        phase = .downloading(0)
        do {
            var current = listing
            if current == nil { current = try await downloads.listing(token: token) }
            guard let found = current, found.entitled else { throw UpdateError.notEntitled }
            guard let file = found.macFile else { throw UpdateError.noBuild }
            let saved = try await downloads.download(file, token: token, to: directory()) { [weak self] fraction in
                Task { @MainActor in
                    if let self, case .downloading = self.phase { self.phase = .downloading(fraction) }
                }
            }
            phase = .downloaded(saved.lastPathComponent)
            _ = await openFile(saved)
        } catch let e as UpdateError {
            phase = before
            fail(e, quiet: false)
        } catch {
            phase = before
            fail(.offline, quiet: false)
        }
    }

    private func fail(_ error: UpdateError, quiet: Bool) {
        if error == .unauthorized {
            // The token is no good any more: forget it so the next step asks for a sign-in.
            auth.signOut()
            signedInName = nil
        }
        if error == .rateLimited { coolDownUntil = now().addingTimeInterval(60) }
        if case .checking = phase { phase = .idle }
        if !quiet { notice = UpdateNotice.make(error) }
    }

    private func blockedByCoolDown() -> Bool {
        if now() < coolDownUntil {
            notice = UpdateNotice.make(.rateLimited)
            return true
        }
        return false
    }

    static let savedNote = "Drag Corporate Warfare into Applications to finish. Your saves are kept."
}

// MARK: - Views

/// The calm banner on the main menu: a new version is out. Draws nothing otherwise.
@MainActor
struct MacUpdateBanner: View {
    private let updater = MacUpdater.shared

    var body: some View {
        if case .available(let version) = updater.phase {
            Panel(padding: 12) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Version \(version) is out").font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    MacUpdateActions()
                }
            }
        } else if case .downloaded(let name) = updater.phase {
            Panel(padding: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("\(name) is in your Downloads").font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    Text(MacUpdater.savedNote).font(.caption).foregroundStyle(Theme.secondary)
                }
            }
        } else if case .downloading(let fraction) = updater.phase {
            Panel(padding: 12) { MacUpdateProgress(fraction: fraction) }
        } else if let notice = updater.notice {
            Panel(padding: 12) { MacUpdateNoticeView(notice: notice) }
        }
    }
}

@MainActor
private struct MacUpdateProgress: View {
    let fraction: Double
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Downloading the update").font(.callout).foregroundStyle(Theme.primaryText)
            ProgressView(value: fraction).tint(Theme.gold)
            Text("\(Int(fraction * 100)) percent").font(.caption2).foregroundStyle(Theme.secondary)
        }
    }
}

@MainActor
private struct MacUpdateNoticeView: View {
    let notice: UpdateNotice
    private let updater = MacUpdater.shared
    @Environment(\.openURL) private var openURL

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(notice.message).font(.callout).foregroundStyle(Theme.primaryText)
            if notice.offersBuy {
                Button("Buy") { openURL(LakesideDownloads.storeURL) }.tint(Theme.gold)
            }
            if notice.offersSignIn {
                Button("Sign in to Lakeside") { Task { await updater.signIn(provider: LakesideProvider.ahd) } }.tint(Theme.gold)
            }
        }
    }
}

/// Download, or the sign-in buttons, depending on whether a Lakeside sign-in exists.
@MainActor
private struct MacUpdateActions: View {
    private let updater = MacUpdater.shared

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if updater.signedInName != nil {
                Button("Download") { Task { await updater.download() } }.tint(Theme.gold)
            } else {
                if let free = updater.freeAccess {
                    Text(free).font(.caption).foregroundStyle(Theme.positive)
                }
                Button("Sign in to Lakeside to download") { Task { await updater.signIn(provider: LakesideProvider.ahd) } }
                    .tint(Theme.gold)
                    .disabled(updater.signingIn)
                Button("Continue with Discord") { Task { await updater.signIn(provider: LakesideProvider.discord) } }
                    .tint(Theme.gold)
                    .disabled(updater.signingIn)
                if updater.signingIn {
                    Button("Cancel sign-in") { updater.cancelSignIn() }.tint(Theme.secondary)
                }
            }
            if let notice = updater.notice { MacUpdateNoticeView(notice: notice) }
        }
    }
}

/// Settings: check for updates, and the Lakeside sign-in. Mac only; the call site is guarded.
@MainActor
struct MacUpdateSettingsPanel: View {
    private let updater = MacUpdater.shared

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                Eyebrow(text: "Updates")
                Button("Check for updates") { Task { await updater.check(manual: true) } }
                    .tint(Theme.gold)
                    .disabled(updater.phase == .checking)
                switch updater.phase {
                case .checking:
                    Text("Checking...").font(.caption).foregroundStyle(Theme.secondary)
                case .upToDate:
                    Text("You have the latest version.").font(.caption).foregroundStyle(Theme.secondary)
                case .available(let version):
                    Text("Version \(version) is out").font(.callout).foregroundStyle(Theme.primaryText)
                    MacUpdateActions()
                case .downloading(let fraction):
                    MacUpdateProgress(fraction: fraction)
                case .downloaded(let name):
                    Text("\(name) is in your Downloads. \(MacUpdater.savedNote)").font(.caption).foregroundStyle(Theme.secondary)
                case .idle:
                    if let notice = updater.notice { MacUpdateNoticeView(notice: notice) }
                }
                if let name = updater.signedInName {
                    Text("Signed in to Lakeside as \(name)").font(.caption).foregroundStyle(Theme.secondary)
                    Button("Sign out of Lakeside") { updater.signOut() }.tint(Theme.gold)
                }
            }
        }
    }
}

/// Settings when the Lakeside launcher started the game: the launcher owns updates, and an account row
/// only when the handoff worked.
@MainActor
struct LauncherManagedPanel: View {
    private let updater = MacUpdater.shared

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Eyebrow(text: "Updates")
                Text("The Lakeside launcher keeps this copy up to date."
                    + (LauncherSession.current.launcherVersion.isEmpty ? "" : " Launcher \(LauncherSession.current.launcherVersion)."))
                    .font(.caption).foregroundStyle(Theme.secondary)
                if let account = updater.launcherAccount {
                    Text("Signed in as \(account.label)").font(.callout).foregroundStyle(Theme.primaryText)
                    Text("From the Lakeside launcher. Your game works the same without it.").font(.caption).foregroundStyle(Theme.secondary)
                }
            }
        }
    }
}
