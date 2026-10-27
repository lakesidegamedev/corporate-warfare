import XCTest
@testable import CorporateWarfare

final class FormatTests: XCTestCase {
    override func tearDown() {
        Format.reset()
        super.tearDown()
    }

    func testSingleMoneyKeepsDollars() {
        Format.reset()
        XCTAssertEqual(Format.money(123_456), "$1,235")
        XCTAssertEqual(Format.money(-123_456), "-$1,235")
        XCTAssertEqual(Format.price(1_235), "$12.35")
        XCTAssertEqual(Format.moneyCompact(120_000_000), "$1.2M")
        XCTAssertEqual(Format.moneyCompact(-2_100_000), "-$21K")
        XCTAssertEqual(Format.signedMoney(500), "+$5")
        XCTAssertEqual(Format.money(123_456, currency: ""), "$1,235")
    }

    func testMultiLetterSymbolsGetAThinGap() {
        Format.configure(reporting: "DM", symbols: ["DM": "DM", "NLG": "f", "BEF": "BF"], numeraire: "DM")
        XCTAssertEqual(Format.money(123_456), "DM\u{2009}1,235")
        XCTAssertEqual(Format.money(-123_456), "-DM\u{2009}1,235")
        XCTAssertEqual(Format.moneyCompact(120_000_000), "DM\u{2009}1.2M")
        XCTAssertEqual(Format.price(1_235), "DM\u{2009}12.35")
    }

    func testCompanyFiguresUseTheirOwnCurrency() {
        Format.configure(reporting: "DM", symbols: ["DM": "DM", "NLG": "f", "BEF": "BF"], numeraire: "DM")
        XCTAssertEqual(Format.moneyCompact(120_000_000, currency: "NLG"), "f1.2M")
        XCTAssertEqual(Format.money(123_456, currency: "BEF"), "BF\u{2009}1,235")
        XCTAssertEqual(Format.symbol(of: "XXX"), "DM")
        XCTAssertEqual(Format.rate(1_850_000), "1.8500")
    }
}
