// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Foundation
import Testing

@testable import KerfKit

/// A temporary folder removed when the test ends.
final class TempDir {
  let url = FileManager.default.temporaryDirectory.appending(path: "kerf-\(UUID().uuidString)")

  init() throws {
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
  }

  deinit { try? FileManager.default.removeItem(at: url) }

  func write(_ name: String, _ text: String) throws -> URL {
    let file = url.appending(path: name)
    try text.write(to: file, atomically: true, encoding: .utf8)
    return file
  }
}

@MainActor
@Suite struct EditorViewTests {
  /// The whole host path: open a file, type through NSTextInputClient,
  /// key bindings, the Edit menu's undo and redo, and Kerf's frame on the
  /// view's layer with the system caret placed on it.
  @Test func opensTypesUndoesAndRedoesThroughTheView() throws {
    let dir = try TempDir()
    let file = try dir.write("main.rs", "fn main() {}\n")
    let theme = EditorTheme.kerf(dark: false)
    let view = try EditorView(
      editor: Editor(contentsOf: file), frame: NSRect(x: 0, y: 0, width: 320, height: 120),
      themeFor: { _ in theme })
    let window = NSWindow(
      contentRect: view.frame, styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = view
    #expect(window.makeFirstResponder(view))

    view.insertText("// é", replacementRange: NSRange(location: NSNotFound, length: 0))
    view.doCommand(by: #selector(NSResponder.insertNewline(_:)))
    view.doCommand(by: #selector(NSResponder.deleteBackward(_:)))
    #expect(try view.editor.text() == "// éfn main() {}\n")
    view.undo(nil)
    #expect(try view.editor.text() == "// é\nfn main() {}\n")
    view.redo(nil)
    #expect(try view.editor.text() == "// éfn main() {}\n")
    view.doCommand(by: #selector(NSResponder.moveLeft(_:)))
    // "// " then "é": three UTF-16 units before the cursor.
    #expect(view.selectedRange() == NSRange(location: 3, length: 0))

    #expect(view.renderError == nil)
    #expect(view.caretFrame.minY == 0)
    #expect(view.caretFrame.minX > 0)
    #expect(view.caretFrame.height == theme.lineHeight)

    // The frame fills the view at the window's scale, on the theme's background.
    let image = try #require(view.frameImage)
    let scale = window.backingScaleFactor
    #expect(image.width == Int(320 * scale) && image.height == Int(120 * scale))
    #expect(view.layer?.contents as! CGImage? === image)
    let pixels = try #require(image.dataProvider?.data as Data?)
    let last = (image.height * image.bytesPerRow) - 4
    let bg = theme.background
    #expect(Array(pixels[last..<last + 4]) == [bg.r, bg.g, bg.b, bg.a])
  }

  /// Shift-arrows, AppKit's UTF-16 ranges and replacing a selection, with
  /// the system caret hidden while something is selected.
  @Test func selectsAndReplacesTextThroughTheView() throws {
    let view = try EditorView(
      editor: Editor(), frame: NSRect(x: 0, y: 0, width: 320, height: 60),
      themeFor: { EditorTheme.kerf(dark: $0) })
    let window = NSWindow(
      contentRect: view.frame, styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = view
    #expect(window.makeFirstResponder(view))
    let none = NSRange(location: NSNotFound, length: 0)
    view.insertText("é hello world", replacementRange: none)
    // "é" is one UTF-16 unit but two UTF-8 bytes.
    view.setSelectedRange(NSRange(location: 8, length: 5))
    #expect(view.selectedRange() == NSRange(location: 8, length: 5))
    #expect(view.caretHidden)
    view.insertText("kerf", replacementRange: none)
    #expect(try view.editor.text() == "é hello kerf")
    #expect(view.selectedRange() == NSRange(location: 12, length: 0))
    #expect(!view.caretHidden)

    view.doCommand(by: #selector(NSResponder.moveLeftAndModifySelection(_:)))
    view.doCommand(by: #selector(NSResponder.moveLeftAndModifySelection(_:)))
    #expect(view.selectedRange() == NSRange(location: 10, length: 2))
    view.doCommand(by: #selector(NSResponder.deleteBackward(_:)))
    #expect(try view.editor.text() == "é hello ke")
    view.undo(nil)
    #expect(try view.editor.text() == "é hello kerf")

    view.insertText("hi", replacementRange: NSRange(location: 2, length: 5))
    #expect(try view.editor.text() == "é hi kerf")
    view.selectAll(nil)
    #expect(view.selectedRange() == NSRange(location: 0, length: 9))
    view.doCommand(by: #selector(NSResponder.moveRight(_:)))
    #expect(view.selectedRange() == NSRange(location: 9, length: 0))
    #expect(view.renderError == nil)
  }

  @Test func followsTheAppearance() throws {
    let view = try EditorView(
      editor: Editor(), frame: NSRect(x: 0, y: 0, width: 100, height: 40),
      themeFor: { EditorTheme.kerf(dark: $0) })
    view.appearance = NSAppearance(named: .darkAqua)
    view.viewDidChangeEffectiveAppearance()
    #expect(view.theme == .kerf(dark: true))
    view.appearance = NSAppearance(named: .aqua)
    view.viewDidChangeEffectiveAppearance()
    #expect(view.theme == .kerf(dark: false))
    #expect(view.renderError == nil)
  }
}
