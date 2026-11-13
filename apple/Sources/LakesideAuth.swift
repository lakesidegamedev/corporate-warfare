import Foundation
import Network
import Security
import UIKit

/// How a Lakeside sign-in ended. The text is plain words for the player and never holds the token or the code.
enum SignInResult: Equatable {
    case done(String)
    case failed(String)
    case cancelled
}

/// Signing in to Lakeside so the in-game updater can download the paid Mac build. Everything the
/// updater needs from sign-in sits behind this, so a later Keycloak flow replaces one class. The game
/// never needs it to play (docs/rules/LAUNCHER.md). Mirrors LakesideAuth.kt.
protocol LakesideAuth: AnyObject {
    /// Who is signed in, or nil.
    var name: String? { get }
    /// Load a saved sign-in from the Keychain, if any.
    func restore()
    /// Open the browser and wait for the player to finish.
    func signIn(provider: String) async -> SignInResult
    /// Stop a sign-in that is waiting on the browser.
    func cancelSignIn()
    /// The session token, or nil when signed out or past its 30 days. Never log it.
    func token() -> String?
    /// Forget the sign-in here: removes it from the Keychain and from memory.
    func signOut()
}

enum LakesideProvider {
    static let ahd = "ahd"
    static let discord = "discord"
}

/// Where the sign-in secret is kept between runs: the Keychain, never a plain file.
protocol TokenStore {
    func load() -> String?
    /// False when this Mac has no secure store that works; the sign-in then lasts this session only.
    func save(_ secret: String) -> Bool
    func delete()
}

/// No secure store: nothing is kept past the session.
struct NoTokenStore: TokenStore {
    func load() -> String? { nil }
    func save(_ secret: String) -> Bool { false }
    func delete() {}
}

/// The Keychain: one generic password item for this app.
struct KeychainTokenStore: TokenStore {
    var service = "net.lakesidegames.corporatewarfare.lakeside"
    var account = "session"

    private var base: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }

    func load() -> String? {
        var query = base
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func save(_ secret: String) -> Bool {
        delete()
        var query = base
        query[kSecValueData as String] = Data(secret.utf8)
        query[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        return SecItemAdd(query as CFDictionary, nil) == errSecSuccess
    }

    func delete() {
        SecItemDelete(base as CFDictionary)
    }
}

/// Answers every redirect with "do not follow": the sign-in and handoff calls go to one fixed address.
final class NoRedirects: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }
}

/// The one-time listener on this Mac that the browser hands the sign-in code to. Bound to 127.0.0.1
/// only, one port out of the accepted range.
final class LoopbackListener {
    enum Outcome {
        case code(String)
        case cancelled
        case timedOut
    }

    private let listener: NWListener
    let port: Int
    private let state: String
    private let queue = DispatchQueue(label: "net.lakesidegames.corporatewarfare.signin")
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Outcome, Never>?
    private var outcome: Outcome?

    private init(listener: NWListener, port: Int, state: String) {
        self.listener = listener
        self.port = port
        self.state = state
    }

    /// Listen on the first free port of the range, or nil when none is free.
    static func open(ports: ClosedRange<Int>, state: String) async -> LoopbackListener? {
        for port in ports {
            guard let number = NWEndpoint.Port(rawValue: UInt16(port)) else { continue }
            let parameters = NWParameters.tcp
            parameters.requiredLocalEndpoint = NWEndpoint.hostPort(host: .ipv4(.loopback), port: number)
            guard let listener = try? NWListener(using: parameters) else { continue }
            let box = LoopbackListener(listener: listener, port: port, state: state)
            if await box.start() { return box }
            listener.cancel()
        }
        return nil
    }

    private func start() async -> Bool {
        await withCheckedContinuation { (resume: CheckedContinuation<Bool, Never>) in
            let once = ResumeOnce(resume)
            listener.stateUpdateHandler = { newState in
                switch newState {
                case .ready: once.fire(true)
                case .failed, .cancelled: once.fire(false)
                default: break
                }
            }
            listener.newConnectionHandler = { [weak self] connection in self?.handle(connection) }
            listener.start(queue: queue)
        }
    }

    /// Wait for the browser to come back, up to `seconds`.
    func wait(seconds: Double) async -> Outcome {
        await withCheckedContinuation { (resume: CheckedContinuation<Outcome, Never>) in
            lock.lock()
            if let done = outcome {
                lock.unlock()
                resume.resume(returning: done)
                return
            }
            continuation = resume
            lock.unlock()
            queue.asyncAfter(deadline: .now() + seconds) { [weak self] in self?.finish(.timedOut) }
        }
    }

    func cancel() { finish(.cancelled) }

    func close() {
        listener.stateUpdateHandler = nil
        listener.newConnectionHandler = nil
        listener.cancel()
    }

    private func finish(_ result: Outcome) {
        lock.lock()
        if outcome != nil {
            lock.unlock()
            return
        }
        outcome = result
        let waiting = continuation
        continuation = nil
        lock.unlock()
        waiting?.resume(returning: result)
    }

    private func handle(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 4096) { [weak self] data, _, _, _ in
            guard let self else {
                connection.cancel()
                return
            }
            let head = data.flatMap { String(data: $0, encoding: .utf8) ?? String(data: $0, encoding: .isoLatin1) } ?? ""
            let line = head.split(separator: "\r\n", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? ""
            let parts = line.split(separator: " ")
            let target = parts.count >= 2 ? String(parts[1]) : ""
            let verdict = Self.judge(target: target, state: self.state)
            self.reply(connection, status: verdict.code == nil ? (verdict.notFound ? 404 : 400) : 200, text: verdict.text)
            if let code = verdict.code { self.finish(.code(code)) }
        }
    }

    /// Check one request target against the expected state. Pure, so it can be tested.
    static func judge(target: String, state: String) -> (code: String?, notFound: Bool, text: String) {
        guard let parsed = URLComponents(string: "http://127.0.0.1" + target), parsed.path == "/callback" else {
            return (nil, true, "Not found.")
        }
        let items = parsed.queryItems ?? []
        let code = items.first { $0.name == "code" }?.value ?? ""
        let given = items.first { $0.name == "state" }?.value ?? ""
        if given != state || code.isEmpty || code.count > 2048 {
            return (nil, false, "This sign-in link does not match. Go back to Corporate Warfare and try again.")
        }
        return (code, false, "You are signed in to Lakeside. You can close this window and go back to Corporate Warfare.")
    }

    private func reply(_ connection: NWConnection, status: Int, text: String) {
        let body = "<!doctype html><meta charset=utf-8><title>Corporate Warfare</title>"
            + "<body style=\"font-family:sans-serif;background:#111;color:#eee;text-align:center;padding-top:20vh\"><p>\(text)</p>"
        let bytes = Data(body.utf8)
        let head = "HTTP/1.1 \(status) \(status == 200 ? "OK" : "Error")\r\nContent-Type: text/html; charset=utf-8\r\n"
            + "Content-Length: \(bytes.count)\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n"
        var out = Data(head.utf8)
        out.append(bytes)
        connection.send(content: out, contentContext: .defaultMessage, isComplete: true,
                        completion: .contentProcessed { _ in connection.cancel() })
    }
}

/// Resumes a continuation once, whatever order the listener's state callbacks arrive in.
private final class ResumeOnce {
    private let lock = NSLock()
    private var resume: CheckedContinuation<Bool, Never>?

    init(_ resume: CheckedContinuation<Bool, Never>) { self.resume = resume }

    func fire(_ value: Bool) {
        lock.lock()
        let pending = resume
        resume = nil
        lock.unlock()
        pending?.resume(returning: value)
    }
}

/// The loopback sign-in of the Lakeside account service: the system browser signs the player in and
/// hands a one-time code back to a port on this Mac, and the code is exchanged at once for a session
/// token that lasts 30 days. There is no refresh token; after 30 days the player signs in again.
final class LakesideSignIn: LakesideAuth {
    static let accountBase = "https://lakesidegames.net"
    static let ports = 47832...47841
    static let lifetime: TimeInterval = 30 * 24 * 60 * 60
    static let fallbackName = "your Lakeside account"

    private struct Saved: Codable {
        var token: String
        var name: String
        var savedAt: TimeInterval
    }

    private let openBrowser: (URL) async -> Bool
    private let store: TokenStore
    private let base: String
    private let ports: ClosedRange<Int>
    private let clock: () -> Date
    private let waitSeconds: Double
    private let session: URLSession
    private let lock = NSLock()
    private var saved: Saved?
    private var loaded = false
    private var listening: LoopbackListener?

    init(openBrowser: @escaping (URL) async -> Bool = { await LakesideSignIn.systemBrowser($0) },
         store: TokenStore = KeychainTokenStore(),
         base: String = LakesideSignIn.accountBase,
         ports: ClosedRange<Int> = LakesideSignIn.ports,
         configuration: URLSessionConfiguration = .ephemeral,
         clock: @escaping () -> Date = Date.init,
         waitSeconds: Double = 300) {
        self.openBrowser = openBrowser
        self.store = store
        self.base = base
        self.ports = ports
        self.clock = clock
        self.waitSeconds = waitSeconds
        self.session = URLSession(configuration: configuration, delegate: NoRedirects(), delegateQueue: nil)
    }

    /// Opens the default browser. On Mac Catalyst this is UIApplication, not NSWorkspace.
    @MainActor
    static func systemBrowser(_ url: URL) async -> Bool {
        await UIApplication.shared.open(url)
    }

    var name: String? {
        lock.lock()
        defer { lock.unlock() }
        return saved.map { $0.name.isEmpty ? Self.fallbackName : $0.name }
    }

    func restore() {
        lock.lock()
        defer { lock.unlock() }
        if loaded { return }
        loaded = true
        guard let text = store.load(), let data = text.data(using: .utf8),
              let found = try? JSONDecoder().decode(Saved.self, from: data) else { return }
        if found.token.isEmpty || clock().timeIntervalSince1970 - found.savedAt > Self.lifetime {
            store.delete()
            return
        }
        saved = found
    }

    func token() -> String? {
        restore()
        lock.lock()
        let current = saved
        lock.unlock()
        guard let current else { return nil }
        if clock().timeIntervalSince1970 - current.savedAt > Self.lifetime {
            signOut()
            return nil
        }
        return current.token
    }

    func signOut() {
        lock.lock()
        saved = nil
        loaded = true
        lock.unlock()
        store.delete()
    }

    func cancelSignIn() {
        lock.lock()
        let active = listening
        lock.unlock()
        active?.cancel()
    }

    func signIn(provider: String) async -> SignInResult {
        let state = Self.randomState()
        guard let listener = await LoopbackListener.open(ports: ports, state: state) else {
            return .failed("Could not open a local port for the sign-in. Close other Lakeside apps and try again.")
        }
        lock.lock()
        listening = listener
        lock.unlock()
        defer {
            lock.lock()
            listening = nil
            lock.unlock()
            listener.close()
        }
        var parts = URLComponents(string: base + "/account/launcher/start")
        parts?.queryItems = [
            URLQueryItem(name: "provider", value: provider),
            URLQueryItem(name: "port", value: String(listener.port)),
            URLQueryItem(name: "state", value: state),
        ]
        guard let url = parts?.url, await openBrowser(url) else {
            return .failed("Could not open your browser. Open \(base)/store/corporate-warfare/ and sign in there, or try again.")
        }
        switch await listener.wait(seconds: waitSeconds) {
        case .code(let code): return await exchange(code: code)
        case .cancelled: return .cancelled
        case .timedOut: return .failed("The sign-in was not finished in time. Try again.")
        }
    }

    /// Exchange the one-time code at once. One attempt, no retry: a code is single use, and 429 means wait.
    func exchange(code: String) async -> SignInResult {
        guard let url = URL(string: base + "/account/launcher/session") else { return .failed("Lakeside sign-in failed. Try again later.") }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 15
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.httpBody = try? JSONSerialization.data(withJSONObject: ["code": code])
        do {
            let (data, response) = try await session.data(for: request)
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            switch status {
            case 200:
                guard data.count <= 16 * 1024,
                      let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                      let token = (object["token"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !token.isEmpty else {
                    return .failed("Lakeside sent back something unexpected. Try again.")
                }
                let info = object["session"] as? [String: Any]
                let raw = (info?["username"] as? String) ?? ""
                let cleaned = String(raw.unicodeScalars.filter { !CharacterSet.controlCharacters.contains($0) }.map(Character.init))
                let who = String(cleaned.trimmingCharacters(in: .whitespacesAndNewlines).prefix(40))
                let shown = who.isEmpty ? Self.fallbackName : who
                keep(Saved(token: token, name: shown, savedAt: clock().timeIntervalSince1970))
                return .done(shown)
            case 400: return .failed("That sign-in link expired. Try again.")
            case 401: return .failed("Lakeside did not accept the sign-in. Try again.")
            case 429: return .failed("Too many sign-in attempts. Wait a few minutes and try again.")
            case 502, 503: return .failed("Lakeside sign-in is not available right now. Try again later.")
            default: return .failed("Lakeside sign-in failed (error \(status)). Try again later.")
            }
        } catch {
            return .failed("Could not reach Lakeside to finish signing in.")
        }
    }

    private func keep(_ value: Saved) {
        lock.lock()
        saved = value
        loaded = true
        lock.unlock()
        // No working Keychain means the sign-in lasts this session only; that is the promise.
        if let data = try? JSONEncoder().encode(value), let text = String(data: data, encoding: .utf8) {
            _ = store.save(text)
        }
    }

    static func randomState() -> String {
        let alphabet = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_")
        var generator = SystemRandomNumberGenerator()
        return String((0..<32).map { _ in alphabet[Int(generator.next() % UInt64(alphabet.count))] })
    }
}

// MARK: - Lakeside launcher handoff

/// The Lakeside account behind a launcher handoff. Kept in memory for the session only.
struct LakesideAccount: Equatable {
    let accountId: String
    let displayName: String
    var label: String { displayName.isEmpty ? "your Lakeside account" : displayName }
}

/// Starting the game from the Lakeside launcher (docs/rules/LAUNCHER.md). The launcher sets
/// LAKESIDE_LAUNCHER=1 and LAKESIDE_LAUNCHER_VERSION, and LAKESIDE_HANDOFF_CODE only when the player
/// is signed in. The game never checks ownership and never needs this to play.
final class LauncherSession {
    static let current = LauncherSession()

    /// True when the launcher started this copy, so the launcher owns its updates.
    let managed: Bool
    let launcherVersion: String
    private let lock = NSLock()
    private var code: String?
    private let session: URLSession
    private let redeemURL: URL

    init(environment: [String: String] = ProcessInfo.processInfo.environment,
         configuration: URLSessionConfiguration = .ephemeral,
         redeemURL: URL = URL(string: "https://lakesidegames.net/account/launcher/handoff/redeem")!) {
        let isManaged = environment["LAKESIDE_LAUNCHER"] == "1"
        managed = isManaged
        launcherVersion = isManaged ? String((environment["LAKESIDE_LAUNCHER_VERSION"] ?? "").trimmingCharacters(in: .whitespaces).prefix(32)) : ""
        let given = isManaged ? (environment["LAKESIDE_HANDOFF_CODE"] ?? "").trimmingCharacters(in: .whitespaces) : ""
        code = given.isEmpty ? nil : given
        let config = configuration
        config.timeoutIntervalForRequest = 5
        config.timeoutIntervalForResource = 5
        session = URLSession(configuration: config, delegate: NoRedirects(), delegateQueue: nil)
        self.redeemURL = redeemURL
    }

    /// Redeem the handoff code, once, off the main actor. The code is forgotten as soon as it is
    /// taken, so a second call never sends it again. Any failure is "not signed in". Never logged.
    func redeemOnce() async -> LakesideAccount? {
        lock.lock()
        let taken = code
        code = nil
        lock.unlock()
        guard let taken else { return nil }
        var request = URLRequest(url: redeemURL)
        request.httpMethod = "POST"
        request.timeoutInterval = 5
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.httpBody = try? JSONSerialization.data(withJSONObject: ["code": taken, "clientId": "corporate-warfare-desktop"])
        guard let (data, response) = try? await session.data(for: request),
              (response as? HTTPURLResponse)?.statusCode == 200, data.count <= 16 * 1024,
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        guard let id = (object["accountId"] as? String)?.trimmingCharacters(in: .whitespaces), !id.isEmpty, id.count <= 128 else { return nil }
        let raw = (object["displayName"] as? String) ?? ""
        let cleaned = String(raw.unicodeScalars.filter { !CharacterSet.controlCharacters.contains($0) }.map(Character.init))
        return LakesideAccount(accountId: id, displayName: String(cleaned.trimmingCharacters(in: .whitespaces).prefix(40)))
    }
}
