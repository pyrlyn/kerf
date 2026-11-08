// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

// The "Kerf color" and "Kerf size" variables of the Figma UI Kit
// (file gNfemQ9NFKFNT2fifxb6sm), copied by hand: the views use these names
// so a change in Figma maps to one line here. Only the Dark mode exists.

import AppKit

extension NSColor {
  /// A colour written `0xRRGGBBAA`, as Figma shows its variables.
  public convenience init(kerfHex v: UInt32) {
    let channel = { (shift: UInt32) in CGFloat((v >> shift) & 0xff) / 255 }
    self.init(srgbRed: channel(24), green: channel(16), blue: channel(8), alpha: channel(0))
  }
}

/// Colour tokens, grouped as the Figma collection groups them.
public enum KerfColor {
  public enum Background {
    public static let window = NSColor(kerfHex: 0x171a_22e6)
    public static let editor = NSColor(kerfHex: 0x1f1f_24ff)
    public static let chrome = NSColor(kerfHex: 0x1b1d_22ff)
    public static let glass = NSColor(kerfHex: 0x292e_3a6b)
    public static let elevated = NSColor(kerfHex: 0x262b_37f7)
    public static let scrim = NSColor(kerfHex: 0x0507_0bb3)
    public static let terminal = NSColor(kerfHex: 0x0a12_14ff)
    public static let terminalChrome = NSColor(kerfHex: 0x0f19_1cff)
    public static let fill = NSColor(kerfHex: 0xffff_ff14)
    public static let fillMedium = NSColor(kerfHex: 0xffff_ff1f)
    public static let fillStrong = NSColor(kerfHex: 0xffff_ff24)
    public static let fillSubtle = NSColor(kerfHex: 0xffff_ff0d)
    public static let switchOff = NSColor(kerfHex: 0xffff_ff2e)
    public static let selection = NSColor(kerfHex: 0x0a84_ff3d)
    public static let lineCurrent = NSColor(kerfHex: 0x2a2c_33ff)
    public static let lineError = NSColor(kerfHex: 0xff75_841f)
    public static let lineWarning = NSColor(kerfHex: 0xf2c1_4e1a)
    public static let lineProgramCounter = NSColor(kerfHex: 0x4fd9_8f24)
    public static let lineAdded = NSColor(kerfHex: 0x4fd9_8f1f)
  }

  public enum Border {
    public static let hairline = NSColor(kerfHex: 0xffff_ff14)
    public static let subtle = NSColor(kerfHex: 0xffff_ff1f)
    public static let strong = NSColor(kerfHex: 0xffff_ff29)
  }

  public enum Text {
    public static let primary = NSColor(kerfHex: 0xeef2_f8ff)
    public static let secondary = NSColor(kerfHex: 0xa3ad_bfff)
    public static let tertiary = NSColor(kerfHex: 0xc9d1_deff)
    public static let code = NSColor(kerfHex: 0xdfdf_e0ff)
    public static let lineNumber = NSColor(kerfHex: 0x8a8a_90ff)
    public static let ghost = NSColor(kerfHex: 0x8a8f_99ff)
    public static let terminal = NSColor(kerfHex: 0xe8f4_f1ff)
    public static let terminalDim = NSColor(kerfHex: 0x8fa3_a8ff)
    public static let onAccent = NSColor(kerfHex: 0xffff_ffff)
    public static let fileIcon = NSColor(kerfHex: 0xffa8_60ff)
  }

  public enum Accent {
    public static let `default` = NSColor(kerfHex: 0x0873_e0ff)
    public static let ink = NSColor(kerfHex: 0x6cb6_ffff)
  }

  public enum Mark {
    public static let mint = NSColor(kerfHex: 0x3ddc_b0ff)
  }

  public enum Status {
    public static let ok = NSColor(kerfHex: 0x4fd9_8fff)
    public static let okBackground = NSColor(kerfHex: 0x4fd9_8f2e)
    public static let warning = NSColor(kerfHex: 0xf2c1_4eff)
    public static let warningBackground = NSColor(kerfHex: 0xf2c1_4e2e)
    public static let error = NSColor(kerfHex: 0xff8e_9aff)
    public static let errorBackground = NSColor(kerfHex: 0xff75_842e)
    public static let busy = NSColor(kerfHex: 0xbcb2_ffff)
    public static let busyBackground = NSColor(kerfHex: 0xaea2_ff2e)
  }

  public enum Banner {
    public static let warningBackground = NSColor(kerfHex: 0x3a2f_12ff)
    public static let warningInk = NSColor(kerfHex: 0xffe6_a3ff)
    public static let errorBackground = NSColor(kerfHex: 0x3d18_20ff)
    public static let errorInk = NSColor(kerfHex: 0xffd5_daff)
    public static let infoBackground = NSColor(kerfHex: 0x0f2a_44ff)
    public static let infoInk = NSColor(kerfHex: 0xcfe5_ffff)
  }

  public enum Issue {
    public static let errorBackground = NSColor(kerfHex: 0x5a1f_2aff)
    public static let warningBackground = NSColor(kerfHex: 0x4d3d_12ff)
  }

  public enum Syntax {
    public static let keyword = NSColor(kerfHex: 0xfc5f_a3ff)
    public static let comment = NSColor(kerfHex: 0x7f8c_98ff)
    public static let type = NSColor(kerfHex: 0x5dd8_ffff)
    public static let function = NSColor(kerfHex: 0x67b7_a4ff)
    public static let number = NSColor(kerfHex: 0xd0bf_69ff)
    public static let string = NSColor(kerfHex: 0xfc6a_5dff)
  }

  public enum WindowControl {
    public static let close = NSColor(kerfHex: 0xff5f_57ff)
    public static let minimize = NSColor(kerfHex: 0xfebc_2eff)
    public static let zoom = NSColor(kerfHex: 0x28c8_40ff)
  }
}

/// Spacing tokens; Figma's `2xl` and up become `xl2` and up.
public enum KerfSpace {
  public static let xxs: CGFloat = 2
  public static let xs: CGFloat = 4
  public static let sm: CGFloat = 6
  public static let md: CGFloat = 8
  public static let lg: CGFloat = 10
  public static let xl: CGFloat = 12
  public static let xl2: CGFloat = 14
  public static let xl3: CGFloat = 16
  public static let xl4: CGFloat = 28
}

/// Corner radius tokens.
public enum KerfRadius {
  public static let sm: CGFloat = 6
  public static let md: CGFloat = 8
  public static let lg: CGFloat = 14
  public static let xl: CGFloat = 16
  public static let xl2: CGFloat = 20
  public static let window: CGFloat = 26
  /// A pill: views clamp it to half their height.
  public static let full: CGFloat = 999
}
