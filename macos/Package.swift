// swift-tools-version: 6.2
// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

// The macOS host: a C module over kerf.h, the editor view in KerfKit, and
// the AppKit app. Build the Rust library first: `cargo build -p kerf-ffi`.

import Foundation
import PackageDescription

// SwiftPM cannot build Cargo crates, so the package links the static library
// Cargo leaves in the workspace target folder. Unsafe flags are fine for a
// root package.
let rustStaticLib = URL(fileURLWithPath: #filePath)
  .deletingLastPathComponent()
  .appendingPathComponent("../target/debug/libkerf_ffi.a")
  .standardizedFileURL.path

let package = Package(
  name: "Kerf",
  // NSTextInsertionIndicator, the system caret, is macOS 14+.
  platforms: [.macOS(.v14)],
  products: [
    .executable(name: "Kerf", targets: ["Kerf"])
  ],
  targets: [
    .systemLibrary(name: "CKerf", path: "Sources/CKerf"),
    .target(
      name: "KerfKit",
      dependencies: ["CKerf"],
      linkerSettings: [
        .unsafeFlags([rustStaticLib]),
        // What Slint's text and image stack (fontique) calls into.
        .linkedFramework("AppKit"),
        .linkedFramework("CoreFoundation"),
        .linkedFramework("CoreGraphics"),
        .linkedFramework("CoreText"),
      ]
    ),
    .executableTarget(name: "Kerf", dependencies: ["KerfKit"]),
    .testTarget(name: "KerfKitTests", dependencies: ["KerfKit", "CKerf"]),
  ]
)
