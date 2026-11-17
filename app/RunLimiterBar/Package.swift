// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "RunLimiterBar",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "RunLimiterKit", targets: ["RunLimiterKit"]),
        .executable(name: "RunLimiterBar", targets: ["RunLimiterBar"])
    ],
    targets: [
        .target(name: "RunLimiterKit"),
        .executableTarget(name: "RunLimiterBar", dependencies: ["RunLimiterKit"]),
        .testTarget(name: "RunLimiterKitTests", dependencies: ["RunLimiterKit"])
    ]
)
