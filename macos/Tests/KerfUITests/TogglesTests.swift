// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Testing

@testable import KerfUI

/// Counts the actions a control sends.
final class ActionCounter: NSObject {
  var hits = 0
  @objc func hit(_ sender: Any?) { hits += 1 }
}

/// A borderless window holding `view`, so clicks and the responder chain
/// behave as in the app.
@MainActor func host(_ view: NSView) -> NSWindow {
  let window = NSWindow(
    contentRect: NSRect(x: 0, y: 0, width: 400, height: 100), styleMask: [], backing: .buffered,
    defer: true)
  // Swift owns the window; AppKit releasing it on close as well would crash.
  window.isReleasedWhenClosed = false
  window.contentView?.addSubview(view)
  return window
}

@Suite @MainActor struct TogglesTests {
  @Test func switchesDrawTheirTrackAndKnobByState() {
    let toggle = KerfSwitch(label: "Wrap lines")
    #expect(toggle.frame.size == NSSize(width: 32, height: 18))
    var rep = render(toggle)
    #expect(close(sample(rep, 24, 9), premultiplied(KerfColor.Background.switchOff)))
    #expect(sample(rep, 9, 9) == bytes(KerfColor.Text.onAccent))
    toggle.isOn = true
    rep = render(toggle)
    #expect(sample(rep, 6, 9) == bytes(KerfColor.Accent.default))
    #expect(sample(rep, 23, 9) == bytes(KerfColor.Text.onAccent))
  }

  @Test func clickingAToggleFlipsItAndSendsTheAction() {
    let counter = ActionCounter()
    for toggle in [
      KerfSwitch(label: "A", target: counter, action: #selector(ActionCounter.hit(_:))),
      KerfCheckbox(label: "B", target: counter, action: #selector(ActionCounter.hit(_:))),
    ] as [KerfToggle] {
      let window = host(toggle)
      toggle.performClick(nil)
      #expect(toggle.isOn)
      toggle.performClick(nil)
      #expect(!toggle.isOn)
      #expect(toggle.cell?.accessibilityRole() == .checkBox)
      window.close()
    }
    #expect(counter.hits == 4)
  }

  @Test func checkboxesFillWhenOn() {
    let box = KerfCheckbox(label: "Trust", isOn: true)
    #expect(box.frame.size == NSSize(width: 16, height: 16))
    #expect(sample(render(box), 3, 3) == bytes(KerfColor.Accent.default))
    #expect(sample(render(box), 7.5, 10.5) == bytes(KerfColor.Text.onAccent))
    box.isOn = false
    #expect(close(sample(render(box), 8, 8), premultiplied(KerfColor.Background.fill)))
  }

  @Test func segmentsSizeByTheirEmphasizedLabels() {
    let control = KerfSegmentedControl(labels: ["General", "Editor", "Keys"])
    let widths = ["General", "Editor", "Keys"].map { Drawing.width($0, .bodyEmphasized) + 24 }
    #expect(control.segmentFrames.map(\.width) == widths)
    #expect(control.frame.size == NSSize(width: widths.reduce(0, +) + 2 * 4, height: 28))
    // The selected segment's white fill-strong lies over the track's white
    // fill: the alphas compose as a + b(1 - a).
    let (track, segment) = (0x14, 0x24)
    let alpha = segment + track * (255 - segment) / 255
    #expect(close(sample(render(control), 6, 14), [alpha, alpha, alpha, alpha]))
  }

  @Test func clicksAndArrowsMoveTheSelection() {
    let counter = ActionCounter()
    let control = KerfSegmentedControl(labels: ["General", "Editor", "Keys"])
    (control.target, control.action) = (counter, #selector(ActionCounter.hit(_:)))
    let window = host(control)
    control.select(at: NSPoint(x: control.segmentFrames[2].midX, y: 14))
    #expect(control.selectedIndex == 2)
    control.moveRight(nil)
    #expect(control.selectedIndex == 2)
    control.moveLeft(nil)
    #expect(control.selectedIndex == 1)
    control.select(at: NSPoint(x: control.segmentFrames[1].midX, y: 14))
    #expect(counter.hits == 2)
    #expect(control.accessibilityValue() as? String == "Editor")
    window.close()
  }
}
