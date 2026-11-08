// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Testing

@testable import KerfUI

/// `view` drawn at 2x the way a window would draw it, flipped or not.
@MainActor func render(_ view: NSView) -> NSBitmapImageRep {
  view.layoutSubtreeIfNeeded()
  return render(view.bounds.size) {
    view.displayIgnoringOpacity(view.bounds, in: NSGraphicsContext.current!)
  }
}

/// Premultiplied bytes of `color` over a transparent pixel, as the bitmap
/// stores a translucent fill.
func premultiplied(_ color: NSColor) -> [Int] {
  let b = bytes(color)
  return b.prefix(3).map { ($0 * b[3] + 127) / 255 } + [b[3]]
}

func close(_ a: [Int], _ b: [Int], within tolerance: Int = 2) -> Bool {
  zip(a, b).allSatisfy { abs($0 - $1) <= tolerance }
}

@Suite @MainActor struct ButtonTests {
  @Test func sizesFollowTheFigmaVariants() {
    for (size, height, padding) in [
      (KerfButton.Size.small, 24.0, 10.0), (.regular, 28, 14), (.large, 36, 28),
    ] {
      let button = KerfButton(title: "Button", size: size)
      let text = Drawing.width("Button", size.text)
      #expect(button.frame.size == NSSize(width: text + padding * 2, height: height))
    }
  }

  @Test func outlinedStylesCountTheirStroke() {
    let filled = KerfButton(title: "Button", style: .secondary)
    let ghost = KerfButton(title: "Button", style: .ghost)
    #expect(ghost.frame.width == filled.frame.width + 2)
  }

  @Test func anIconAddsItsWidthAndGap() {
    let plain = KerfButton(title: "Run")
    let withIcon = KerfButton(title: "Run", icon: .play)
    #expect(withIcon.frame.width == plain.frame.width + 22)
  }

  @Test func fillsMatchTheTokens() {
    let primary = render(KerfButton(title: "Button", style: .primary))
    #expect(sample(primary, 4, 14) == bytes(KerfColor.Accent.default))
    let secondary = render(KerfButton(title: "Button", style: .secondary))
    #expect(close(sample(secondary, 4, 14), premultiplied(KerfColor.Background.fillStrong)))
    // Pill corners stay transparent.
    #expect(sample(primary, 0.5, 0.5)[3] == 0)
  }

  @Test func ghostButtonsAreOutlinesOnly() {
    let button = KerfButton(title: "Button", style: .ghost)
    let rep = render(button)
    #expect(sample(rep, button.frame.width / 2, 0.25)[3] > 0)
    #expect(sample(rep, 6, 14)[3] == 0)
  }

  @Test func destructiveLabelsUseTheErrorInk() {
    let button = KerfButton(title: "Delete", style: .destructiveOutline)
    let rep = render(button)
    let ink = (0..<Int(button.frame.width * 2)).map { sample(rep, CGFloat($0) / 2, 14) }
    #expect(ink.contains { close($0, bytes(KerfColor.Status.error), within: 40) })
  }

  @Test func buttonsSendTheirAction() {
    final class Target: NSObject {
      var hits = 0
      @objc func hit(_ sender: Any?) { hits += 1 }
    }
    let target = Target()
    let button = KerfButton(title: "Go", target: target, action: #selector(Target.hit(_:)))
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 100, height: 40), styleMask: [], backing: .buffered,
      defer: true)
    window.contentView?.addSubview(button)
    button.performClick(nil)
    #expect(target.hits == 1)
    // AppKit reports a button's role through its cell, as for a plain NSButton.
    #expect(button.cell?.accessibilityRole() == .button)
    #expect(button.accessibilityTitle() == "Go")
  }

  @Test func iconButtonsShowTheirStateInTheIconInk() {
    let button = KerfIconButton(icon: .close, label: "Close")
    #expect(button.frame.size == NSSize(width: 28, height: 28))
    #expect(sample(render(button), 14, 14) == bytes(KerfColor.Text.secondary))
    button.isActive = true
    #expect(sample(render(button), 14, 14) == bytes(KerfColor.Text.primary))
    #expect(button.accessibilityLabel() == "Close")
  }
}
