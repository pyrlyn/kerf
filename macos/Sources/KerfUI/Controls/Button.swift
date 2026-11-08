// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// The UI Kit's pill button. An `NSButton` underneath, so target, action,
/// key equivalents and accessibility work as for any Mac button; only the
/// drawing is Kerf's.
public final class KerfButton: NSButton {
  public enum Style: CaseIterable, Sendable {
    case primary, secondary, destructive, destructiveOutline, ghost
  }

  public enum Size: CaseIterable, Sendable {
    case small, regular, large

    var height: CGFloat { [24, 28, 36][index] }
    var padding: CGFloat { [KerfSpace.lg, KerfSpace.xl2, KerfSpace.xl4][index] }
    var text: KerfTextStyle { self == .small ? .smallEmphasized : .bodyEmphasized }
    private var index: Int { Self.allCases.firstIndex(of: self)! }
  }

  public var style: Style { didSet { needsDisplay = true } }
  public var size: Size { didSet { invalidateIntrinsicContentSize() } }
  public var icon: KerfIcon? { didSet { invalidateIntrinsicContentSize() } }

  public init(
    title: String, style: Style = .secondary, size: Size = .regular, icon: KerfIcon? = nil,
    target: AnyObject? = nil, action: Selector? = nil
  ) {
    (self.style, self.size, self.icon) = (style, size, icon)
    super.init(frame: .zero)
    (self.title, self.target, self.action) = (title, target, action)
    isBordered = false
    setFrameSize(intrinsicContentSize)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }
  public override var title: String { didSet { invalidateIntrinsicContentSize() } }

  /// Outlined styles are 2 pt wider: Figma counts their 1 pt stroke in the
  /// hug width.
  private var stroke: NSColor? {
    switch style {
    case .destructiveOutline: KerfColor.Status.error
    case .ghost: KerfColor.Border.strong
    default: nil
    }
  }

  private var fill: NSColor? {
    switch style {
    case .primary: KerfColor.Accent.default
    case .secondary, .destructive: KerfColor.Background.fillStrong
    case .destructiveOutline, .ghost: nil
    }
  }

  private var ink: NSColor {
    switch style {
    case .primary: KerfColor.Text.onAccent
    case .secondary, .ghost: KerfColor.Text.primary
    case .destructive, .destructiveOutline: KerfColor.Status.error
    }
  }

  private var contentWidth: CGFloat {
    (icon == nil ? 0 : KerfIcon.size + KerfSpace.sm) + Drawing.width(title, size.text)
  }

  public override var intrinsicContentSize: NSSize {
    NSSize(
      width: contentWidth + size.padding * 2 + (stroke == nil ? 0 : 2), height: size.height)
  }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.enabled(isEnabled) {
      Drawing.box(bounds, radius: KerfRadius.full, fill: fill, stroke: stroke)
      if isHighlighted {
        Drawing.box(bounds, radius: KerfRadius.full, fill: KerfColor.Background.fillMedium)
      }
      var x = ((bounds.width - contentWidth) / 2).rounded()
      if let icon {
        Drawing.icon(icon, color: ink, x: x, in: bounds)
        x += KerfIcon.size + KerfSpace.sm
      }
      Drawing.label(title, size.text, color: ink, x: x, in: bounds)
    }
  }
}

/// A 28 pt round button with a single icon, as in toolbars and panel
/// headers. `isActive` marks the panel or mode it shows as current.
public final class KerfIconButton: NSButton {
  public static let side: CGFloat = 28

  public var icon: KerfIcon { didSet { needsDisplay = true } }
  public var isActive = false { didSet { needsDisplay = true } }

  /// `label` is what VoiceOver reads; the icon alone says nothing.
  public init(icon: KerfIcon, label: String, target: AnyObject? = nil, action: Selector? = nil) {
    self.icon = icon
    super.init(frame: NSRect(x: 0, y: 0, width: Self.side, height: Self.side))
    (self.target, self.action) = (target, action)
    (title, isBordered, toolTip) = ("", false, label)
    setAccessibilityLabel(label)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }
  public override var intrinsicContentSize: NSSize { NSSize(width: Self.side, height: Self.side) }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.enabled(isEnabled) {
      if isHighlighted {
        Drawing.box(bounds, radius: KerfRadius.full, fill: KerfColor.Background.fill)
      }
      let ink = isActive ? KerfColor.Text.primary : KerfColor.Text.secondary
      Drawing.icon(icon, color: ink, x: ((bounds.width - KerfIcon.size) / 2).rounded(), in: bounds)
    }
  }
}
