// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// Shapes and text placement the controls share, so each control states only
/// its colours and metrics.
enum Drawing {
  /// A rounded rectangle; `radius` is clamped, so `KerfRadius.full` is a pill.
  static func rounded(_ rect: NSRect, radius: CGFloat) -> NSBezierPath {
    let r = min(radius, rect.height / 2, rect.width / 2)
    return NSBezierPath(roundedRect: rect, xRadius: r, yRadius: r)
  }

  /// Fills and strokes `rect` the way Figma does: the stroke sits inside.
  static func box(
    _ rect: NSRect, radius: CGFloat, fill: NSColor?, stroke: NSColor? = nil, lineWidth: CGFloat = 1
  ) {
    if let fill {
      fill.setFill()
      rounded(rect, radius: radius).fill()
    }
    if let stroke {
      let inset = lineWidth / 2
      let path = rounded(rect.insetBy(dx: inset, dy: inset), radius: radius - inset)
      path.lineWidth = lineWidth
      stroke.setStroke()
      path.stroke()
    }
  }

  /// The width `text` takes in `style`, rounded up to whole points as
  /// Figma's hug layout does.
  static func width(_ text: String, _ style: KerfTextStyle) -> CGFloat {
    ceil(style.string(text, color: .black).size().width)
  }

  /// Draws `text` at `x`, centred vertically in `rect` by the style's line
  /// height, as Figma's centre-aligned auto layout places a label.
  static func label(
    _ text: String, _ style: KerfTextStyle, color: NSColor, x: CGFloat, in rect: NSRect
  ) {
    let line = NSRect(
      x: x, y: rect.minY + ((rect.height - style.lineHeight) / 2).rounded(),
      width: rect.maxX - x, height: style.lineHeight)
    style.string(text, color: color).draw(with: line, options: [.usesLineFragmentOrigin])
  }

  /// Draws a 16 pt icon at `x`, centred vertically in `rect`.
  static func icon(_ icon: KerfIcon, color: NSColor, x: CGFloat, in rect: NSRect) {
    let y = rect.minY + ((rect.height - KerfIcon.size) / 2).rounded()
    icon.image(color: color).draw(
      in: NSRect(x: x, y: y, width: KerfIcon.size, height: KerfIcon.size))
  }

  /// Runs `body` faded when the control is disabled.
  static func enabled(_ isEnabled: Bool, _ body: () -> Void) {
    guard !isEnabled, let context = NSGraphicsContext.current?.cgContext else { return body() }
    context.saveGState()
    context.setAlpha(0.4)
    context.beginTransparencyLayer(auxiliaryInfo: nil)
    body()
    context.endTransparencyLayer()
    context.restoreGState()
  }
}
