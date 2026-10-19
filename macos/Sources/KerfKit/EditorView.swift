// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import CKerf

/// The code editor. Kerf draws the text on the GPU into this view's layer;
/// AppKit supplies the input, the caret and the theme, so typing, IME, key
/// bindings and the caret behave as in any Mac text view.
public final class EditorView: NSView, @preconcurrency NSTextInputClient {
  public let editor: Editor
  public private(set) var theme: EditorTheme
  /// The last render's failure; a hidden window's skipped frame is none.
  public private(set) var renderError: KerfError?
  private let themeFor: (_ dark: Bool) -> EditorTheme
  private let caret = NSTextInsertionIndicator()
  // Read by `deinit`, which is not main-actor isolated before macOS 15.4;
  // the view is only ever used from the main thread.
  nonisolated(unsafe) private var view: OpaquePointer?

  /// `themeFor` picks the theme for Light (false) or Dark (true).
  public init(
    editor: Editor,
    frame: NSRect = .zero,
    themeFor: @escaping (_ dark: Bool) -> EditorTheme = { XcodeTheme.current(dark: $0) }
  ) throws {
    self.editor = editor
    self.themeFor = themeFor
    theme = themeFor(false)
    super.init(frame: frame)
    // wgpu puts its Metal layer into this view's layer.
    wantsLayer = true
    theme = themeFor(isDark)
    let size = pixelSize
    var out: OpaquePointer?
    try theme.withKerfTheme { theme in
      try check(
        kerf_view_attach_appkit(
          Unmanaged.passUnretained(self).toOpaque(), size.width, size.height,
          Float(scale), theme, &out))
    }
    view = try unwrap(out)
    caret.displayMode = .hidden
    addSubview(caret)
    render()
  }

  required init?(coder: NSCoder) { nil }

  deinit {
    kerf_view_free(view)
  }

  public override var isFlipped: Bool { true }
  public override var acceptsFirstResponder: Bool { true }

  /// The caret in view coordinates.
  public var caretFrame: NSRect { caret.frame }

  /// Draws the editor now and moves the system caret to it.
  public func render() {
    guard let view else { return }
    let status = kerf_view_render(view, editor.handle)
    renderError =
      status == KERF_STATUS_OK || status == KERF_STATUS_NOTHING ? nil : KerfError(status: status)
    var rect = KerfRect()
    guard kerf_view_caret(view, &rect) == KERF_STATUS_OK else {
      caret.displayMode = .hidden
      return
    }
    // Kerf reports physical pixels from the top left; the view is flipped.
    caret.frame = NSRect(
      x: CGFloat(rect.x) / scale, y: CGFloat(rect.y) / scale,
      width: CGFloat(rect.width) / scale, height: CGFloat(rect.height) / scale)
    caret.displayMode = window?.firstResponder === self ? .automatic : .hidden
  }

  private var scale: CGFloat {
    window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
  }

  private var isDark: Bool { effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua }

  private var pixelSize: (width: UInt32, height: UInt32) {
    let size = bounds.size
    return (
      UInt32(max(1, (size.width * scale).rounded())),
      UInt32(max(1, (size.height * scale).rounded()))
    )
  }

  private func resizeSurface() {
    guard let view else { return }
    let size = pixelSize
    _ = kerf_view_resize(view, size.width, size.height, Float(scale))
    render()
  }

  public override func setFrameSize(_ newSize: NSSize) {
    super.setFrameSize(newSize)
    resizeSurface()
  }

  public override func viewDidChangeBackingProperties() {
    super.viewDidChangeBackingProperties()
    resizeSurface()
  }

  public override func viewDidChangeEffectiveAppearance() {
    super.viewDidChangeEffectiveAppearance()
    guard let view else { return }
    theme = themeFor(isDark)
    _ = theme.withKerfTheme { kerf_view_set_theme(view, $0) }
    render()
  }

  public override func becomeFirstResponder() -> Bool {
    caret.displayMode = .automatic
    return true
  }

  public override func resignFirstResponder() -> Bool {
    caret.displayMode = .hidden
    return true
  }

  public override func mouseDown(with event: NSEvent) {
    window?.makeFirstResponder(self)
  }

  public override func keyDown(with event: NSEvent) {
    // The system's key bindings and input methods turn keys into
    // insertText and doCommand calls.
    interpretKeyEvents([event])
  }

  /// One edit, then a redraw; a refused edit (e.g. an oversized paste) beeps.
  private func edit(_ change: () throws -> Void) {
    do { try change() } catch { NSSound.beep() }
    render()
  }

  @objc public func undo(_ sender: Any?) { edit { try editor.run(.undo) } }
  @objc public func redo(_ sender: Any?) { edit { try editor.run(.redo) } }

  // MARK: NSTextInputClient

  public func insertText(_ string: Any, replacementRange: NSRange) {
    let text = (string as? NSAttributedString)?.string ?? string as? String ?? ""
    edit { try editor.insert(text) }
  }

  public override func doCommand(by selector: Selector) {
    switch selector {
    case #selector(deleteBackward(_:)): edit { try editor.run(.backspace) }
    case #selector(moveLeft(_:)): edit { try editor.run(.left) }
    case #selector(moveRight(_:)): edit { try editor.run(.right) }
    case #selector(insertNewline(_:)): edit { try editor.insert("\n") }
    case #selector(insertTab(_:)): edit { try editor.insert("\t") }
    // Other bindings need core support (lines, words, selection).
    default: break
    }
  }

  // An input method's composition is committed as typed: showing it inline
  // needs a marked range in the core.
  public func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {}
  public func unmarkText() {}
  public func hasMarkedText() -> Bool { false }
  public func markedRange() -> NSRange { NSRange(location: NSNotFound, length: 0) }
  public func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }

  public func selectedRange() -> NSRange {
    // AppKit counts UTF-16 units; Kerf's cursor is a UTF-8 offset.
    guard let text = try? editor.text(), let cursor = try? editor.cursor() else {
      return NSRange(location: NSNotFound, length: 0)
    }
    let index = text.utf8.index(text.startIndex, offsetBy: cursor)
    return NSRange(location: text.utf16.distance(from: text.startIndex, to: index), length: 0)
  }

  public func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?)
    -> NSAttributedString?
  { nil }

  public func characterIndex(for point: NSPoint) -> Int { NSNotFound }

  /// Where an input method puts its candidate window: at the caret.
  public func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
    window?.convertToScreen(convert(caret.frame, to: nil)) ?? .zero
  }
}
