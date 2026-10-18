// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

import CKerf
import Foundation

/// A Kerf call that did not return `KERF_STATUS_OK`.
public struct KerfError: LocalizedError, Equatable {
  public let status: KerfStatus

  public var errorDescription: String? {
    switch status {
    case KERF_STATUS_INVALID_UTF8: "The file is not UTF-8 text."
    case KERF_STATUS_TOO_LARGE: "The file is too large."
    case KERF_STATUS_IO: "The file could not be read."
    case KERF_STATUS_GPU: "No GPU is available to draw the editor."
    default: "Kerf failed (status \(status.rawValue))."
    }
  }
}

func check(_ status: KerfStatus) throws {
  if status != KERF_STATUS_OK {
    throw KerfError(status: status)
  }
}

func unwrap(_ handle: OpaquePointer?) throws -> OpaquePointer {
  guard let handle else { throw KerfError(status: KERF_STATUS_NULL_POINTER) }
  return handle
}

/// One buffer and its cursor in Kerf's core. Like the handle it owns, it is
/// used from one thread: the main thread in the app.
public final class Editor {
  public enum Command {
    case backspace, left, right, undo, redo, selectLeft, selectRight

    var code: UInt32 {
      switch self {
      case .backspace: UInt32(KERF_COMMAND_BACKSPACE)
      case .left: UInt32(KERF_COMMAND_LEFT)
      case .right: UInt32(KERF_COMMAND_RIGHT)
      case .undo: UInt32(KERF_COMMAND_UNDO)
      case .redo: UInt32(KERF_COMMAND_REDO)
      case .selectLeft: UInt32(KERF_COMMAND_SELECT_LEFT)
      case .selectRight: UInt32(KERF_COMMAND_SELECT_RIGHT)
      }
    }
  }

  let handle: OpaquePointer

  public init() throws {
    var out: OpaquePointer?
    try check(kerf_editor_new(&out))
    handle = try unwrap(out)
  }

  public init(contentsOf url: URL) throws {
    var out: OpaquePointer?
    let path = Array(url.path(percentEncoded: false).utf8)
    try check(kerf_editor_open(path, path.count, &out))
    handle = try unwrap(out)
  }

  deinit {
    kerf_editor_free(handle)
  }

  /// Typing or a paste, replacing the selection.
  public func insert(_ text: String) throws {
    let bytes = Array(text.utf8)
    try check(kerf_editor_insert(handle, bytes, bytes.count))
  }

  /// Runs `command`; false when it had nothing to do.
  @discardableResult
  public func run(_ command: Command) throws -> Bool {
    let status = kerf_editor_command(handle, command.code)
    if status == KERF_STATUS_NOTHING { return false }
    try check(status)
    return true
  }

  public func text() throws -> String {
    var length = 0
    let probe = kerf_editor_copy_text(handle, nil, 0, &length)
    if probe == KERF_STATUS_OK { return "" }
    guard probe == KERF_STATUS_TOO_LARGE else { throw KerfError(status: probe) }
    var bytes = [UInt8](repeating: 0, count: length)
    try check(kerf_editor_copy_text(handle, &bytes, bytes.count, &length))
    return String(decoding: bytes, as: UTF8.self)
  }

  /// The cursor as a UTF-8 byte offset into `text()`.
  public func cursor() throws -> Int {
    var out = 0
    try check(kerf_editor_cursor(handle, &out))
    return out
  }

  /// The selection's fixed end and the cursor as UTF-8 byte offsets; equal
  /// when nothing is selected.
  public func selection() throws -> (anchor: Int, cursor: Int) {
    var (anchor, cursor) = (0, 0)
    try check(kerf_editor_selection(handle, &anchor, &cursor))
    return (anchor, cursor)
  }

  /// Selects from `anchor` to `cursor`, UTF-8 byte offsets in either order.
  public func select(anchor: Int, cursor: Int) throws {
    guard anchor >= 0, cursor >= 0 else { throw KerfError(status: KERF_STATUS_OUT_OF_RANGE) }
    try check(kerf_editor_select(handle, anchor, cursor))
  }
}
