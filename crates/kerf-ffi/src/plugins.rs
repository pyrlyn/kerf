// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The plugins settings screen and the plugin permission sheet
//! (`kerf-plugin-manager`): a host lists rows, shows a row's grant lines and
//! digest, and sends back Allow or Don't allow.

use std::path::PathBuf;

use kerf_plugin_manager::{Decision, Error, Grants, Manager, Row, Source, State, display};

use crate::{KerfStatus, guard, put, text};

/// Longest path or text a host passes to these calls.
const MAX_PATH_BYTES: usize = 64 * 1024;

/// The plugin's id.
pub const KERF_PLUGIN_TEXT_ID: u32 = 0;
/// The manifest's display name.
pub const KERF_PLUGIN_TEXT_NAME: u32 = 1;
/// The manifest version.
pub const KERF_PLUGIN_TEXT_VERSION: u32 = 2;
/// The package digest, 64 hex characters.
pub const KERF_PLUGIN_TEXT_DIGEST: u32 = 3;
/// Why it is invalid or does not match the lockfile; empty otherwise.
pub const KERF_PLUGIN_TEXT_REASON: u32 = 4;
/// The version `kerf-plugins.lock` pins; empty when not pinned.
pub const KERF_PLUGIN_TEXT_LOCK_VERSION: u32 = 5;
/// The digest `kerf-plugins.lock` pins; empty when not pinned.
pub const KERF_PLUGIN_TEXT_LOCK_DIGEST: u32 = 6;
/// Grant line `item` of `KerfPluginRow.lines`, as the prompt shows it.
pub const KERF_PLUGIN_TEXT_LINE: u32 = 7;
/// Line `item` of `KerfPluginRow.added`: asked for and not granted.
pub const KERF_PLUGIN_TEXT_ADDED: u32 = 8;
/// Line `item` of `KerfPluginRow.removed`: granted, no longer asked for.
pub const KERF_PLUGIN_TEXT_REMOVED: u32 = 9;
/// Export `item` of `KerfPluginRow.disabled_exports`.
pub const KERF_PLUGIN_TEXT_DISABLED_EXPORT: u32 = 10;

/// Where a plugin stands.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KerfPluginState {
    /// Granted for these bytes; it loads.
    Enabled = 0,
    /// Enabled, but some exports failed three times in a row and stay off
    /// for the session.
    ExportsDisabled = 1,
    /// Not granted for these bytes, or it asks for more: show the prompt.
    NeedsApproval = 2,
    /// A project plugin not enabled for this repository: show the prompt.
    ProjectNotEnabled = 3,
    /// Does not match `kerf-plugins.lock`; off until reviewed.
    LockMismatch = 4,
    /// The manifest was refused; nothing enables it.
    Invalid = 5,
}

/// One plugin's numbers; its texts come from `kerf_plugins_copy_text`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KerfPluginRow {
    /// Where it stands.
    pub state: KerfPluginState,
    /// Non-zero for a project plugin, zero for a user plugin.
    pub project: u8,
    /// Non-zero when `kerf-plugins.lock` pins this id.
    pub locked: u8,
    /// The linear memory cap it runs under, in MiB.
    pub memory_mib: u32,
    /// Grant lines it asks for (`KERF_PLUGIN_TEXT_LINE`).
    pub lines: usize,
    /// Lines the prompt asks for anew (`KERF_PLUGIN_TEXT_ADDED`).
    pub added: usize,
    /// Lines granted and no longer asked for (`KERF_PLUGIN_TEXT_REMOVED`).
    pub removed: usize,
    /// Exports off for the session (`KERF_PLUGIN_TEXT_DISABLED_EXPORT`).
    pub disabled_exports: usize,
}

/// The plugins of one session. Opaque to the host.
pub struct KerfPlugins {
    manager: Manager,
}

impl From<Error> for KerfStatus {
    fn from(error: Error) -> Self {
        match error {
            Error::NoPlugin(_) => Self::OutOfRange,
            Error::NotAskable(_) | Error::StaleDigest(_) | Error::NoProject => Self::Unsupported,
            Error::TooManyGrants { .. } => Self::TooLarge,
            Error::Lockfile(_) => Self::Io,
        }
    }
}

/// The lists behind a row's counts, in `KERF_PLUGIN_TEXT_*` order.
fn lists(row: &Row) -> [&[String]; 4] {
    let (added, removed, off): (&[String], &[String], &[String]) = match &row.state {
        State::NeedsApproval { added, removed } => (added, removed, &[]),
        State::ProjectNotEnabled => (&row.lines, &[], &[]),
        State::ExportsDisabled(exports) => (&[], &[], exports),
        _ => (&[], &[], &[]),
    };
    [&row.lines, added, removed, off]
}

/// The manager behind a host handle.
///
/// # Safety
///
/// `p` is null or a live handle from `kerf_plugins_open` that no other call
/// is using.
unsafe fn manager<'a>(p: *mut KerfPlugins) -> Result<&'a mut Manager, KerfStatus> {
    // SAFETY: the caller promised a live, unaliased handle or null.
    unsafe { p.as_mut() }
        .map(|p| &mut p.manager)
        .ok_or(KerfStatus::NullPointer)
}

/// Discover the plugins under `home` (Kerf's home, UTF-8) and the project at
/// `project` (UTF-8; `project_len` 0 for none), read the project's
/// `kerf-plugins.lock`, and put a new handle in `*out`. Nothing is granted
/// yet. Free it with `kerf_plugins_free`.
///
/// # Safety
///
/// `home` points to `home_len` readable bytes, `project` to `project_len`;
/// `out` is valid for a write of one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_open(
    home: *const u8,
    home_len: usize,
    project: *const u8,
    project_len: usize,
    out: *mut *mut KerfPlugins,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (home, project) = unsafe {
            (
                text(home, home_len, MAX_PATH_BYTES)?,
                text(project, project_len, MAX_PATH_BYTES)?,
            )
        };
        if home.is_empty() {
            return Err(KerfStatus::NullPointer);
        }
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let project = (!project.is_empty()).then(|| PathBuf::from(project));
        let manager = Manager::open(PathBuf::from(home), project, Grants::default());
        let handle = Box::into_raw(Box::new(KerfPlugins { manager }));
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, handle) }
    })
}

/// Free a handle. Null is ignored.
///
/// # Safety
///
/// `p` is null or a live handle not used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_free(p: *mut KerfPlugins) {
    if !p.is_null() {
        // SAFETY: the handle came from `Box::into_raw` and is not used again.
        let p = unsafe { Box::from_raw(p) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(p)));
    }
}

/// Read the plugin folders and the lockfile again, after an install, an
/// update or a lockfile change. Grants and this session's failures stay;
/// row indices may change.
///
/// # Safety
///
/// `p` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_rescan(p: *mut KerfPlugins) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    guard(|| unsafe { manager(p) }.map(Manager::rescan))
}

/// The number of plugins, rows `0..count` sorted by id.
///
/// # Safety
///
/// `p` is a live handle; `out` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_count(p: *mut KerfPlugins, out: *mut usize) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    guard(|| unsafe { put(out, manager(p)?.len()) })
}

/// Row `index` in `*out`; `KERF_STATUS_OUT_OF_RANGE` past the last.
///
/// # Safety
///
/// `p` is a live handle; `out` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_row(
    p: *mut KerfPlugins,
    index: usize,
    out: *mut KerfPluginRow,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let row = unsafe { manager(p) }?
            .row(index)
            .ok_or(KerfStatus::OutOfRange)?;
        let [lines, added, removed, off] = lists(&row).map(<[String]>::len);
        let state = match row.state {
            State::Enabled => KerfPluginState::Enabled,
            State::ExportsDisabled(_) => KerfPluginState::ExportsDisabled,
            State::NeedsApproval { .. } => KerfPluginState::NeedsApproval,
            State::ProjectNotEnabled => KerfPluginState::ProjectNotEnabled,
            State::LockMismatch(_) => KerfPluginState::LockMismatch,
            State::Invalid(_) => KerfPluginState::Invalid,
        };
        let value = KerfPluginRow {
            state,
            project: u8::from(row.source == Source::Project),
            locked: u8::from(row.lock.is_some()),
            memory_mib: row.memory_mib,
            lines,
            added,
            removed,
            disabled_exports: off,
        };
        // SAFETY: forwarded from the caller.
        unsafe { put(out, value) }
    })
}

/// Copy text `field` (a `KERF_PLUGIN_TEXT_*`) of row `index` as UTF-8 into
/// `buf` (`cap` bytes, no NUL written) and its length into `*out_len`; `item`
/// picks the entry of a list field and is ignored otherwise. When `cap` is
/// too small, nothing is copied, `*out_len` still gets the length needed and
/// the status is `KERF_STATUS_TOO_LARGE`; a null `buf` with `cap` 0 asks for
/// the length. Plugin-written text arrives with control and direction
/// characters escaped.
///
/// # Safety
///
/// `p` is a live handle; `buf` is null or valid for `cap` bytes of writes;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_copy_text(
    p: *mut KerfPlugins,
    index: usize,
    field: u32,
    item: usize,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let row = unsafe { manager(p) }?
            .row(index)
            .ok_or(KerfStatus::OutOfRange)?;
        let lock = row.lock.as_ref();
        let value = match field {
            KERF_PLUGIN_TEXT_ID => display(&row.id),
            KERF_PLUGIN_TEXT_NAME => row.name.clone(),
            KERF_PLUGIN_TEXT_VERSION => row.version.clone(),
            KERF_PLUGIN_TEXT_DIGEST => row.digest.clone(),
            KERF_PLUGIN_TEXT_REASON => match &row.state {
                State::Invalid(reason) | State::LockMismatch(reason) => reason.clone(),
                _ => String::new(),
            },
            KERF_PLUGIN_TEXT_LOCK_VERSION => lock.map(|e| e.version.clone()).unwrap_or_default(),
            KERF_PLUGIN_TEXT_LOCK_DIGEST => lock.map(|e| e.digest.to_string()).unwrap_or_default(),
            KERF_PLUGIN_TEXT_LINE..=KERF_PLUGIN_TEXT_DISABLED_EXPORT => {
                let list = lists(&row)[(field - KERF_PLUGIN_TEXT_LINE) as usize];
                list.get(item).ok_or(KerfStatus::OutOfRange)?.clone()
            }
            _ => return Err(KerfStatus::Unsupported),
        };
        // SAFETY: forwarded from the caller.
        unsafe { put(out_len, value.len()) }?;
        if value.len() > cap {
            return Err(KerfStatus::TooLarge);
        }
        if value.is_empty() {
            return Ok(());
        }
        if buf.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        // SAFETY: `buf` is writable for `cap >= value.len()` bytes and cannot
        // overlap a `String` Kerf just allocated.
        unsafe { std::ptr::copy_nonoverlapping(value.as_ptr(), buf, value.len()) };
        Ok(())
    })
}

/// Answer the permission prompt for row `index`: non-zero `allow` grants
/// every line for `digest` (the `KERF_PLUGIN_TEXT_DIGEST` the prompt showed),
/// zero grants nothing. `KERF_STATUS_UNSUPPORTED`, and nothing granted, when
/// the row is invalid or does not match the lockfile, or its digest is no
/// longer `digest`: read the row again and show the prompt anew.
///
/// # Safety
///
/// `p` is a live handle; `digest` points to `digest_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_decide(
    p: *mut KerfPlugins,
    index: usize,
    digest: *const u8,
    digest_len: usize,
    allow: u8,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (manager, digest) = unsafe { (manager(p)?, text(digest, digest_len, MAX_PATH_BYTES)?) };
        let decision = if allow != 0 {
            Decision::Allow
        } else {
            Decision::Deny
        };
        Ok(manager.decide(index, digest, decision)?)
    })
}

/// Turn row `index` off: its grant is cleared, so it asks before it loads
/// again.
///
/// # Safety
///
/// `p` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_revoke(p: *mut KerfPlugins, index: usize) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    guard(|| Ok(unsafe { manager(p) }?.revoke(index)?))
}

/// The user reviewed row `index`'s lockfile mismatch and accepts these
/// bytes: pin its version and digest in the project's `kerf-plugins.lock`.
/// The grant is still checked, so new bytes then ask for permission.
/// `KERF_STATUS_IO` when the lockfile cannot be read or written.
///
/// # Safety
///
/// `p` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_accept_lock(p: *mut KerfPlugins, index: usize) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    guard(|| Ok(unsafe { manager(p) }?.accept_lock(index)?))
}

/// Record a call to `export` (UTF-8) of row `index`: non-zero `ok` for a
/// success. The third failure in a row turns that export off for the
/// session.
///
/// # Safety
///
/// `p` is a live handle; `export` points to `export_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_plugins_record_call(
    p: *mut KerfPlugins,
    index: usize,
    export: *const u8,
    export_len: usize,
    ok: u8,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (manager, export) = unsafe { (manager(p)?, text(export, export_len, MAX_PATH_BYTES)?) };
        let id = manager.row(index).ok_or(KerfStatus::OutOfRange)?.id;
        manager.record_call(&id, export, ok != 0);
        Ok(())
    })
}
