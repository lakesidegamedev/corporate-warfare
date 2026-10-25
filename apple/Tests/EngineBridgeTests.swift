import XCTest
@testable import CorporateWarfare

final class EngineBridgeTests: XCTestCase {
    func testGeneratedBindingsLoadTheRustLibrary() {
        let info = engineInfo()
        XCTAssertEqual(info.interfaceVersion, 1)
        XCTAssertEqual(info.version, "0.1.0")
    }
}
