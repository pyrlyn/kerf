// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Testing

@testable import KerfUI

@Suite @MainActor struct LabelsTests {
  @Test func chipsHugTheirTextAfterTheDot() {
    let chip = KerfChip("Label", tone: .ok)
    let text = Drawing.width("Label", .captionBold)
    #expect(chip.frame.size == NSSize(width: 8 + 6 + 4 + text + 8, height: 18))
  }

  @Test func chipsTintBoxDotAndTextByTone() {
    for tone in KerfTone.allCases {
      let rep = render(KerfChip("Label", tone: tone))
      #expect(close(sample(rep, 9, 2), premultiplied(tone.background)), "\(tone) box")
      #expect(sample(rep, 11, 9) == bytes(tone.ink), "\(tone) dot")
    }
  }

  @Test func statusLinesAreADotAndSecondaryText() {
    let status = KerfStatus("rust-analyzer · ready", tone: .busy)
    #expect(status.frame.height == 16)
    let rep = render(status)
    #expect(sample(rep, 3.5, 8) == bytes(KerfColor.Status.busy))
    #expect(sample(rep, 3.5, 1)[3] == 0)
  }

  @Test func keysSitInAFilledCap() {
    let key = KerfKey("⌘⇧O")
    #expect(key.frame.height == 18)
    #expect(key.frame.width == Drawing.width("⌘⇧O", .caption) + 12)
    #expect(close(sample(render(key), 2, 9), premultiplied(KerfColor.Background.fillMedium)))
  }

  @Test func countBadgesKeepAMinimumWidthAndCentreTheCount() {
    let badge = KerfCountBadge(3, tone: .error)
    #expect(
      badge.frame.size == NSSize(width: max(16, Drawing.width("3", .captionBold) + 12), height: 16))
    #expect(sample(render(badge), 1.5, 8) == bytes(KerfColor.Status.error))
    badge.count = 128
    #expect(badge.frame.width == Drawing.width("128", .captionBold) + 12)
    #expect(badge.accessibilityValue() as? String == "128")
  }

  @Test func avatarsShowInitials() {
    let avatar = KerfAvatar(name: "Yuki Nakamura")
    #expect(avatar.text == "YN")
    #expect(avatar.frame.size == NSSize(width: 24, height: 24))
    #expect(sample(render(avatar), 12, 3) == bytes(KerfColor.Accent.default))
    #expect(sample(render(avatar), 0.5, 0.5)[3] == 0)
    #expect(avatar.accessibilityLabel() == "Yuki Nakamura")
  }
}
