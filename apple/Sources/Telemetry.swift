import Foundation

/// Anonymous market-balance analytics. Every five game weeks the engine's snapshot (goods made,
/// sold, imported, exported, held and priced; corridor health; the player's company results)
/// is posted to the Lakeside ops ingest with a random install id. No names, account or device
/// data. On by default; the player can switch it off in Settings. Uploads never block play and
/// a failed upload is dropped, not retried.
enum Telemetry {
    static let endpoint = URL(string: "https://ops.lakesidegames.net/api/cw/telemetry")!
    private static let enabledKey = "cw.telemetry.enabled"
    private static let installKey = "cw.telemetry.installId"

    /// Test runs (CI's simulator and snapshot tests) never upload: their careers are not play.
    private static var underTest: Bool {
        ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
            || NSClassFromString("XCTestCase") != nil
    }

    static var enabled: Bool {
        get { !underTest && (UserDefaults.standard.object(forKey: enabledKey) as? Bool ?? true) }
        set { UserDefaults.standard.set(newValue, forKey: enabledKey) }
    }

    /// A random id for this install, not tied to any account or device identifier.
    static var installId: String {
        if let id = UserDefaults.standard.string(forKey: installKey) { return id }
        let id = UUID().uuidString.lowercased()
        UserDefaults.standard.set(id, forKey: installKey)
        return id
    }

    /// True on the weeks a snapshot is due.
    static func due(week: UInt32) -> Bool { week > 0 && week % 5 == 0 }

    /// True when an advance from `from` to `to` crossed a five-week boundary, however many weeks it took.
    static func crossed(from: UInt32, to: UInt32) -> Bool { to / 5 > from / 5 }

    /// Post one snapshot in the background.
    static func send(snapshotJSON: String) {
        guard enabled, !snapshotJSON.isEmpty else { return }
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? ""
        let build = info?["CFBundleVersion"] as? String ?? ""
        let envelope = "{\"install_id\":\"\(installId)\",\"app_version\":\"\(version)\",\"build\":\"\(build)\",\"snapshot\":\(snapshotJSON)}"
        var request = URLRequest(url: endpoint, timeoutInterval: 10)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = Data(envelope.utf8)
        URLSession.shared.dataTask(with: request) { _, _, _ in }.resume()
    }
}
