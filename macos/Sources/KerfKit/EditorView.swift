// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import CKerf

/// The code editor. Kerf's Slint component draws the text on the CPU and
/// this view shows the frame as its layer's contents; AppKit supplies the
/// input, the caret and the theme, so typing, IME, key bindings and the
/// caret behave as in any Mac text view.
public final class EditorView: NSView, @preconcurrency NSTextInputClient {
  public let editor: Editor
  public private(set) var theme: EditorTheme
  /// The last render's failure.
  public private(set) var renderError: KerfError?
  /// The frame on screen, as Kerf last drew it.
  public private(set) var frameImage: CGImage?
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
    wantsLayer = true
    theme = themeFor(isDark)
    let size = pixelSize
    var out: OpaquePointer?
    try theme.withKerfTheme { theme in
      try check(kerf_view_new(size.width, size.height, Float(scale), theme, &out))
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
  var caretHidden: Bool { caret.displayMode == .hidden }

  /// Draws the editor now and moves the system caret to it.
  public func render() {
    guard let view else { return }
    let status = kerf_view_render(view, editor.handle)
    renderError =
      status == KERF_STATUS_OK || status == KERF_STATUS_NOTHING ? nil : KerfError(status: status)
    if status == KERF_STATUS_OK { present(view) }
    var rect = KerfRect()
    // As in any Mac text view, a selection shows no caret.
    let selected = (try? editor.selection()).map { $0.anchor != $0.cursor } ?? false
    guard !selected, kerf_view_caret(view, &rect) == KERF_STATUS_OK else {
      caret.displayMode = .hidden
      return
    }
    // Kerf reports points from the top left; the view is flipped.
    caret.frame = NSRect(
      x: CGFloat(rect.x), y: CGFloat(rect.y), width: CGFloat(rect.width),
      height: CGFloat(rect.height))
    caret.displayMode = window?.firstResponder === self ? .automatic : .hidden
  }

  /// Copies Kerf's frame out and puts it on the layer.
  private func present(_ view: OpaquePointer) {
    var len = 0
    _ = kerf_view_read_rgba(view, nil, 0, &len)
    var pixels = Data(count: len)
    let status = pixels.withUnsafeMutableBytes {
      kerf_view_read_rgba(view, $0.baseAddress?.assumingMemoryBound(to: UInt8.self), len, &len)
    }
    let size = pixelSize
    guard status == KERF_STATUS_OK, len == Int(size.width) * Int(size.height) * 4,
      let provider = CGDataProvider(data: pixels as CFData),
      let image = CGImage(
        width: Int(size.width), height: Int(size.height), bitsPerComponent: 8,
        bitsPerPixel: 32, bytesPerRow: Int(size.width) * 4,
        space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
        // Slint's software renderer writes premultiplied RGBA.
        bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    else {
      renderError = KerfError(status: status == KERF_STATUS_OK ? KERF_STATUS_RENDER : status)
      return
    }
    frameImage = image
    layer?.contentsScale = scale
    layer?.contents = image
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
  public override func selectAll(_ sender: Any?) {
    edit { try editor.select(anchor: 0, cursor: editor.text().utf8.count) }
  }

  /// Selects `range`, in AppKit's UTF-16 units; the caret goes to its end.
  public func setSelectedRange(_ range: NSRange) {
    edit { try select(range) }
  }

  private func select(_ range: NSRange) throws {
    let text = try editor.text()
    // A range off the text or inside a surrogate pair maps to no UTF-8
    // offset, and the core refuses it.
    func utf8(_ unit: Int) throws -> Int {
      guard let at = text.utf16.index(text.startIndex, offsetBy: unit, limitedBy: text.endIndex),
        let scalar = at.samePosition(in: text.unicodeScalars)
      else { throw KerfError(status: KERF_STATUS_OUT_OF_RANGE) }
      return text.utf8.distance(from: text.startIndex, to: scalar)
    }
    try editor.select(anchor: utf8(range.location), cursor: utf8(NSMaxRange(range)))
  }

  // MARK: NSTextInputClient

  public func insertText(_ string: Any, replacementRange: NSRange) {
    let text = (string as? NSAttributedString)?.string ?? string as? String ?? ""
    edit {
      if replacementRange.location != NSNotFound { try select(replacementRange) }
      try editor.insert(text)
    }
  }

  public override func doCommand(by selector: Selector) {
    switch selector {
    case #selector(deleteBackward(_:)): edit { try editor.run(.backspace) }
    case #selector(moveLeft(_:)): edit { try editor.run(.left) }
    case #selector(moveRight(_:)): edit { try editor.run(.right) }
    case #selector(moveLeftAndModifySelection(_:)): edit { try editor.run(.selectLeft) }
    case #selector(moveRightAndModifySelection(_:)): edit { try editor.run(.selectRight) }
    case #selector(insertNewline(_:)): edit { try editor.insert("\n") }
    case #selector(insertTab(_:)): edit { try editor.insert("\t") }
    // Other bindings need core support (lines, words).
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
    // AppKit counts UTF-16 units; Kerf's offsets are UTF-8.
    guard let text = try? editor.text(), let (anchor, cursor) = try? editor.selection() else {
      return NSRange(location: NSNotFound, length: 0)
    }
    let utf16 = { (byte: Int) in
      text.utf16.distance(
        from: text.startIndex, to: text.utf8.index(text.startIndex, offsetBy: byte))
    }
    let (start, end) = (utf16(min(anchor, cursor)), utf16(max(anchor, cursor)))
    return NSRange(location: start, length: end - start)
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
