// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Testing

@testable import KerfUI

/// The colour's sRGB channels as bytes, the way Figma lists them.
func bytes(_ color: NSColor) -> [Int] {
  let c = color.usingColorSpace(.sRGB)!
  return [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent]
    .map { Int(($0 * 255).rounded()) }
}

@Suite struct TokensTests {
  @Test func hexColoursKeepEveryChannel() {
    #expect(bytes(KerfColor.Background.window) == [0x17, 0x1a, 0x22, 0xe6])
    #expect(bytes(KerfColor.Accent.default) == [0x08, 0x73, 0xe0, 0xff])
    #expect(bytes(KerfColor.Background.selection) == [0x0a, 0x84, 0xff, 0x3d])
  }

  @Test func textStylesMatchTheFigmaSizes() {
    #expect(KerfTextStyle.body.font.pointSize == 13)
    #expect(KerfTextStyle.code.font.isFixedPitch)
    #expect(!KerfTextStyle.title.font.isFixedPitch)
    let attrs = KerfTextStyle.largeTitle.attributes(color: .white)
    let paragraph = attrs[.paragraphStyle] as? NSParagraphStyle
    #expect(paragraph?.minimumLineHeight == 32)
    #expect(attrs[.kern] as? CGFloat == -0.5)
  }

  @Test func sectionLabelsAreUpperCase() {
    let label = KerfTextStyle.sectionLabel.string("Project", color: KerfColor.Text.secondary)
    #expect(label.string == "PROJECT")
  }

  @Test func laidOutLinesKeepTheStyleHeight() {
    let text = KerfTextStyle.small.string("Kerf", color: .white)
    let height = text.boundingRect(
      with: NSSize(width: 200, height: 100), options: [.usesLineFragmentOrigin]
    ).height
    #expect(height == 16)
  }
}
