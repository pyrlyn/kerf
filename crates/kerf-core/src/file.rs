// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Reading a text file with a size cap: every file Kerf opens (a buffer, the
//! trust file, a file a native host or a remote client names) is untrusted
//! input, so none is loaded whole before its size is known.

use std::io::{self, Read};
use std::path::Path;

/// Why a file was not read as text.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// Opening or reading failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// The file is larger than the cap.
    #[error("file is larger than {cap} bytes")]
    TooLarge {
        /// The cap in bytes.
        cap: u64,
    },
    /// The file is not UTF-8.
    #[error("file is not UTF-8")]
    NotUtf8,
}

/// The file at `path` as UTF-8 text of at most `cap` bytes.
pub fn read_text(path: &Path, cap: u64) -> Result<String, ReadError> {
    // One byte past the cap tells "exactly at the cap" from "over it"
    // without reading an attacker-sized file into memory.
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > cap {
        return Err(ReadError::TooLarge { cap });
    }
    String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8)
}

#[cfg(test)]
mod tests {
    use super::{ReadError, read_text};

    #[test]
    fn reads_up_to_the_cap_and_refuses_past_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "héllo").unwrap();
        assert_eq!(read_text(&path, 6).unwrap(), "héllo");
        assert!(matches!(
            read_text(&path, 5),
            Err(ReadError::TooLarge { cap: 5 })
        ));
    }

    #[test]
    fn refuses_bytes_that_are_not_utf8_and_reports_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bin");
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(matches!(read_text(&path, 10), Err(ReadError::NotUtf8)));
        let missing = read_text(&dir.path().join("nope"), 10);
        assert!(
            matches!(missing, Err(ReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound)
        );
    }
}
