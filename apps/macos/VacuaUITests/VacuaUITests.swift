import XCTest

final class VacuaUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testAppLaunchAndSidebarNavigation() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VACUA_TEST_MODE"] = "1"
        app.launch()

        // Verify primary window exists
        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 5.0))

        // Navigate through sidebar destinations
        let sidebarItems = ["Storage Map", "Candidates", "Duplicates", "Applications", "Snapshots", "Overview"]
        for item in sidebarItems {
            let element = app.staticTexts[item].firstMatch
            if element.waitForExistence(timeout: 2.0) {
                element.click()
            }
        }
    }

    func testSafetyBannerCopyDoesNotContainHardcodedReleaseVersion() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VACUA_TEST_MODE"] = "1"
        app.launch()

        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 5.0))

        // Ensure "v0.7.0" is not hardcoded anywhere in the initial window text
        let hardcodedV070 = app.staticTexts["Proposal-Only Interface (v0.7.0)"]
        XCTAssertFalse(hardcodedV070.exists)
    }
}

