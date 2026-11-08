// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit

/// Every control and icon of the UI Kit on one dark page, laid out like the
/// Figma "Controls" and "Icons" frames, to compare the two side by side.
/// `Kerf --ui-kit` opens it in a window.
@MainActor public enum KerfGallery {
  public static func makeWindow() -> NSWindow {
    let content = makeView()
    let window = NSWindow(
      contentRect: NSRect(origin: .zero, size: content.fittingSize),
      styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
    (window.title, window.appearance) = ("Kerf UI Kit", NSAppearance(named: .darkAqua))
    window.backgroundColor = KerfColor.Background.editor
    window.contentView = content
    window.isReleasedWhenClosed = false
    return window
  }

  public static func makeView() -> NSView {
    let page = stack(
      .vertical, spacing: KerfSpace.xl4,
      [
        section("Buttons", buttons()),
        section("Labels", labels()),
        section("Toggles", toggles()),
        section("Fields", fields()),
        section("Icons", icons()),
      ])
    // A stack's own edge insets give way on the trailing side of a leading
    // aligned column, so the margins are required constraints on a container.
    let container = NSView()
    page.translatesAutoresizingMaskIntoConstraints = false
    container.addSubview(page)
    let margin = KerfSpace.xl4
    NSLayoutConstraint.activate([
      page.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: margin),
      page.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -margin),
      page.topAnchor.constraint(equalTo: container.topAnchor, constant: margin),
      page.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -margin),
    ])
    return container
  }

  private static func buttons() -> NSView {
    stack(
      .vertical, spacing: KerfSpace.xl,
      KerfButton.Style.allCases.map { style in
        row(KerfButton.Size.allCases.map { KerfButton(title: "Button", style: style, size: $0) })
      } + [row([KerfButton(title: "Run", style: .primary, icon: .play), iconButtons()])])
  }

  private static func iconButtons() -> NSView {
    let active = KerfIconButton(icon: .navigator, label: "Navigator")
    active.isActive = true
    return row([KerfIconButton(icon: .navigator, label: "Navigator"), active])
  }

  private static func labels() -> NSView {
    stack(
      .vertical, spacing: KerfSpace.xl,
      [
        row(KerfTone.allCases.map { KerfChip(String(describing: $0).capitalized, tone: $0) }),
        row(
          [KerfTone.ok, .warning, .error, .busy].map {
            KerfStatus("rust-analyzer · \($0)", tone: $0)
          }),
        row(
          [KerfKey("⌘⇧O"), KerfAvatar(name: "Yuki Nakamura")]
            + KerfCountBadge.Tone.allCases.map { KerfCountBadge(3, tone: $0) }),
      ])
  }

  private static func toggles() -> NSView {
    row([
      KerfSwitch(label: "On", isOn: true), KerfSwitch(label: "Off"),
      KerfCheckbox(label: "On", isOn: true), KerfCheckbox(label: "Off"),
      KerfSegmentedControl(labels: ["General", "Text Editing", "Themes"]),
    ])
  }

  private static func fields() -> NSView {
    let spinner = KerfSpinner()
    spinner.isAnimating = true
    let value = KerfTextField(placeholder: "Filter", icon: .search)
    value.stringValue = "kerf-core"
    return stack(
      .vertical, spacing: KerfSpace.xl,
      [
        row([KerfTextField(placeholder: "Filter", icon: .search), value]),
        row([
          KerfSelect(items: ["Every hour", "Daily", "Never"]), spinner,
          KerfProgressBar(value: 2.0 / 3),
        ]),
      ])
  }

  private static func icons() -> NSView {
    let views = KerfIcon.allCases.map { icon in
      let view = NSImageView(image: icon.image())
      view.toolTip = icon.rawValue
      return view
    }
    return stack(
      .vertical, spacing: KerfSpace.xl,
      stride(from: 0, to: views.count, by: 14).map {
        row(Array(views[$0..<min($0 + 14, views.count)]))
      })
  }

  private static func section(_ title: String, _ body: NSView) -> NSView {
    let label = NSTextField(
      labelWithAttributedString: KerfTextStyle.sectionLabel.string(
        title, color: KerfColor.Text.secondary))
    return stack(.vertical, spacing: KerfSpace.xl, [label, body])
  }

  private static func row(_ views: [NSView]) -> NSStackView {
    stack(.horizontal, spacing: KerfSpace.xl, views)
  }

  private static func stack(
    _ orientation: NSUserInterfaceLayoutOrientation, spacing: CGFloat, _ views: [NSView]
  ) -> NSStackView {
    let stack = NSStackView(views: views)
    (stack.orientation, stack.spacing) = (orientation, spacing)
    stack.alignment = orientation == .vertical ? .leading : .centerY
    // The page sizes the window, so no stack may clip what it holds.
    stack.setClippingResistancePriority(.required, for: .horizontal)
    // Fields and bars stretch in real layouts; here they keep their frame.
    for view in views
    where !(view is NSStackView) && view.intrinsicContentSize.width == NSView.noIntrinsicMetric {
      view.widthAnchor.constraint(equalToConstant: view.frame.width).isActive = true
    }
    return stack
  }
}
