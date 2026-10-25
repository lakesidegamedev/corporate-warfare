import XCTest

final class AppFlowUITests: XCTestCase {
    func testStartCareerStepWeekAndVisitEveryTab() {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing"]
        app.launch()

        XCTAssertTrue(app.staticTexts["Corporate Warfare"].waitForExistence(timeout: 15))
        let start = app.buttons["Start"]
        XCTAssertTrue(start.waitForExistence(timeout: 10))
        start.tap()

        let step = app.buttons["Step week"]
        XCTAssertTrue(step.waitForExistence(timeout: 30), "the portfolio loads after starting")
        step.tap()
        XCTAssertTrue(app.tabBars.buttons["Portfolio"].waitForExistence(timeout: 15))

        for tab in ["Company", "Regions", "Markets", "Inbox", "Portfolio"] {
            let button = app.tabBars.buttons[tab]
            XCTAssertTrue(button.waitForExistence(timeout: 10), "\(tab) tab exists")
            button.tap()
        }
        XCTAssertTrue(app.buttons["Step week"].waitForExistence(timeout: 10))
    }
}
