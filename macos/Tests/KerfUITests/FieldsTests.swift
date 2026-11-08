// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Testing

@testable import KerfUI

@Suite @MainActor struct FieldsTests {
  @Test func textFieldsPlaceTheEditorAfterTheIcon() {
    let box = KerfTextField(placeholder: "Filter", icon: .search)
    box.layoutSubtreeIfNeeded()
    #expect(box.frame.size == NSSize(width: 240, height: 28))
    #expect(abs(box.field.frame.minX - (10 + 16 + 8)) < 0.01)
    #expect(abs(box.field.frame.maxX - 230) < 0.01)
    let rep = render(box)
    #expect(close(sample(rep, 120, 0.25), over(KerfColor.Border.subtle, KerfColor.Background.fill)))
    #expect(sample(rep, 19, 14)[3] > 0, "the search icon")
  }

  @Test func textFieldsOutlineInAccentWhileFocused() {
    let box = KerfTextField(placeholder: "Filter")
    let window = host(box)
    #expect(!box.isFocused)
    window.makeFirstResponder(box.field)
    #expect(box.isFocused)
    #expect(sample(render(box), 120, 0.25) == bytes(KerfColor.Accent.ink))
    box.stringValue = "kerf"
    #expect(box.field.stringValue == "kerf")
    window.close()
  }

  @Test func selectsAreAsWideAsTheirLongestItem() {
    let select = KerfSelect(items: ["Every hour", "Daily"])
    let text = Drawing.width("Every hour", .body)
    #expect(select.frame.size == NSSize(width: 10 + text + 8 + 10 + 8, height: 28))
    select.selectItem(at: 1)
    #expect(select.intrinsicContentSize.width == select.frame.width)
    #expect(select.titleOfSelectedItem == "Daily")
    let rep = render(select)
    // The chevrons' tips, top and bottom.
    let x = select.frame.width - 18 + 5
    #expect(sample(rep, x, 9.3)[3] > 0 && sample(rep, x, 18.7)[3] > 0)
  }

  @Test func spinnersDrawTheArcOverTheTrack() {
    let spinner = KerfSpinner()
    let rep = render(spinner)
    // At rest the arc covers the right half; the left half shows the track.
    #expect(sample(rep, 14, 8) == bytes(KerfColor.Accent.default))
    #expect(close(sample(rep, 2, 8), premultiplied(KerfColor.Background.fillStrong), within: 3))
    spinner.angle = .pi
    #expect(sample(render(spinner), 2, 8) == bytes(KerfColor.Accent.default))
  }

  @Test func spinnersTurnOnlyInAWindow() {
    let spinner = KerfSpinner()
    spinner.isAnimating = true
    #expect(!spinner.isTurning)
    let window = host(spinner)
    #expect(spinner.isTurning == !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion)
    spinner.isAnimating = false
    #expect(!spinner.isTurning)
    window.close()
  }

  @Test func progressBarsFillTheirShareAndClamp() {
    let bar = KerfProgressBar(value: 2.0 / 3)
    #expect(bar.frame.size == NSSize(width: 240, height: 4))
    let rep = render(bar)
    #expect(sample(rep, 150, 2) == bytes(KerfColor.Accent.default))
    #expect(close(sample(rep, 200, 2), premultiplied(KerfColor.Background.fillMedium)))
    bar.value = 7
    #expect(bar.value == 1)
    #expect(bar.accessibilityValue() as? NSNumber == 1)
  }
}
