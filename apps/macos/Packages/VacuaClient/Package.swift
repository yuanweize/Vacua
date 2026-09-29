// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "VacuaClient",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .library(
            name: "VacuaClient",
            targets: ["VacuaClient"]
        ),
    ],
    dependencies: [],
    targets: [
        .target(
            name: "VacuaClient",
            dependencies: [],
            path: "Sources/VacuaClient",
            swiftSettings: [
                .enableUpcomingFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "VacuaClientTests",
            dependencies: ["VacuaClient"],
            path: "Tests/VacuaClientTests",
            resources: []
        ),
    ]
)
