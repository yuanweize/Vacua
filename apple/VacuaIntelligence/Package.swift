// swift-tools-version: 6.0
import PackageDescription
import Foundation

var swiftSettings: [SwiftSetting] = []
let xcodePluginPath = "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/usr/lib/swift/host/plugins"
if FileManager.default.fileExists(atPath: xcodePluginPath) {
    swiftSettings.append(.unsafeFlags(["-plugin-path", xcodePluginPath]))
}

let package = Package(
    name: "VacuaIntelligence",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .executable(name: "vacua-intelligence", targets: ["VacuaIntelligence"]),
        .executable(name: "foundation-models-proof", targets: ["FoundationModelsProof"])
    ],
    targets: [
        .executableTarget(
            name: "VacuaIntelligence",
            path: "Sources/VacuaIntelligence",
            swiftSettings: swiftSettings
        ),
        .executableTarget(
            name: "FoundationModelsProof",
            path: "Sources/FoundationModelsProof"
        )
    ]
)
