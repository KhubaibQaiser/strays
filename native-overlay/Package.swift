// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "StraysOverlay",
    platforms: [.macOS("13.1")],
    dependencies: [
        .package(url: "https://github.com/rive-app/rive-ios", exact: "6.28.1"),
    ],
    targets: [
        .executableTarget(
            name: "StraysOverlay",
            dependencies: [.product(name: "RiveRuntime", package: "rive-ios")],
            resources: [.copy("Resources/cat.riv")]
        ),
    ]
)
