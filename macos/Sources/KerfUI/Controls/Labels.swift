// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// The status colours chips and status lines share.
public enum KerfTone: CaseIterable, Sendable {
  case ok, warning, error, busy, neutral, accent

  public var ink: NSColor {
    switch self {
    case .ok: KerfColor.Status.ok
    case .warning: KerfColor.Status.warning
    case .error: KerfColor.Status.error
    case .busy: KerfColor.Status.busy
    case .neutral: KerfColor.Text.tertiary
    case .accent: KerfColor.Accent.ink
    }
  }

  public var background: NSColor {
    switch self {
    case .ok: KerfColor.Status.okBackground
    case .warning: KerfColor.Status.warningBackground
    case .error: KerfColor.Status.errorBackground
    case .busy: KerfColor.Status.busyBackground
    case .neutral: KerfColor.Background.fillMedium
    case .accent: KerfColor.Background.selection
    }
  }
}

/// A short read-only text in a box: sized by its text and read by VoiceOver
/// as static text. Subclasses give the metrics and draw the box.
public class KerfLabelView: NSView {
  public var text: String {
    didSet {
      invalidateIntrinsicContentSize()
      setFrameSize(intrinsicContentSize)
      needsDisplay = true
    }
  }

  var style: KerfTextStyle { .caption }
  var height: CGFloat { 18 }
  var padding: CGFloat { KerfSpace.sm }
  /// Width before the text: a dot and its gap.
  var lead: CGFloat { 0 }
  var minimumWidth: CGFloat { 0 }

  init(text: String) {
    self.text = text
    super.init(frame: .zero)
    setFrameSize(intrinsicContentSize)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }

  public override var intrinsicContentSize: NSSize {
    let width = padding * 2 + lead + Drawing.width(text, style)
    return NSSize(width: max(width, minimumWidth), height: height)
  }

  public override func isAccessibilityElement() -> Bool { true }
  public override func accessibilityRole() -> NSAccessibility.Role? { .staticText }
  public override func accessibilityValue() -> Any? { text }

  /// Draws a `size` dot at `x`, centred vertically.
  func dot(_ color: NSColor, size: CGFloat, x: CGFloat) {
    color.setFill()
    let y = ((bounds.height - size) / 2).rounded()
    NSBezierPath(ovalIn: NSRect(x: x, y: y, width: size, height: size)).fill()
  }

  /// Draws the text after the padding and lead, or centred when the view is
  /// wider than its text (a badge at its minimum width).
  func drawText(_ color: NSColor) {
    let natural = padding + lead
    let x = max(natural, ((bounds.width - Drawing.width(text, style)) / 2).rounded())
    Drawing.label(text, style, color: color, x: lead > 0 ? natural : x, in: bounds)
  }
}

/// An 18 pt tinted pill with a dot: a state next to a name.
public final class KerfChip: KerfLabelView {
  public var tone: KerfTone { didSet { needsDisplay = true } }

  public init(_ text: String, tone: KerfTone) {
    self.tone = tone
    super.init(text: text)
  }

  override var style: KerfTextStyle { .captionBold }
  override var padding: CGFloat { KerfSpace.md }
  override var lead: CGFloat { 6 + KerfSpace.xs }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.box(bounds, radius: KerfRadius.full, fill: tone.background)
    dot(tone.ink, size: 6, x: padding)
    drawText(tone.ink)
  }
}

/// A status line: a coloured dot and secondary text, as in the status bar.
public final class KerfStatus: KerfLabelView {
  public var tone: KerfTone { didSet { needsDisplay = true } }

  public init(_ text: String, tone: KerfTone) {
    self.tone = tone
    super.init(text: text)
  }

  override var height: CGFloat { 16 }
  override var padding: CGFloat { 0 }
  override var lead: CGFloat { 7 + KerfSpace.sm }

  public override func draw(_ dirtyRect: NSRect) {
    dot(tone.ink, size: 7, x: 0)
    drawText(KerfColor.Text.secondary)
  }
}

/// A keyboard shortcut, such as ⌘⇧O, in a key cap.
public final class KerfKey: KerfLabelView {
  public init(_ keys: String) { super.init(text: keys) }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.box(bounds, radius: KerfRadius.sm, fill: KerfColor.Background.fillMedium)
    drawText(KerfColor.Text.tertiary)
  }
}

/// A count in a 16 pt pill, as on a tab or a list section.
public final class KerfCountBadge: KerfLabelView {
  public enum Tone: CaseIterable, Sendable { case accent, neutral, error }

  public var tone: Tone { didSet { needsDisplay = true } }
  public var count: Int {
    get { Int(text) ?? 0 }
    set { text = String(newValue) }
  }

  public init(_ count: Int, tone: Tone = .neutral) {
    self.tone = tone
    super.init(text: String(count))
  }

  override var style: KerfTextStyle { .captionBold }
  override var height: CGFloat { 16 }
  override var minimumWidth: CGFloat { 16 }

  public override func draw(_ dirtyRect: NSRect) {
    let (fill, ink): (NSColor, NSColor) =
      switch tone {
      case .accent: (KerfColor.Accent.default, KerfColor.Text.onAccent)
      case .neutral: (KerfColor.Background.fillStrong, KerfColor.Text.secondary)
      case .error: (KerfColor.Status.error, KerfColor.Text.onAccent)
      }
    Drawing.box(bounds, radius: KerfRadius.full, fill: fill)
    drawText(ink)
  }
}

/// A person's initials in a 24 pt accent circle.
public final class KerfAvatar: KerfLabelView {
  public init(name: String) {
    let initials = name.split(separator: " ").prefix(2).compactMap(\.first)
    super.init(text: String(initials).uppercased())
    setAccessibilityLabel(name)
  }

  override var style: KerfTextStyle { .captionBold }
  override var height: CGFloat { 24 }
  override var minimumWidth: CGFloat { 24 }
  public override var intrinsicContentSize: NSSize { NSSize(width: 24, height: 24) }

  public override func draw(_ dirtyRect: NSRect) {
    KerfColor.Accent.default.setFill()
    NSBezierPath(ovalIn: bounds).fill()
    drawText(KerfColor.Text.onAccent)
  }
}
