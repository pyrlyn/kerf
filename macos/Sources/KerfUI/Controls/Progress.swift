// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// The UI Kit's 16 pt spinner: a half-circle accent arc turning over a track.
/// It turns only while it is in a window and animating, and stands still
/// when the user asked macOS to reduce motion.
public final class KerfSpinner: NSView {
  public static let side: CGFloat = 16
  /// One turn per second, as the system's spinning indicator.
  static let turnsPerSecond: CGFloat = 1

  public var isAnimating = false { didSet { updateDisplayLink() } }
  /// The arc's rotation in radians, clockwise from twelve o'clock.
  var angle: CGFloat = 0 { didSet { needsDisplay = true } }
  private var link: CADisplayLink?

  public init() {
    super.init(frame: NSRect(x: 0, y: 0, width: Self.side, height: Self.side))
    setAccessibilityElement(true)
    setAccessibilityRole(.progressIndicator)
    setAccessibilityLabel("In progress")
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }
  public override var intrinsicContentSize: NSSize { NSSize(width: Self.side, height: Self.side) }

  public override func viewDidMoveToWindow() {
    super.viewDidMoveToWindow()
    updateDisplayLink()
  }

  var isTurning: Bool { link != nil }

  private func updateDisplayLink() {
    let turn =
      isAnimating && window != nil
      && !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
    if turn, link == nil {
      link = displayLink(target: self, selector: #selector(step(_:)))
      link?.add(to: .main, forMode: .common)
    } else if !turn {
      link?.invalidate()
      link = nil
    }
  }

  @objc private func step(_ link: CADisplayLink) {
    let turns = link.targetTimestamp * Self.turnsPerSecond
    angle = (turns - turns.rounded(.down)) * 2 * .pi
  }

  public override func draw(_ dirtyRect: NSRect) {
    // Figma strokes the 14 pt circle 2 pt inside: a 6 pt radius centre line.
    let center = NSPoint(x: bounds.midX, y: bounds.midY)
    let track = NSBezierPath(
      ovalIn: NSRect(x: center.x - 6, y: center.y - 6, width: 12, height: 12))
    track.lineWidth = 2
    KerfColor.Background.fillStrong.setStroke()
    track.stroke()
    // In this flipped view, degrees run clockwise from three o'clock; the
    // arc spans 12 to 6 o'clock before turning.
    let start = -90 + angle * 180 / .pi
    let arc = NSBezierPath()
    arc.appendArc(withCenter: center, radius: 6, startAngle: start, endAngle: start + 180)
    (arc.lineWidth, arc.lineCapStyle) = (2, .round)
    KerfColor.Accent.default.setStroke()
    arc.stroke()
  }
}

/// The UI Kit's 4 pt progress bar.
public final class KerfProgressBar: NSView {
  /// Progress from 0 to 1; values outside are clamped.
  public var value: Double = 0 {
    didSet {
      value = min(max(value, 0), 1)
      needsDisplay = true
      setAccessibilityValue(NSNumber(value: value))
    }
  }

  public init(value: Double = 0, width: CGFloat = 240) {
    super.init(frame: NSRect(x: 0, y: 0, width: width, height: 4))
    setAccessibilityElement(true)
    setAccessibilityRole(.progressIndicator)
    setAccessibilityMinValue(NSNumber(value: 0))
    setAccessibilityMaxValue(NSNumber(value: 1))
    defer { self.value = value }
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }
  public override var intrinsicContentSize: NSSize {
    NSSize(width: NSView.noIntrinsicMetric, height: 4)
  }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.box(bounds, radius: KerfRadius.full, fill: KerfColor.Background.fillMedium)
    guard value > 0 else { return }
    var filled = bounds
    filled.size.width = max((bounds.width * value).rounded(), bounds.height)
    Drawing.box(filled, radius: KerfRadius.full, fill: KerfColor.Accent.default)
  }
}
