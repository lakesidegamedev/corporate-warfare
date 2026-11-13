import CryptoKit
import XCTest
@testable import CorporateWarfare

/// Answers every request from a closure, so no test touches the network.
final class StubProtocol: URLProtocol {
    enum Reply {
        case http(Int, [String: String], Data)
        case redirect(String)
        case fail(URLError.Code)
    }

    private static let lock = NSLock()
    private static var handler: ((URLRequest) -> Reply)?
    private static var log: [URLRequest] = []

    static func install(_ handler: @escaping (URLRequest) -> Reply) {
        lock.lock()
        self.handler = handler
        log = []
        lock.unlock()
    }

    static func reset() {
        lock.lock()
        handler = nil
        log = []
        lock.unlock()
    }

    static var seen: [URLRequest] {
        lock.lock()
        defer { lock.unlock() }
        return log
    }

    static var configuration: URLSessionConfiguration {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubProtocol.self]
        return config
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lock.lock()
        Self.log.append(request)
        let handler = Self.handler
        Self.lock.unlock()
        guard let url = request.url, let reply = handler?(request) else {
            client?.urlProtocol(self, didFailWithError: URLError(.unknown))
            return
        }
        switch reply {
        case .http(let status, let headers, let body):
            let response = HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: body)
            client?.urlProtocolDidFinishLoading(self)
        case .redirect(let to):
            let response = HTTPURLResponse(url: url, statusCode: 302, httpVersion: "HTTP/1.1", headerFields: ["Location": to])!
            client?.urlProtocol(self, wasRedirectedTo: URLRequest(url: URL(string: to)!), redirectResponse: response)
        case .fail(let code):
            client?.urlProtocol(self, didFailWithError: URLError(code))
        }
    }

    override func stopLoading() {}
}

final class MemoryTokenStore: TokenStore {
    var value: String?
    func load() -> String? { value }
    func save(_ secret: String) -> Bool { value = secret; return true }
    func delete() { value = nil }
}

final class MacUpdateTests: XCTestCase {
    private let storage = "https://abc123.r2.cloudflarestorage.com/corporate-warfare/CorporateWarfare.dmg?sig=x"
    private var directory: URL!

    override func setUp() {
        super.setUp()
        directory = FileManager.default.temporaryDirectory.appendingPathComponent("cw-mac-update-\(UUID().uuidString)")
    }

    override func tearDown() {
        StubProtocol.reset()
        try? FileManager.default.removeItem(at: directory)
        super.tearDown()
    }

    private func downloads() -> LakesideDownloads {
        LakesideDownloads(base: "https://lakesidegames.net", configuration: StubProtocol.configuration)
    }

    private func hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private func file(for data: Data, sha: String? = nil) -> DownloadFile {
        DownloadFile(platform: "macos-universal", name: "CorporateWarfare.dmg", size: Int64(data.count),
                     sha256: sha ?? hex(data), url: "/account/launcher/download/corporate-warfare/macos-universal")
    }

    private func serve(_ body: Data) {
        StubProtocol.install { [storage] request in
            if request.url?.host == "lakesidegames.net" { return .redirect(storage) }
            return .http(200, [:], body)
        }
    }

    func testEntitledDownloadIsVerifiedAndSaved() async throws {
        let body = Data("pretend disk image".utf8)
        serve(body)
        let saved = try await downloads().download(file(for: body), token: "tok", to: directory) { _ in }
        XCTAssertEqual(try Data(contentsOf: saved), body)
        XCTAssertEqual(saved.lastPathComponent, "CorporateWarfare.dmg")
        XCTAssertEqual(StubProtocol.seen.first?.value(forHTTPHeaderField: "Authorization"), "Bearer tok")
    }

    func testRedirectToStorageDropsTheAuthorizationHeader() async throws {
        let body = Data("bytes".utf8)
        serve(body)
        _ = try await downloads().download(file(for: body), token: "tok", to: directory) { _ in }
        let storageRequests = StubProtocol.seen.filter { $0.url?.host?.hasSuffix("r2.cloudflarestorage.com") == true }
        XCTAssertFalse(storageRequests.isEmpty)
        for request in storageRequests {
            XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"))
        }
    }

    func testHashMismatchIsRefusedAndNothingIsKept() async {
        let body = Data("tampered".utf8)
        serve(body)
        let wrong = String(repeating: "0", count: 64)
        do {
            _ = try await downloads().download(file(for: body, sha: wrong), token: "tok", to: directory) { _ in }
            XCTFail("a mismatch must be refused")
        } catch {
            XCTAssertEqual(error as? UpdateError, .mismatch)
        }
        let kept = (try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? []
        XCTAssertTrue(kept.isEmpty)
    }

    func testMissingChecksumIsRefused() async {
        let body = Data("x".utf8)
        serve(body)
        var f = file(for: body)
        f.sha256 = nil
        do {
            _ = try await downloads().download(f, token: "tok", to: directory) { _ in }
            XCTFail("no checksum must be refused")
        } catch {
            XCTAssertEqual(error as? UpdateError, .noChecksum)
        }
    }

    func testForbiddenMeansTheAccountDoesNotOwnTheGame() async {
        StubProtocol.install { _ in .http(403, [:], Data()) }
        let body = Data("x".utf8)
        do {
            _ = try await downloads().download(file(for: body), token: "tok", to: directory) { _ in }
            XCTFail("403 must throw")
        } catch {
            XCTAssertEqual(error as? UpdateError, .notEntitled)
        }
        XCTAssertTrue(UpdateNotice.make(.notEntitled).offersBuy)
    }

    func testListingNotEntitledAndMacFile() async throws {
        let json = """
        {"product":"corporate-warfare","version":"2.0.2","entitled":true,"files":[
        {"platform":"windows-x86_64","name":"a.exe","size":1,"sha256":"aa","installer":"inno","url":"/w"},
        {"platform":"macos-universal","name":"CorporateWarfare.dmg","size":9,"sha256":"bb","installer":"dmg","url":"/m"}]}
        """
        StubProtocol.install { _ in .http(200, [:], Data(json.utf8)) }
        let listing = try await downloads().listing(token: "tok")
        XCTAssertTrue(listing.entitled)
        XCTAssertEqual(listing.version, "2.0.2")
        XCTAssertEqual(listing.macFile?.url, "/m")
        StubProtocol.install { _ in .http(200, [:], Data("{\"entitled\":false,\"version\":null,\"files\":[]}".utf8)) }
        let none = try await downloads().listing(token: "tok")
        XCTAssertFalse(none.entitled)
        XCTAssertNil(none.macFile)
    }

    func testOfflineIsReportedAsOffline() async {
        StubProtocol.install { _ in .fail(.notConnectedToInternet) }
        do {
            _ = try await downloads().listing(token: "tok")
            XCTFail("offline must throw")
        } catch {
            XCTAssertEqual(error as? UpdateError, .offline)
        }
        let body = Data("x".utf8)
        do {
            _ = try await downloads().download(file(for: body), token: "tok", to: directory) { _ in }
            XCTFail("offline must throw")
        } catch {
            XCTAssertEqual(error as? UpdateError, .offline)
        }
    }

    func testStatusMessages() {
        XCTAssertEqual(LakesideDownloads.error(forStatus: 401), .unauthorized)
        XCTAssertEqual(LakesideDownloads.error(forStatus: 404), .noBuild)
        XCTAssertEqual(LakesideDownloads.error(forStatus: 429), .rateLimited)
        XCTAssertNil(LakesideDownloads.error(forStatus: 200))
        XCTAssertTrue(UpdateNotice.make(.unauthorized).offersSignIn)
    }

    func testVersionComparison() {
        XCTAssertTrue(AppVersion.isNewer("v2.0.2", than: "2.0.1"))
        XCTAssertTrue(AppVersion.isNewer("2.1", than: "2.0.9"))
        XCTAssertFalse(AppVersion.isNewer("2.0.1", than: "2.0.1"))
        XCTAssertFalse(AppVersion.isNewer("1.9.9", than: "2.0.0"))
    }

    func testSignInExchangeKeepsTheTokenAndNeverRetries() async {
        StubProtocol.install { _ in
            .http(200, [:], Data("{\"token\":\"secret-token\",\"session\":{\"username\":\"Ada\"}}".utf8))
        }
        let store = MemoryTokenStore()
        let auth = LakesideSignIn(store: store, configuration: StubProtocol.configuration)
        let result = await auth.exchange(code: "one-time")
        XCTAssertEqual(result, .done("Ada"))
        XCTAssertEqual(auth.token(), "secret-token")
        XCTAssertEqual(auth.name, "Ada")
        XCTAssertNotNil(store.value)
        auth.signOut()
        XCTAssertNil(auth.token())
        XCTAssertNil(store.value)
        StubProtocol.install { _ in .http(429, [:], Data()) }
        let limited = await auth.exchange(code: "again")
        XCTAssertEqual(StubProtocol.seen.count, 1)
        if case .failed = limited {} else { XCTFail("429 must fail") }
    }

    func testCallbackNeedsTheRightStateAndPath() {
        XCTAssertEqual(LoopbackListener.judge(target: "/callback?code=abc&state=s1", state: "s1").code, "abc")
        XCTAssertNil(LoopbackListener.judge(target: "/callback?code=abc&state=other", state: "s1").code)
        XCTAssertTrue(LoopbackListener.judge(target: "/elsewhere", state: "s1").notFound)
        XCTAssertEqual(LakesideSignIn.randomState().count, 32)
    }

    func testLauncherEnvironmentIsReadOnlyWhenManaged() {
        let managed = LauncherSession(environment: ["LAKESIDE_LAUNCHER": "1", "LAKESIDE_LAUNCHER_VERSION": "3.1"])
        XCTAssertTrue(managed.managed)
        XCTAssertEqual(managed.launcherVersion, "3.1")
        let plain = LauncherSession(environment: ["LAKESIDE_HANDOFF_CODE": "x"])
        XCTAssertFalse(plain.managed)
    }
}
