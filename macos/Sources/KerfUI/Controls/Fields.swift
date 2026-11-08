// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// The UI Kit's 28 pt text field: a filled, outlined box with an optional
/// leading icon around a borderless `NSTextField`, so editing, undo, input
/// methods and accessibility stay AppKit's. While it has focus the outline
/// turns accent, in place of the system's rectangular focus ring.
public final class KerfTextField: NSView {
  public let field: NSTextField
  public var icon: KerfIcon? { didSet { needsLayout = true } }

  public var stringValue: String {
    get { field.stringValue }
    set { field.stringValue = newValue }
  }

  public init(placeholder: String, icon: KerfIcon? = nil, width: CGFloat = 240) {
    field = FocusField(string: "")
    self.icon = icon
    super.init(frame: NSRect(x: 0, y: 0, width: width, height: 28))
    let style = KerfTextStyle.body
    (field.isBordered, field.drawsBackground, field.focusRingType) = (false, false, .none)
    (field.font, field.textColor) = (style.font, KerfColor.Text.primary)
    field.placeholderAttributedString = style.string(placeholder, color: KerfColor.Text.secondary)
    field.setAccessibilityLabel(placeholder)
    field.cell?.isScrollable = true
    addSubview(field)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }
  public override var intrinsicContentSize: NSSize {
    NSSize(width: NSView.noIntrinsicMetric, height: 28)
  }

  var isFocused: Bool {
    guard let editor = window?.firstResponder as? NSText else { return false }
    return editor.delegate === field
  }

  public override func layout() {
    super.layout()
    let x = KerfSpace.lg + (icon == nil ? 0 : KerfIcon.size + KerfSpace.md)
    let height = field.intrinsicContentSize.height
    field.frame = NSRect(
      x: x, y: ((bounds.height - height) / 2).rounded(), width: bounds.width - x - KerfSpace.lg,
      height: height)
  }

  public override func draw(_ dirtyRect: NSRect) {
    let outline = isFocused ? KerfColor.Accent.ink : KerfColor.Border.subtle
    Drawing.box(bounds, radius: KerfRadius.md, fill: KerfColor.Background.fill, stroke: outline)
    if let icon {
      Drawing.icon(icon, color: KerfColor.Text.secondary, x: KerfSpace.lg + 1, in: bounds)
    }
  }

  public override func mouseDown(with event: NSEvent) {
    window?.makeFirstResponder(field)
  }
}

/// Redraws the surrounding box when focus arrives or leaves.
private final class FocusField: NSTextField {
  override func becomeFirstResponder() -> Bool {
    defer { superview?.needsDisplay = true }
    return super.becomeFirstResponder()
  }

  override func textDidEndEditing(_ notification: Notification) {
    super.textDidEndEditing(notification)
    superview?.needsDisplay = true
  }
}

/// The UI Kit's select: an `NSPopUpButton` drawn as a 28 pt filled box with
/// up and down chevrons. The menu, keyboard and accessibility are AppKit's.
/// It is as wide as its longest item, so choosing does not resize it.
public final class KerfSelect: NSPopUpButton {
  public init(items: [String], target: AnyObject? = nil, action: Selector? = nil) {
    super.init(frame: .zero, pullsDown: false)
    addItems(withTitles: items)
    (self.target, self.action, isBordered) = (target, action, false)
    setFrameSize(intrinsicContentSize)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }
  /// The box is drawn edge to edge: no inset for AppKit's bezel shadow.
  public override var alignmentRectInsets: NSEdgeInsets { NSEdgeInsets() }

  public override var intrinsicContentSize: NSSize {
    let text = itemTitles.map { Drawing.width($0, .body) }.max() ?? 0
    return NSSize(width: KerfSpace.lg + text + KerfSpace.md + 10 + KerfSpace.md, height: 28)
  }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.enabled(isEnabled) {
      Drawing.box(
        bounds, radius: KerfRadius.md, fill: KerfColor.Background.fill,
        stroke: KerfColor.Border.subtle)
      Drawing.label(
        titleOfSelectedItem ?? "", .body, color: KerfColor.Text.primary, x: KerfSpace.lg + 1,
        in: bounds)
      // Figma's chevrons: a 6 × 10 pair in a 10 × 14 box, 8 pt from the end.
      let origin = NSPoint(x: bounds.width - KerfSpace.md - 10 + 2, y: 7 + 2)
      let chevrons = NSBezierPath()
      for (tip, base) in [(0.0, 3.0), (10.0, 7.0)] {
        chevrons.move(to: NSPoint(x: origin.x, y: origin.y + base))
        chevrons.line(to: NSPoint(x: origin.x + 3, y: origin.y + tip))
        chevrons.line(to: NSPoint(x: origin.x + 6, y: origin.y + base))
      }
      (chevrons.lineWidth, chevrons.lineCapStyle, chevrons.lineJoinStyle) = (1.2, .round, .round)
      KerfColor.Text.secondary.setStroke()
      chevrons.stroke()
    }
  }
}
