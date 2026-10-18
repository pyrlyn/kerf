// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! LSP base protocol: `Content-Length` headers, then a JSON body. Every read
//! is bounded before the bytes arrive, so a server cannot make the client
//! allocate what the header promises.

use std::io::{self, BufRead, Read, Write};

use serde_json::Value;

use crate::Error;

/// Header bytes per message. Real servers send one or two short lines.
const MAX_HEADER_BYTES: u64 = 4096;

/// Reads one message. `Ok(None)` is a clean end of stream between messages.
///
/// # Errors
///
/// [`Error::TooLarge`] when the body is over `max` bytes, [`Error::Protocol`]
/// for a malformed header or body, [`Error::Io`] when the stream fails.
pub fn read_message(r: &mut impl BufRead, max: usize) -> Result<Option<Value>, Error> {
    let mut header = (&mut *r).take(MAX_HEADER_BYTES);
    let mut len = None;
    let mut line = String::new();
    loop {
        line.clear();
        if header.read_line(&mut line)? == 0 {
            if len.is_none() && header.limit() == MAX_HEADER_BYTES {
                return Ok(None);
            }
            return Err(Error::Protocol("header cut off or over 4 KiB"));
        }
        let field = line.trim_end_matches(['\r', '\n']);
        if field.is_empty() {
            break;
        }
        if let Some((name, value)) = field.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            let n = value.trim().parse::<usize>();
            len = Some(n.map_err(|_| Error::Protocol("bad Content-Length"))?);
        }
    }
    let len = len.ok_or(Error::Protocol("no Content-Length"))?;
    if len > max {
        return Err(Error::TooLarge { len, max });
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body)?;
    // serde_json stops at 128 levels of nesting, so depth is capped too.
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|_| Error::Protocol("body is not JSON"))
}

/// Writes one message with its header.
///
/// # Errors
///
/// The stream's error.
pub fn write_message(w: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = message.to_string();
    write!(w, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(bytes: &[u8]) -> Result<Option<Value>, Error> {
        read_message(&mut &bytes[..], 64)
    }

    #[test]
    fn a_written_message_reads_back() {
        let mut buf = Vec::new();
        write_message(&mut buf, &serde_json::json!({"id": 1})).unwrap();
        let mut r = &buf[..];
        assert_eq!(
            read_message(&mut r, 64).unwrap(),
            Some(serde_json::json!({"id": 1}))
        );
        assert_eq!(read_message(&mut r, 64).unwrap(), None);
    }

    #[test]
    fn hostile_headers_are_refused_before_reading_the_body() {
        let big = format!("Content-Length: {}\r\n\r\n", usize::MAX);
        assert!(matches!(read(big.as_bytes()), Err(Error::TooLarge { .. })));
        assert!(matches!(
            read(b"Content-Length: x\r\n\r\n"),
            Err(Error::Protocol(_))
        ));
        assert!(matches!(read(b"X: 1\r\n\r\n{}"), Err(Error::Protocol(_))));
        let endless = vec![b'a'; 10_000];
        assert!(matches!(read(&endless), Err(Error::Protocol(_))));
        assert!(matches!(
            read(b"Content-Length: 2\r\n\r\n{"),
            Err(Error::Io(_))
        ));
        assert!(matches!(
            read(b"Content-Length: 2\r\n\r\n}{"),
            Err(Error::Protocol(_))
        ));
    }
}
