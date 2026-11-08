// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Testing

@testable import KerfUI

@Suite @MainActor struct GalleryTests {
  /// Every view under `view`, depth first.
  func descendants(_ view: NSView) -> [NSView] {
    view.subviews.flatMap { [$0] + descendants($0) }
  }

  @Test func theGalleryShowsEveryControlAndIcon() {
    let views = descendants(KerfGallery.makeView())
    let kinds: [AnyClass] = [
      KerfButton.self, KerfIconButton.self, KerfChip.self, KerfStatus.self, KerfKey.self,
      KerfCountBadge.self, KerfAvatar.self, KerfSwitch.self, KerfCheckbox.self,
      KerfSegmentedControl.self, KerfTextField.self, KerfSelect.self, KerfSpinner.self,
      KerfProgressBar.self,
    ]
    for kind in kinds {
      #expect(views.contains { type(of: $0) == kind }, "\(kind) missing")
    }
    let buttons = views.compactMap { $0 as? KerfButton }
    #expect(buttons.count == KerfButton.Style.allCases.count * KerfButton.Size.allCases.count + 1)
    #expect(views.filter { $0 is NSImageView }.count == KerfIcon.allCases.count)
  }

  @Test func theGalleryWindowFitsItsContentWithoutSqueezingIt() {
    let window = KerfGallery.makeWindow()
    let content = window.contentView!
    content.layoutSubtreeIfNeeded()
    #expect(window.contentLayoutRect.size == content.fittingSize)
    let margin = KerfSpace.xl4
    for view in descendants(content) where !(view is NSStackView) {
      let frame = view.convert(view.bounds, to: content)
      #expect(frame.maxX <= content.bounds.width - margin, "\(type(of: view)) reaches the edge")
    }
    for view in descendants(content) where !(view is NSStackView) {
      let natural = view.intrinsicContentSize.width
      if natural != NSView.noIntrinsicMetric {
        #expect(view.frame.width >= natural, "\(type(of: view)) is squeezed")
      }
      #expect(view.frame.width > 0, "\(type(of: view)) collapsed")
    }
    window.close()
  }
}
