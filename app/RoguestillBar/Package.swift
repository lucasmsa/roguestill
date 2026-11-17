// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "RoguestillBar",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "RoguestillKit", targets: ["RoguestillKit"]),
        .executable(name: "RoguestillBar", targets: ["RoguestillBar"])
    ],
    targets: [
        .target(name: "RoguestillKit"),
        .executableTarget(name: "RoguestillBar", dependencies: ["RoguestillKit"], resources: [.copy("Resources")]),
        .testTarget(name: "RoguestillKitTests", dependencies: ["RoguestillKit"])
    ]
)
