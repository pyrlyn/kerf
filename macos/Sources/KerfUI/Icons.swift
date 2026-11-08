// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// The UI Kit's 16 pt stroke icons. The path data is Figma's SVG export
/// rounded to 0.01 pt; it is drawn in code because `swift build` does not
/// compile asset catalogs, and so every icon takes any token colour.
public enum KerfIcon: String, CaseIterable, Sendable {
  case navigator
  case inspector
  case plus
  case folder
  case search
  case issue
  case branch
  case bug
  case terminal
  case plugin
  case shield
  case check
  case close
  case split
  case trash
  case chevronUp = "chevron-up"
  case panel
  case server
  case clone
  case gear
  case text
  case sparkle
  case lock
  case `continue`
  case stepOver = "step-over"
  case stepInto = "step-into"
  case stepOut = "step-out"
  case stop
  case play
  case document
  case eye
  case key
  case chevronLeft = "chevron-left"
  case chevronRight = "chevron-right"
  case code
  case globe
  case phone
  case compass
  case package
  case refresh
  case pulse
  case copy

  public static let size: CGFloat = 16
  public static let lineWidth: CGFloat = 1.2

  /// The icon as a 16 pt image drawn in `color`, sharp at any backing scale.
  public func image(color: NSColor = KerfColor.Text.secondary) -> NSImage {
    let image = NSImage(size: NSSize(width: Self.size, height: Self.size), flipped: true) { _ in
      color.setStroke()
      self.path.stroke()
      return true
    }
    image.accessibilityDescription = rawValue
    return image
  }

  /// The outline in a flipped 16 pt square, styled as the Figma strokes are.
  public var path: NSBezierPath {
    let path = NSBezierPath(svgPath: pathData)
    path.lineWidth = Self.lineWidth
    path.lineCapStyle = .round
    path.lineJoinStyle = .round
    return path
  }

  var pathData: String {
    switch self {
    case .navigator:
      """
      M12 3.33 H4 C2.9 3.33 2 4.23 2 5.33 V10.67 C2 11.77 2.9 12.67 4 12.67 H12 \
      C13.1 12.67 14 11.77 14 10.67 V5.33 C14 4.23 13.1 3.33 12 3.33 Z M6 3.33 V12.67
      """
    case .inspector:
      """
      M12 3.33 H4 C2.9 3.33 2 4.23 2 5.33 V10.67 C2 11.77 2.9 12.67 4 12.67 H12 \
      C13.1 12.67 14 11.77 14 10.67 V5.33 C14 4.23 13.1 3.33 12 3.33 Z M10 3.33 V12.67
      """
    case .plus: "M8 3.33V12.67M3.33 8H12.67"
    case .folder:
      """
      M2 4.67 C2 4.31 2.14 3.97 2.39 3.72 C2.64 3.47 2.98 3.33 3.33 3.33 H6 L7.33 4.67 H12.67 \
      C13.02 4.67 13.36 4.81 13.61 5.06 C13.86 5.31 14 5.65 14 6 V11.33 \
      C14 11.69 13.86 12.03 13.61 12.28 C13.36 12.53 13.02 12.67 12.67 12.67 H3.33 \
      C2.98 12.67 2.64 12.53 2.39 12.28 C2.14 12.03 2 11.69 2 11.33 V4.67 Z
      """
    case .search:
      """
      M7.33 11.33 C9.54 11.33 11.33 9.54 11.33 7.33 C11.33 5.12 9.54 3.33 7.33 3.33 \
      C5.12 3.33 3.33 5.12 3.33 7.33 C3.33 9.54 5.12 11.33 7.33 11.33 Z M13.33 13.33 L10.33 10.33
      """
    case .issue: "M8 2.67L14 13.33H2L8 2.67Z M8 6.67V9.33"
    case .branch:
      """
      M4 5.67 C4.92 5.67 5.67 4.92 5.67 4 C5.67 3.08 4.92 2.33 4 2.33 C3.08 2.33 2.33 3.08 2.33 4 \
      C2.33 4.92 3.08 5.67 4 5.67 Z M4 13.67 C4.92 13.67 5.67 12.92 5.67 12 \
      C5.67 11.08 4.92 10.33 4 10.33 C3.08 10.33 2.33 11.08 2.33 12 C2.33 12.92 3.08 13.67 4 13.67 \
      Z M12 7.67 C12.92 7.67 13.67 6.92 13.67 6 C13.67 5.08 12.92 4.33 12 4.33 \
      C11.08 4.33 10.33 5.08 10.33 6 C10.33 6.92 11.08 7.67 12 7.67 Z M4 5.67 V10.33 M12 7.67 \
      C12 9.67 9.33 10 5.67 11
      """
    case .bug:
      """
      M11.33 8.67 C11.33 6.83 9.84 5.33 8 5.33 C6.16 5.33 4.67 6.83 4.67 8.67 V10 \
      C4.67 11.84 6.16 13.33 8 13.33 C9.84 13.33 11.33 11.84 11.33 10 V8.67 Z M8 5.33 V3.33 \
      M6 3.33 L7 4.67 M10 3.33 L9 4.67 M2.67 8.67 H4.67 M11.33 8.67 H13.33 M3.33 12.67 L5 11.33 \
      M12.67 12.67 L11 11.33
      """
    case .terminal: "M3.33 4.67L6.67 8L3.33 11.33M8 12H12.67"
    case .plugin:
      """
      M6 2.67 H8.67 V4.67 C8.67 5.02 8.81 5.36 9.06 5.61 C9.31 5.86 9.65 6 10 6 \
      C10.35 6 10.69 5.86 10.94 5.61 C11.19 5.36 11.33 5.02 11.33 4.67 V2.67 H13.33 V6.67 H11.33 \
      C10.98 6.67 10.64 6.81 10.39 7.06 C10.14 7.31 10 7.65 10 8 C10 8.35 10.14 8.69 10.39 8.94 \
      C10.64 9.19 10.98 9.33 11.33 9.33 H13.33 V13.33 H2.67 V9.33 H4.67 \
      C5.02 9.33 5.36 9.19 5.61 8.94 C5.86 8.69 6 8.35 6 8 C6 7.65 5.86 7.31 5.61 7.06 \
      C5.36 6.81 5.02 6.67 4.67 6.67 H2.67 V2.67 H6 Z
      """
    case .shield: "M8 2L13.33 4V8C13.33 11 11.07 13.2 8 14C4.93 13.2 2.67 11 2.67 8V4L8 2Z"
    case .check: "M3.33 8.33L6.33 11.33L12.67 5"
    case .close: "M4 4L12 12M12 4L4 12"
    case .split:
      """
      M12 3.33 H4 C2.9 3.33 2 4.23 2 5.33 V10.67 C2 11.77 2.9 12.67 4 12.67 H12 \
      C13.1 12.67 14 11.77 14 10.67 V5.33 C14 4.23 13.1 3.33 12 3.33 Z M8 3.33 V12.67
      """
    case .trash: "M2.67 4.67H13.33M6.67 2.67H9.33M4 4.67L4.67 13.33H11.33L12 4.67"
    case .chevronUp: "M4 10L8 6L12 10"
    case .panel:
      """
      M12 3.33 H4 C2.9 3.33 2 4.23 2 5.33 V10.67 C2 11.77 2.9 12.67 4 12.67 H12 \
      C13.1 12.67 14 11.77 14 10.67 V5.33 C14 4.23 13.1 3.33 12 3.33 Z M2 9.33 H14
      """
    case .server:
      """
      M12 2.67 H4 C3.26 2.67 2.67 3.26 2.67 4 V6 C2.67 6.74 3.26 7.33 4 7.33 H12 \
      C12.74 7.33 13.33 6.74 13.33 6 V4 C13.33 3.26 12.74 2.67 12 2.67 Z M12 8.67 H4 \
      C3.26 8.67 2.67 9.26 2.67 10 V12 C2.67 12.74 3.26 13.33 4 13.33 H12 \
      C12.74 13.33 13.33 12.74 13.33 12 V10 C13.33 9.26 12.74 8.67 12 8.67 Z M5.33 5 H5.34 \
      M5.33 11 H5.34
      """
    case .clone:
      """
      M8 2.67 V9.33 M10.67 6.67 L8 9.33 L5.33 6.67 M3.33 10.67 V12 \
      C3.33 12.35 3.47 12.69 3.72 12.94 C3.97 13.19 4.31 13.33 4.67 13.33 H11.33 \
      C11.69 13.33 12.03 13.19 12.28 12.94 C12.53 12.69 12.67 12.35 12.67 12 V10.67
      """
    case .gear:
      """
      M8 10 C9.1 10 10 9.1 10 8 C10 6.9 9.1 6 8 6 C6.9 6 6 6.9 6 8 C6 9.1 6.9 10 8 10 Z M8 2 V4 \
      M8 12 V14 M2 8 H4 M12 8 H14 M3.73 3.73 L5.13 5.13 M10.87 10.87 L12.27 12.27 M3.73 12.27 \
      L5.13 10.87 M10.87 5.13 L12.27 3.73
      """
    case .text: "M3.33 4H12.67M8 4V12.67M6 12.67H10"
    case .sparkle:
      """
      M8 2 V4.67 M8 11.33 V14 M2 8 H4.67 M11.33 8 H14 M4 4 L5.67 5.67 M10.33 10.33 L12 12 M4 12 \
      L5.67 10.33 M10.33 5.67 L12 4
      """
    case .lock:
      """
      M11.33 7.33 H4.67 C3.93 7.33 3.33 7.93 3.33 8.67 V12 C3.33 12.74 3.93 13.33 4.67 13.33 \
      H11.33 C12.07 13.33 12.67 12.74 12.67 12 V8.67 C12.67 7.93 12.07 7.33 11.33 7.33 Z \
      M5.33 7.33 V5.33 C5.33 4.63 5.61 3.95 6.11 3.45 C6.61 2.95 7.29 2.67 8 2.67 \
      C8.71 2.67 9.39 2.95 9.89 3.45 C10.39 3.95 10.67 4.63 10.67 5.33 V7.33
      """
    case .continue: "M4 3.33V12.67M6.67 3.33L12.67 8L6.67 12.67V3.33Z"
    case .stepOver:
      """
      M2.67 9.33 C2.78 8.2 3.26 7.13 4.02 6.28 C4.79 5.44 5.81 4.86 6.93 4.64 \
      C8.05 4.41 9.21 4.55 10.24 5.04 C11.27 5.52 12.12 6.33 12.67 7.33 M8.67 7.33 H12.67 V3.33 \
      M7.33 12.67 H8.67
      """
    case .stepInto: "M8 2.67V10M11.33 6.67L8 10L4.67 6.67M5.33 13.33H10.67"
    case .stepOut: "M8 13.33V6M11.33 9.33L8 6L4.67 9.33M5.33 2.67H10.67"
    case .stop:
      """
      M10.67 4 H5.33 C4.6 4 4 4.6 4 5.33 V10.67 C4 11.4 4.6 12 5.33 12 H10.67 \
      C11.4 12 12 11.4 12 10.67 V5.33 C12 4.6 11.4 4 10.67 4 Z
      """
    case .play: "M4.67 3V13L12.67 8L4.67 3Z"
    case .document: "M4.67 2H9.33L12.67 5.33V14H4.67V2Z M9.33 2V5.33H12.67"
    case .eye:
      """
      M1.33 8 C1.33 8 3.73 4 8 4 C12.27 4 14.67 8 14.67 8 C14.67 8 12.27 12 8 12 \
      C3.73 12 1.33 8 1.33 8 Z M8 9.67 C8.92 9.67 9.67 8.92 9.67 8 C9.67 7.08 8.92 6.33 8 6.33 \
      C7.08 6.33 6.33 7.08 6.33 8 C6.33 8.92 7.08 9.67 8 9.67 Z
      """
    case .key:
      """
      M5.33 12.67 C6.81 12.67 8 11.47 8 10 C8 8.53 6.81 7.33 5.33 7.33 \
      C3.86 7.33 2.67 8.53 2.67 10 C2.67 11.47 3.86 12.67 5.33 12.67 Z M7.33 8 L12.67 2.67 \
      M10.67 4.67 L12 6
      """
    case .chevronLeft: "M10 12L6 8L10 4"
    case .chevronRight: "M6 12L10 8L6 4"
    case .code: "M5.33 6L2.67 8L5.33 10M10.67 6L13.33 8L10.67 10M9.33 3.33L6.67 12.67"
    case .globe:
      """
      M8 14 C11.31 14 14 11.31 14 8 C14 4.69 11.31 2 8 2 C4.69 2 2 4.69 2 8 C2 11.31 4.69 14 8 14 \
      Z M2 8 H14 M8 2 C9.41 3.68 10.18 5.81 10.18 8 C10.18 10.19 9.41 12.32 8 14 \
      C6.59 12.32 5.82 10.19 5.82 8 C5.82 5.81 6.59 3.68 8 2 Z
      """
    case .phone:
      """
      M10 2 H6 C4.9 2 4 2.9 4 4 V12 C4 13.1 4.9 14 6 14 H10 C11.1 14 12 13.1 12 12 V4 \
      C12 2.9 11.1 2 10 2 Z M7.33 12 H8.67
      """
    case .compass:
      """
      M8 14 C11.31 14 14 11.31 14 8 C14 4.69 11.31 2 8 2 C4.69 2 2 4.69 2 8 C2 11.31 4.69 14 8 14 \
      Z M10.33 5.67 L9 9 L5.67 10.33 L7 7 L10.33 5.67 Z
      """
    case .package:
      """
      M14 5.33 L8 2 L2 5.33 V10.67 L8 14 L14 10.67 V5.33 Z M2 5.33 L8 8.67 M8 8.67 L14 5.33 \
      M8 8.67 V14
      """
    case .refresh:
      """
      M13.33 8 C13.32 9.23 12.88 10.42 12.09 11.37 C11.3 12.31 10.2 12.96 8.99 13.18 \
      C7.78 13.41 6.52 13.21 5.44 12.62 C4.36 12.03 3.52 11.08 3.05 9.94 \
      C2.59 8.79 2.54 7.53 2.91 6.35 C3.28 5.17 4.04 4.16 5.07 3.48 C6.11 2.81 7.34 2.51 8.57 2.64 \
      C9.79 2.77 10.93 3.32 11.8 4.2 M13.33 2.67 V6 H10
      """
    case .pulse: "M2 8H4.67L6.67 2.67L9.33 13.33L11.33 8H14"
    case .copy:
      """
      M12.33 6 H7.67 C6.75 6 6 6.75 6 7.67 V12.33 C6 13.25 6.75 14 7.67 14 H12.33 \
      C13.25 14 14 13.25 14 12.33 V7.67 C14 6.75 13.25 6 12.33 6 Z M3.33 10 V3.33 \
      C3.33 2.98 3.47 2.64 3.72 2.39 C3.97 2.14 4.31 2 4.67 2 H11.33
      """
    }
  }
}

extension NSBezierPath {
  /// Absolute `M L H V C Z` path data, the subset Figma's export writes.
  convenience init(svgPath: String) {
    self.init()
    var command: Character = "M"
    var numbers: [CGFloat] = []
    var current = NSPoint.zero
    func take(_ count: Int) {
      while count > 0, numbers.count >= count {
        let n = Array(numbers.prefix(count))
        numbers.removeFirst(count)
        switch command {
        case "M":
          current = NSPoint(x: n[0], y: n[1])
          move(to: current)
          command = "L"
        case "L":
          current = NSPoint(x: n[0], y: n[1])
          line(to: current)
        case "H":
          current.x = n[0]
          line(to: current)
        case "V":
          current.y = n[0]
          line(to: current)
        case "C":
          current = NSPoint(x: n[4], y: n[5])
          curve(
            to: current, controlPoint1: NSPoint(x: n[0], y: n[1]),
            controlPoint2: NSPoint(x: n[2], y: n[3]))
        default: return
        }
      }
    }
    let arity: [Character: Int] = ["M": 2, "L": 2, "H": 1, "V": 1, "C": 6]
    var token = ""
    func flush() {
      if let v = Double(token) { numbers.append(CGFloat(v)) }
      token = ""
      take(arity[command] ?? 0)
    }
    for ch in svgPath {
      if ch.isLetter {
        flush()
        command = ch
        if ch == "Z" { close() }
      } else if ch == " " || ch == "," || (ch == "-" && !token.isEmpty) {
        flush()
        if ch == "-" { token = "-" }
      } else {
        token.append(ch)
      }
    }
    flush()
  }
}
