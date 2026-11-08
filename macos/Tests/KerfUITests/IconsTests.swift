// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Testing

@testable import KerfUI

/// `size` points drawn by `draw` at 2x into an sRGB bitmap, so tests sample
/// the bytes the tokens name with no colour matching in between.
@MainActor func render(_ size: NSSize, _ draw: () -> Void) -> NSBitmapImageRep {
  let context = CGContext(
    data: nil, width: Int(size.width * 2), height: Int(size.height * 2), bitsPerComponent: 8,
    bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
  context.scaleBy(x: 2, y: 2)
  NSGraphicsContext.saveGraphicsState()
  NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: false)
  draw()
  NSGraphicsContext.restoreGraphicsState()
  return NSBitmapImageRep(cgImage: context.makeImage()!)
}

@MainActor func render(_ image: NSImage) -> NSBitmapImageRep {
  render(image.size) { image.draw(in: NSRect(origin: .zero, size: image.size)) }
}

/// The raw sRGB bytes under point `(x, y)`, measured from the top-left in
/// points. Read with `getPixel`: `colorAt` labels them Generic RGB, and
/// converting that back to sRGB shifts every channel.
func sample(_ rep: NSBitmapImageRep, _ x: CGFloat, _ y: CGFloat) -> [Int] {
  var pixel = [Int](repeating: 0, count: 4)
  rep.getPixel(&pixel, atX: Int(x * 2), y: Int(y * 2))
  return pixel
}

@Suite struct IconsTests {
  @Test func everyIconStaysInsideItsSquare() {
    for icon in KerfIcon.allCases {
      let bounds = icon.path.bounds
      #expect(!bounds.isEmpty, "\(icon) has no outline")
      #expect(NSRect(x: 1, y: 1, width: 14, height: 14).contains(bounds), "\(icon) \(bounds)")
    }
  }

  @Test func pathDataParsesEveryCommand() {
    #expect(KerfIcon.close.path.bounds == NSRect(x: 4, y: 4, width: 8, height: 8))
    // H and V lines plus curves: the folder spans 2...14 by 3.33...12.67.
    let folder = KerfIcon.folder.path.bounds
    #expect(abs(folder.minY - 3.33) < 0.01 && abs(folder.maxY - 12.67) < 0.01)
    #expect(folder.minX == 2 && folder.maxX == 14)
  }

  @Test @MainActor func iconsDrawInTheGivenColour() {
    let rep = render(KerfIcon.close.image(color: KerfColor.Status.error))
    #expect(sample(rep, 8, 8) == bytes(KerfColor.Status.error))
    #expect(sample(rep, 8, 2)[3] == 0)
  }

  @Test @MainActor func flippedDrawingKeepsTheFigmaOrientation() {
    // The chevron points up: its tip at y 6 is ink, below it at y 10 the
    // middle is empty.
    let rep = render(KerfIcon.chevronUp.image(color: .white))
    #expect(sample(rep, 8, 6.2)[3] > 0)
    #expect(sample(rep, 8, 10)[3] == 0)
  }
}
