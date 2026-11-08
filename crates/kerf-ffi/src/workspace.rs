// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Welcome screen and Workspace Trust model (`kerf-workspace`) for the
//! native hosts. Actions are calls; state comes back as one JSON snapshot,
//! so every host decodes the same shape instead of walking per-field
//! accessors.

use std::path::{Path, PathBuf};

use kerf_core::TrustError;
use kerf_workspace::{Config, Mode, WorkspaceError, Workspaces};

use crate::{KerfStatus, guard, put, text};

/// Longest path a host can pass.
const MAX_PATH_BYTES: usize = 64 * 1024;

/// Recent folders, trust and open folders. Opaque to the host.
pub struct KerfWorkspace {
    workspaces: Workspaces,
    /// The last revoke's report, JSON, until the next revoke.
    stop: String,
}

/// The model behind a host handle.
///
/// # Safety
///
/// `ws` is null or a live handle from `kerf_workspace_new` that no other
/// call is using.
unsafe fn handle<'a>(ws: *mut KerfWorkspace) -> Result<&'a mut KerfWorkspace, KerfStatus> {
    // SAFETY: the caller promised a live, unaliased handle or null.
    unsafe { ws.as_mut() }.ok_or(KerfStatus::NullPointer)
}

/// The host's UTF-8 path; empty is refused.
///
/// # Safety
///
/// As for `text`.
unsafe fn path<'a>(ptr: *const u8, len: usize) -> Result<&'a Path, KerfStatus> {
    // SAFETY: forwarded from the caller.
    let path = unsafe { text(ptr, len, MAX_PATH_BYTES) }?;
    if path.is_empty() {
        return Err(KerfStatus::NullPointer);
    }
    Ok(Path::new(path))
}

fn status(e: &WorkspaceError) -> KerfStatus {
    match e {
        WorkspaceError::Trust(TrustError::Unresolved { .. }) => KerfStatus::Io,
        WorkspaceError::Trust(TrustError::TooManyRoots { .. })
        | WorkspaceError::TooManyFolders { .. } => KerfStatus::TooLarge,
        WorkspaceError::Trust(TrustError::Untrusted { .. }) | WorkspaceError::InvalidHost(_) => {
            KerfStatus::Unsupported
        }
    }
}

fn json(value: &impl serde::Serialize) -> Result<String, KerfStatus> {
    // Only a non-UTF-8 path fails to encode.
    serde_json::to_string(value).map_err(|_| KerfStatus::InvalidUtf8)
}

/// Copies `bytes` out the way `kerf_editor_copy_text` does.
///
/// # Safety
///
/// `buf` is null or valid for `cap` bytes of writes; `out_len` is valid for
/// a write.
unsafe fn copy_out(
    bytes: &[u8],
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> Result<(), KerfStatus> {
    // SAFETY: forwarded from the caller.
    unsafe { put(out_len, bytes.len()) }?;
    if bytes.len() > cap {
        return Err(KerfStatus::TooLarge);
    }
    if bytes.is_empty() {
        return Ok(());
    }
    if buf.is_null() {
        return Err(KerfStatus::NullPointer);
    }
    // SAFETY: `buf` is writable for `cap >= bytes.len()` bytes and cannot
    // overlap a string Kerf owns.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len()) };
    Ok(())
}

/// A workspace model in `*out`, reading `trust.toml` and `recent.toml` from
/// `config_dir` (empty: Kerf's folder in the OS config directory) and user
/// plugins from `<plugin_home>/plugins/`. Damaged files become notices in
/// the state, never a failure; a relative folder is `KERF_STATUS_IO`. Free
/// it with `kerf_workspace_free`.
///
/// # Safety
///
/// `config_dir` points to `config_dir_len` readable bytes, `plugin_home` to
/// `plugin_home_len`; `out` is valid for a write of one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_new(
    config_dir: *const u8,
    config_dir_len: usize,
    plugin_home: *const u8,
    plugin_home_len: usize,
    out: *mut *mut KerfWorkspace,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (dir, home) = unsafe {
            (
                text(config_dir, config_dir_len, MAX_PATH_BYTES)?,
                path(plugin_home, plugin_home_len)?,
            )
        };
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let dir = match dir {
            "" => Config::default_dir().ok_or(KerfStatus::Io)?,
            dir => PathBuf::from(dir),
        };
        // A relative folder would resolve against wherever the host runs,
        // which may be a repository.
        if !dir.is_absolute() || !home.is_absolute() {
            return Err(KerfStatus::Io);
        }
        let workspaces = Workspaces::load(Config::in_dir(&dir, home.to_path_buf()));
        let ws = Box::into_raw(Box::new(KerfWorkspace {
            workspaces,
            stop: String::new(),
        }));
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, ws) }
    })
}

/// Free a workspace model. Null is ignored.
///
/// # Safety
///
/// `ws` is null or a live handle not used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_free(ws: *mut KerfWorkspace) {
    if !ws.is_null() {
        // SAFETY: the handle came from `Box::into_raw` and is not used again.
        let ws = unsafe { Box::from_raw(ws) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(ws)));
    }
}

/// Open the folder at `path` and remember it; `*out_trusted` is 1 when it
/// opened trusted, 0 when restricted (the state then asks).
/// `KERF_STATUS_IO` when the folder cannot be resolved,
/// `KERF_STATUS_TOO_LARGE` when too many folders are open.
///
/// # Safety
///
/// `ws` is a live handle; `path` points to `path_len` readable bytes;
/// `out_trusted` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_open(
    ws: *mut KerfWorkspace,
    path: *const u8,
    path_len: usize,
    out_trusted: *mut u8,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (ws, path) = unsafe { (handle(ws)?, self::path(path, path_len)?) };
        if out_trusted.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let folder = ws.workspaces.open(path).map_err(|e| status(&e))?;
        let trusted = u8::from(folder.mode == Mode::Trusted);
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out_trusted, trusted) }
    })
}

/// The user's "Trust folder" for `path`.
///
/// # Safety
///
/// `ws` is a live handle; `path` points to `path_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_trust(
    ws: *mut KerfWorkspace,
    path: *const u8,
    path_len: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (ws, path) = unsafe { (handle(ws)?, self::path(path, path_len)?) };
        ws.workspaces.trust(path).map(drop).map_err(|e| status(&e))
    })
}

/// The user's "Stay restricted" for the open folder `path`.
/// `KERF_STATUS_NOTHING` when it is not open.
///
/// # Safety
///
/// `ws` is a live handle; `path` points to `path_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_stay_restricted(
    ws: *mut KerfWorkspace,
    path: *const u8,
    path_len: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (ws, path) = unsafe { (handle(ws)?, self::path(path, path_len)?) };
        ws.workspaces
            .stay_restricted(path)
            .then_some(())
            .ok_or(KerfStatus::Nothing)
    })
}

/// Revoke trust in `path` and every trusted root inside it.
/// `*out_stop` gets the number of open folders that lost trust; what each
/// must stop is in `kerf_workspace_stop_json` until the next revoke.
///
/// # Safety
///
/// `ws` is a live handle; `path` points to `path_len` readable bytes;
/// `out_stop` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_revoke(
    ws: *mut KerfWorkspace,
    path: *const u8,
    path_len: usize,
    out_stop: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (ws, path) = unsafe { (handle(ws)?, self::path(path, path_len)?) };
        if out_stop.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let revoked = ws.workspaces.revoke(path);
        ws.stop = json(&revoked)?;
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out_stop, revoked.stop.len()) }
    })
}

/// Close the open folder `path`. `KERF_STATUS_NOTHING` when it is not open.
///
/// # Safety
///
/// `ws` is a live handle; `path` points to `path_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_close(
    ws: *mut KerfWorkspace,
    path: *const u8,
    path_len: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (ws, path) = unsafe { (handle(ws)?, self::path(path, path_len)?) };
        ws.workspaces
            .close(path)
            .then_some(())
            .ok_or(KerfStatus::Nothing)
    })
}

/// Remove `path` from the recent folders. `KERF_STATUS_NOTHING` when it
/// was not listed.
///
/// # Safety
///
/// `ws` is a live handle; `path` points to `path_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_forget(
    ws: *mut KerfWorkspace,
    path: *const u8,
    path_len: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (ws, path) = unsafe { (handle(ws)?, self::path(path, path_len)?) };
        ws.workspaces
            .forget(path)
            .then_some(())
            .ok_or(KerfStatus::Nothing)
    })
}

/// Copy the state as UTF-8 JSON into `buf`: `recent` (path, host, chip
/// `trusted` / `restricted` / `remote`), `folders` (root, mode, asking, and
/// the prompt's items with kind, plugin, name, command and args verbatim)
/// and `notices`. Sizing as for `kerf_editor_copy_text`.
///
/// # Safety
///
/// `ws` is a live handle; `buf` is null or valid for `cap` bytes of writes;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_state_json(
    ws: *mut KerfWorkspace,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let ws = unsafe { handle(ws) }?;
        let state = json(&ws.workspaces.snapshot())?;
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(state.as_bytes(), buf, cap, out_len) }
    })
}

/// Copy the last revoke's report as UTF-8 JSON: `roots` removed and `stop`,
/// one entry per folder with the items to stop. Empty before any revoke.
/// Sizing as for `kerf_editor_copy_text`.
///
/// # Safety
///
/// `ws` is a live handle; `buf` is null or valid for `cap` bytes of writes;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_workspace_stop_json(
    ws: *mut KerfWorkspace,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let ws = unsafe { handle(ws) }?;
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(ws.stop.as_bytes(), buf, cap, out_len) }
    })
}
