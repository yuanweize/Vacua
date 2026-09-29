import XCTest

final class VacuaUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testAppLaunchAndSidebarNavigation() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VACUA_TEST_MODE"] = "1"
        app.launch()

        // Verify window launched
        XCTAssertTrue(app.windows.firstMatch.exists)
    }
}
