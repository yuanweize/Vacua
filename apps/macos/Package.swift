// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "VacuaApp",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .executable(name: "Vacua", targets: ["Vacua"])
    ],
    dependencies: [
        .package(path: "Packages/VacuaClient")
    ],
    targets: [
        .executableTarget(
            name: "Vacua",
            dependencies: [
                .product(name: "VacuaClient", package: "VacuaClient")
            ],
            path: "Vacua",
            exclude: [
                "Resources/Info.plist"
            ],
            resources: [
                .process("Resources/Assets.xcassets"),
                .process("Resources/Localizable.xcstrings")
            ]
        )
    ]
)
