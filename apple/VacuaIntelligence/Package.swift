// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "VacuaIntelligence",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .executable(name: "vacua-intelligence", targets: ["VacuaIntelligence"])
    ],
    targets: [
        .executableTarget(
            name: "VacuaIntelligence",
            path: "Sources/VacuaIntelligence"
        )
    ]
)
