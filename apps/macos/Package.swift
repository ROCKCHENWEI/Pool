// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Pool",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "Pool", targets: ["Pool"])
    ],
    dependencies: [],
    targets: [
        .executableTarget(
            name: "Pool",
            dependencies: ["PoolCore"]
        ),
        .target(
            name: "PoolCore",
            dependencies: [],
            path: "Sources/PoolCore"
        )
    ]
)
