// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

// Kerf for macOS: an AppKit window per file around Kerf's editor view.
// `swift run Kerf <file>` opens a file; File > Open picks one.

import AppKit
import KerfKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
  private var windows: [NSWindow] = []

  func applicationDidFinishLaunching(_ notification: Notification) {
    NSApp.mainMenu = mainMenu()
    open(CommandLine.arguments.dropFirst().first.map { URL(fileURLWithPath: $0) })
    NSApp.activate()
  }

  func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

  @objc func openDocument(_ sender: Any?) {
    let panel = NSOpenPanel()
    if panel.runModal() == .OK { open(panel.url) }
  }

  private func open(_ url: URL?) {
    do {
      let editor = try url.map(Editor.init(contentsOf:)) ?? Editor()
      let view = try EditorView(editor: editor, frame: NSRect(x: 0, y: 0, width: 900, height: 600))
      let window = NSWindow(
        contentRect: view.frame, styleMask: [.titled, .closable, .miniaturizable, .resizable],
        backing: .buffered, defer: false)
      window.title = url?.lastPathComponent ?? "Untitled"
      window.representedURL = url
      window.isReleasedWhenClosed = false
      window.contentView = view
      window.makeFirstResponder(view)
      window.center()
      window.makeKeyAndOrderFront(nil)
      windows.append(window)
    } catch {
      NSAlert(error: error).runModal()
    }
  }

  /// The standard App, File and Edit menus; Undo and Redo reach the
  /// focused editor through the responder chain.
  private func mainMenu() -> NSMenu {
    let app = NSMenu()
    app.addItem(
      withTitle: "Quit Kerf", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
    let file = NSMenu(title: "File")
    file.addItem(withTitle: "Open…", action: #selector(openDocument(_:)), keyEquivalent: "o")
    let edit = NSMenu(title: "Edit")
    edit.addItem(withTitle: "Undo", action: #selector(EditorView.undo(_:)), keyEquivalent: "z")
    edit.addItem(withTitle: "Redo", action: #selector(EditorView.redo(_:)), keyEquivalent: "z")
      .keyEquivalentModifierMask = [.command, .shift]
    let bar = NSMenu()
    for menu in [app, file, edit] {
      let item = NSMenuItem()
      item.submenu = menu
      bar.addItem(item)
    }
    return bar
  }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
