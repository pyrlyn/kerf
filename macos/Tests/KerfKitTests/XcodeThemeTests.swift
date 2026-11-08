// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import Foundation
import Testing

@testable import KerfKit

/// A theme in Xcode's property-list shape, written by the test: Kerf never
/// copies Apple's own theme files.
private func writeTheme(_ dir: TempDir, _ name: String, background: String, font: String) throws {
  let plist: [String: Any] = [
    "DVTSourceTextBackground": background,
    "DVTSourceTextSelectionColor": "0.2 0.4 0.6 1",
    "DVTSourceTextSyntaxColors": ["xcode.syntax.plain": "1 1 1 0.85"],
    "DVTSourceTextSyntaxFonts": ["xcode.syntax.plain": font],
  ]
  let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
  try data.write(to: dir.url.appending(path: name))
}

@Suite struct XcodeThemeTests {
  @Test func readsTheCurrentDarkThemeFromXcodesDefaults() throws {
    let dir = try TempDir()
    try writeTheme(
      dir, "Mine.xccolortheme", background: "0.1 0.2 0.3 1", font: "Menlo-Regular - 14.0")
    let defaults = try #require(UserDefaults(suiteName: "kerf-test-\(UUID().uuidString)"))
    defaults.set("Mine.xccolortheme", forKey: "XCFontAndColorCurrentDarkTheme")

    let theme = XcodeTheme.current(dark: true, defaults: defaults, folders: [dir.url])
    #expect(theme.background == RGBA(26, 51, 77))
    #expect(theme.text == RGBA(255, 255, 255, 217))
    #expect(theme.selection == RGBA(51, 102, 153))
    #expect(theme.fontFamily == "Menlo")
    #expect(theme.fontSize == 14)
    #expect(theme.lineHeight > 14)
  }

  @Test func fallsBackToKerfsThemeWhenXcodesIsMissingOrBroken() throws {
    let dir = try TempDir()
    try writeTheme(
      dir, "Default (Light).xccolortheme", background: "not a colour", font: "Menlo-Regular - 14.0")
    let defaults = try #require(UserDefaults(suiteName: "kerf-test-\(UUID().uuidString)"))
    #expect(
      XcodeTheme.current(dark: false, defaults: defaults, folders: [dir.url]) == .kerf(dark: false))
    #expect(
      XcodeTheme.current(dark: true, defaults: defaults, folders: [dir.url]) == .kerf(dark: true))
    defaults.set("../Mine.xccolortheme", forKey: "XCFontAndColorCurrentTheme")
    #expect(
      XcodeTheme.current(dark: false, defaults: defaults, folders: [dir.url]) == .kerf(dark: false))
  }

  @Test func parsesColoursAndFontSpecs() {
    #expect(XcodeTheme.color("1 0 0.5 1") == RGBA(255, 0, 128))
    #expect(XcodeTheme.color("2 -1 0 1") == RGBA(255, 0, 0))
    #expect(XcodeTheme.color("1 0 0") == nil)
    #expect(XcodeTheme.font("Menlo-Regular - 12.0")?.pointSize == 12)
    #expect(XcodeTheme.font("Menlo-Regular") == nil)
    #expect(XcodeTheme.font("Menlo-Regular - 0") == nil)
  }

  @Test func kerfsThemeSelectsWithTheSystemsColour() {
    for dark in [false, true] {
      let theme = EditorTheme.kerf(dark: dark)
      #expect(theme.selection == EditorTheme.systemSelection(dark: dark))
      #expect(theme.selection != theme.background)
    }
    #expect(EditorTheme.systemSelection(dark: false) != EditorTheme.systemSelection(dark: true))
  }

  @Test func kerfsThemeUsesAFamilyKerfsFontDatabaseCanFind() {
    // AppKit's virtual ".AppleSystemUIFontMonospaced" is in no font file.
    let family = EditorTheme.kerf(dark: false).fontFamily
    #expect(family != nil)
    #expect(family?.hasPrefix(".AppleSystemUI") == false)
  }
}
