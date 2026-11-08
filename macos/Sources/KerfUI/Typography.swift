// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// A text style of the Figma UI Kit. Figma draws Inter and Roboto Mono; the
/// app uses the system's SF Pro and SF Mono at the same size, weight and line
/// height, so it ships no font files and follows the system's rendering.
public struct KerfTextStyle: Sendable {
  public let size: CGFloat
  public let weight: NSFont.Weight
  public let lineHeight: CGFloat
  /// Letter spacing in points.
  public let tracking: CGFloat
  public let monospaced: Bool
  public let uppercase: Bool

  public var font: NSFont {
    monospaced
      ? .monospacedSystemFont(ofSize: size, weight: weight)
      : .systemFont(ofSize: size, weight: weight)
  }

  /// Attributes that draw this style in `color`, its line height fixed as in
  /// Figma so rows line up with the mockups.
  public func attributes(color: NSColor) -> [NSAttributedString.Key: Any] {
    let paragraph = NSMutableParagraphStyle()
    paragraph.minimumLineHeight = lineHeight
    paragraph.maximumLineHeight = lineHeight
    paragraph.lineBreakMode = .byTruncatingTail
    return [.font: font, .foregroundColor: color, .kern: tracking, .paragraphStyle: paragraph]
  }

  /// `text` in this style; upper-cases it where the style does.
  public func string(_ text: String, color: NSColor) -> NSAttributedString {
    NSAttributedString(
      string: uppercase ? text.uppercased() : text, attributes: attributes(color: color))
  }

  init(
    _ size: CGFloat, _ weight: NSFont.Weight, line: CGFloat, tracking: CGFloat = 0,
    monospaced: Bool = false, uppercase: Bool = false
  ) {
    (self.size, self.weight, lineHeight) = (size, weight, line)
    (self.tracking, self.monospaced, self.uppercase) = (tracking, monospaced, uppercase)
  }

  public static let largeTitle = KerfTextStyle(26, .bold, line: 32, tracking: -0.5)
  public static let title = KerfTextStyle(17, .semibold, line: 22, tracking: -0.1)
  public static let body = KerfTextStyle(13, .regular, line: 18)
  public static let bodyEmphasized = KerfTextStyle(13, .semibold, line: 18)
  public static let small = KerfTextStyle(12, .regular, line: 16)
  public static let smallEmphasized = KerfTextStyle(12, .semibold, line: 16)
  public static let caption = KerfTextStyle(11, .regular, line: 14)
  public static let captionBold = KerfTextStyle(11, .bold, line: 14)
  public static let sectionLabel = KerfTextStyle(
    11, .semibold, line: 14, tracking: 0.6, uppercase: true)
  public static let code = KerfTextStyle(12, .regular, line: 18, monospaced: true)
  public static let terminal = KerfTextStyle(12, .regular, line: 17, monospaced: true)
  public static let codeSmall = KerfTextStyle(10, .regular, line: 12, monospaced: true)
}
