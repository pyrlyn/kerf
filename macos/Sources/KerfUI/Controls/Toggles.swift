// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// An on/off `NSButton` drawn by Kerf: clicks and Space flip `state` and
/// send the action, and VoiceOver reads it as a checkbox, as AppKit does.
public class KerfToggle: NSButton {
  public var isOn: Bool {
    get { state == .on }
    set {
      state = newValue ? .on : .off
      needsDisplay = true
    }
  }

  init(size: NSSize, label: String, isOn: Bool, target: AnyObject?, action: Selector?) {
    super.init(frame: NSRect(origin: .zero, size: size))
    setButtonType(.switch)
    (title, isBordered, imagePosition) = ("", false, .noImage)
    (self.target, self.action, self.isOn) = (target, action, isOn)
    setAccessibilityLabel(label)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }
  public override var intrinsicContentSize: NSSize { frame.size }
}

/// The UI Kit's 32 × 18 switch.
public final class KerfSwitch: KerfToggle {
  public init(label: String, isOn: Bool = false, target: AnyObject? = nil, action: Selector? = nil)
  {
    super.init(
      size: NSSize(width: 32, height: 18), label: label, isOn: isOn, target: target, action: action)
  }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.enabled(isEnabled) {
      let track = isOn ? KerfColor.Accent.default : KerfColor.Background.switchOff
      Drawing.box(bounds, radius: KerfRadius.full, fill: track)
      KerfColor.Text.onAccent.setFill()
      let knob = NSRect(x: isOn ? bounds.width - 16 : 2, y: 2, width: 14, height: 14)
      NSBezierPath(ovalIn: knob).fill()
    }
  }
}

/// The UI Kit's 16 pt checkbox, without a title: the row beside it labels it.
public final class KerfCheckbox: KerfToggle {
  public init(label: String, isOn: Bool = false, target: AnyObject? = nil, action: Selector? = nil)
  {
    super.init(
      size: NSSize(width: 16, height: 16), label: label, isOn: isOn, target: target, action: action)
  }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.enabled(isEnabled) {
      guard isOn else {
        return Drawing.box(
          bounds, radius: 4, fill: KerfColor.Background.fill, stroke: KerfColor.Border.strong)
      }
      Drawing.box(bounds, radius: 4, fill: KerfColor.Accent.default)
      let check = NSBezierPath()
      check.move(to: NSPoint(x: 5, y: 8.5))
      check.line(to: NSPoint(x: 7.5, y: 11))
      check.line(to: NSPoint(x: 12, y: 6))
      (check.lineWidth, check.lineCapStyle, check.lineJoinStyle) = (1.6, .round, .round)
      KerfColor.Text.onAccent.setStroke()
      check.stroke()
    }
  }
}

/// A pill of mutually exclusive segments, such as the tabs of a settings
/// pane. Every segment is as wide as its label in the selected (emphasized)
/// style, so the control does not shift when the selection moves.
public final class KerfSegmentedControl: NSControl {
  public var labels: [String] { didSet { invalidateIntrinsicContentSize() } }
  public var selectedIndex: Int {
    didSet {
      needsDisplay = true
      setAccessibilityValue(labels.indices.contains(selectedIndex) ? labels[selectedIndex] : nil)
    }
  }

  private static let segmentHeight: CGFloat = 24
  private static let inset: CGFloat = 2

  public init(labels: [String], selectedIndex: Int = 0) {
    (self.labels, self.selectedIndex) = (labels, selectedIndex)
    super.init(frame: .zero)
    setFrameSize(intrinsicContentSize)
    setAccessibilityElement(true)
    setAccessibilityRole(.radioGroup)
    setAccessibilityValue(labels.indices.contains(selectedIndex) ? labels[selectedIndex] : nil)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  public override var isFlipped: Bool { true }
  public override var acceptsFirstResponder: Bool { true }

  /// Each segment's frame, left to right.
  var segmentFrames: [NSRect] {
    var x = Self.inset
    return labels.map { label in
      let width = Drawing.width(label, .bodyEmphasized) + KerfSpace.xl * 2
      defer { x += width + Self.inset }
      return NSRect(x: x, y: Self.inset, width: width, height: Self.segmentHeight)
    }
  }

  public override var intrinsicContentSize: NSSize {
    NSSize(width: (segmentFrames.last?.maxX ?? 0) + Self.inset, height: 28)
  }

  public override func draw(_ dirtyRect: NSRect) {
    Drawing.enabled(isEnabled) {
      Drawing.box(bounds, radius: KerfRadius.full, fill: KerfColor.Background.fill)
      for (index, frame) in segmentFrames.enumerated() {
        let selected = index == selectedIndex
        if selected {
          Drawing.box(frame, radius: KerfRadius.full, fill: KerfColor.Background.fillStrong)
        }
        let style: KerfTextStyle = selected ? .bodyEmphasized : .body
        let ink = selected ? KerfColor.Text.primary : KerfColor.Text.secondary
        let x = frame.minX + ((frame.width - Drawing.width(labels[index], style)) / 2).rounded()
        Drawing.label(labels[index], style, color: ink, x: x, in: frame)
      }
    }
  }

  /// Selects the segment under `point` (in this view) and sends the action;
  /// clicks between segments do nothing.
  func select(at point: NSPoint) {
    guard let index = segmentFrames.firstIndex(where: { $0.contains(point) }) else { return }
    select(index)
  }

  private func select(_ index: Int) {
    guard isEnabled, labels.indices.contains(index), index != selectedIndex else { return }
    selectedIndex = index
    sendAction(action, to: target)
  }

  public override func mouseDown(with event: NSEvent) {
    select(at: convert(event.locationInWindow, from: nil))
  }

  public override func moveLeft(_ sender: Any?) { select(selectedIndex - 1) }
  public override func moveRight(_ sender: Any?) { select(selectedIndex + 1) }
  public override func keyDown(with event: NSEvent) { interpretKeyEvents([event]) }
}
