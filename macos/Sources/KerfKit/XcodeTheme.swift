// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import AppKit
import CKerf

/// An sRGB colour with 8-bit channels, as Kerf takes it.
public struct RGBA: Equatable, Sendable {
  public var r, g, b, a: UInt8

  public init(_ r: UInt8, _ g: UInt8, _ b: UInt8, _ a: UInt8 = 255) {
    (self.r, self.g, self.b, self.a) = (r, g, b, a)
  }

  var tuple: (UInt8, UInt8, UInt8, UInt8) { (r, g, b, a) }
}

/// The editor's font and colours. Kerf hard-codes none; the host picks them.
public struct EditorTheme: Equatable, Sendable {
  /// The family as the font file names it, which is the name Kerf's font
  /// database looks up (the system's SF Mono is ".SF NS Mono").
  public var fontFamily: String?
  public var fontSize: CGFloat
  public var lineHeight: CGFloat
  public var background: RGBA
  public var text: RGBA
  public var selection: RGBA

  /// Kerf's own Light and Dark themes, for a Mac without Xcode.
  public static func kerf(dark: Bool) -> EditorTheme {
    let font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    return EditorTheme(
      fontFamily: XcodeTheme.fileFamily(of: font),
      fontSize: 12,
      lineHeight: NSLayoutManager().defaultLineHeight(for: font),
      background: dark ? RGBA(31, 31, 36) : RGBA(255, 255, 255),
      text: dark ? RGBA(223, 223, 224) : RGBA(29, 29, 31),
      selection: systemSelection(dark: dark)
    )
  }

  /// The system's selected-text background for this appearance, as every
  /// Mac text view fills it; it follows the user's highlight colour.
  static func systemSelection(dark: Bool) -> RGBA {
    var rgba = dark ? RGBA(63, 99, 139) : RGBA(179, 215, 255)
    NSAppearance(named: dark ? .darkAqua : .aqua)?.performAsCurrentDrawingAppearance {
      guard let c = NSColor.selectedTextBackgroundColor.usingColorSpace(.sRGB) else { return }
      let byte = { (v: CGFloat) in UInt8((min(max(v, 0), 1) * 255).rounded()) }
      rgba = RGBA(
        byte(c.redComponent), byte(c.greenComponent), byte(c.blueComponent),
        byte(c.alphaComponent))
    }
    return rgba
  }

  /// Calls `body` with this theme as a `KerfTheme`; the caret is the
  /// system's, so Kerf draws none.
  func withKerfTheme<R>(_ body: (UnsafePointer<KerfTheme>) throws -> R) rethrows -> R {
    try Array((fontFamily ?? "").utf8).withUnsafeBufferPointer { family in
      var theme = KerfTheme(
        font_family: family.baseAddress,
        font_family_len: family.count,
        font_size: Float(fontSize),
        line_height: Float(lineHeight),
        background: background.tuple,
        text: text.tuple,
        selection: selection.tuple,
        caret: (0, 0, 0, 0),
        caret_width: 1,
        draw_caret: 0
      )
      return try body(&theme)
    }
  }
}

/// Xcode's editor theme, read from Xcode's own files at run time and never
/// copied. The format is undocumented (`research.md` §7), so anything
/// missing or malformed falls back to Kerf's theme.
public enum XcodeTheme {
  /// The theme Xcode uses for this appearance.
  public static func current(
    dark: Bool,
    defaults: UserDefaults? = UserDefaults(suiteName: "com.apple.dt.Xcode"),
    folders: [URL] = themeFolders()
  ) -> EditorTheme {
    let key = dark ? "XCFontAndColorCurrentDarkTheme" : "XCFontAndColorCurrentTheme"
    let name =
      defaults?.string(forKey: key)
      ?? (dark ? "Default (Dark).xccolortheme" : "Default (Light).xccolortheme")
    // A theme name from defaults is a file name; refuse one that walks
    // out of the theme folders.
    guard !name.contains("/") else { return .kerf(dark: dark) }
    for folder in folders {
      if let theme = load(folder.appending(path: name), dark: dark) { return theme }
    }
    return .kerf(dark: dark)
  }

  /// The user's themes first, then the ones Xcode ships.
  public static func themeFolders() -> [URL] {
    var folders = [
      FileManager.default.homeDirectoryForCurrentUser
        .appending(path: "Library/Developer/Xcode/UserData/FontAndColorThemes")
    ]
    if let xcode = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.apple.dt.Xcode")
    {
      folders.append(
        xcode.appending(
          path: "Contents/SharedFrameworks/DVTUserInterfaceKit.framework/"
            + "Versions/A/Resources/FontAndColorThemes"))
    }
    return folders
  }

  /// One `.xccolortheme` property list: background, plain text colour,
  /// selection colour and plain text font. A theme without a selection
  /// colour gets the system's for `dark`.
  public static func load(_ url: URL, dark: Bool = false) -> EditorTheme? {
    guard let data = try? Data(contentsOf: url),
      let plist = try? PropertyListSerialization.propertyList(from: data, format: nil)
        as? [String: Any],
      let background = (plist["DVTSourceTextBackground"] as? String).flatMap(color),
      let text =
        ((plist["DVTSourceTextSyntaxColors"] as? [String: String])?[
          "xcode.syntax.plain"]).flatMap(color)
    else { return nil }
    let spec = (plist["DVTSourceTextSyntaxFonts"] as? [String: String])?["xcode.syntax.plain"]
    let font = spec.flatMap(font) ?? NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    return EditorTheme(
      fontFamily: fileFamily(of: font),
      fontSize: font.pointSize,
      lineHeight: NSLayoutManager().defaultLineHeight(for: font),
      background: background,
      text: text,
      selection: (plist["DVTSourceTextSelectionColor"] as? String).flatMap(color)
        ?? EditorTheme.systemSelection(dark: dark)
    )
  }

  /// "r g b a" with each channel in 0...1.
  static func color(_ spec: String) -> RGBA? {
    let parts = spec.split(separator: " ").compactMap { Double($0) }
    guard parts.count == 4 else { return nil }
    let bytes = parts.map { UInt8((min(max($0, 0), 1) * 255).rounded()) }
    return RGBA(bytes[0], bytes[1], bytes[2], bytes[3])
  }

  /// "SFMono-Medium - 12.0": a PostScript name and a size in points. A
  /// font this Mac lacks is nil.
  static func font(_ spec: String) -> NSFont? {
    guard let split = spec.range(of: " - ", options: .backwards),
      let size = Double(spec[split.upperBound...]), size > 0
    else { return nil }
    return NSFont(name: String(spec[..<split.lowerBound]), size: size)
  }

  /// The family in the font's own `name` table (typographic family, else
  /// family), which is what Kerf's font database reads. CoreText reports a
  /// virtual name for the system monospace (".AppleSystemUIFontMonospaced")
  /// that no font file carries.
  static func fileFamily(of font: NSFont) -> String? {
    guard let table = CTFontCopyTable(font, CTFontTableTag(kCTFontTableName), []) as Data? else {
      return nil
    }
    let bytes = [UInt8](table)
    func u16(_ at: Int) -> Int? {
      at >= 0 && at + 1 < bytes.count ? Int(bytes[at]) << 8 | Int(bytes[at + 1]) : nil
    }
    guard let count = u16(2), let strings = u16(4) else { return nil }
    // Windows and Unicode names are UTF-16BE, Mac Roman names (Menlo has
    // only these) are Mac Roman; Windows wins when a font has both.
    var names: [Int: (rank: Int, name: String)] = [:]
    for record in 0..<count {
      let at = 6 + record * 12
      guard let platform = u16(at), let encoding = u16(at + 2), let id = u16(at + 6),
        id == 1 || id == 16, let length = u16(at + 8), let offset = u16(at + 10),
        strings + offset + length <= bytes.count
      else { continue }
      let (rank, utf16): (Int, Bool) =
        switch (platform, encoding) {
        case (3, _): (2, true)
        case (0, _): (1, true)
        case (1, 0): (0, false)
        default: (-1, false)
        }
      let raw = bytes[(strings + offset)..<(strings + offset + length)]
      guard rank > (names[id]?.rank ?? -1),
        let name = String(bytes: raw, encoding: utf16 ? .utf16BigEndian : .macOSRoman)
      else { continue }
      names[id] = (rank, name)
    }
    return (names[16] ?? names[1])?.name
  }
}
