// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! One UI connection: bytes in, lines decoded under a cap, requests queued
//! under a cap, answers out. No I/O here, so tests and any transport drive it
//! the same way; [`Connection::serve`] is the blocking loop over a byte stream.

use std::io::{self, Read, Write};

use jsonrpsee_types::{Id, InvalidRequest, Request, Response, ResponsePayload};
use kerf_core::BoundedQueue;
use serde_json::Value;
use serde_json::value::RawValue;
use tokio_util::bytes::BytesMut;
use tokio_util::codec::{Decoder, LinesCodec, LinesCodecError};

use crate::error::RemoteError;
use crate::host::{self, Host};
use crate::protocol::{InitializeParams, InitializeResult, PROTOCOL_MAJOR, Version, method};

/// Bytes asked of the stream per read.
const READ_CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    AwaitingInitialize,
    Ready,
    Closed,
}

/// A connection between one UI and a [`Host`].
pub struct Connection {
    host: Host,
    codec: LinesCodec,
    inbox: BytesMut,
    queue: BoundedQueue<String>,
    out: Vec<u8>,
    state: State,
}

impl Connection {
    /// A fresh connection waiting for `initialize`.
    #[must_use]
    pub fn new(host: Host) -> Self {
        let limits = host.limits();
        Self {
            host,
            codec: LinesCodec::new_with_max_length(limits.max_message_bytes),
            inbox: BytesMut::new(),
            queue: BoundedQueue::new(limits.max_queued),
            out: Vec::new(),
            state: State::AwaitingInitialize,
        }
    }

    /// Takes bytes from the stream and queues every complete line. A line
    /// over the cap is refused and skipped; a request past the queue cap is
    /// answered busy, which bounds how many requests a UI may pipeline.
    pub fn receive(&mut self, bytes: &[u8]) {
        self.inbox.extend_from_slice(bytes);
        loop {
            let line = self.codec.decode(&mut self.inbox);
            if !self.take(line) {
                break;
            }
        }
    }

    /// The stream ended: a last line without a newline still counts.
    pub fn finish(&mut self) {
        let line = self.codec.decode_eof(&mut self.inbox);
        self.take(line);
    }

    /// Answers the oldest queued request. `false` when nothing was queued or
    /// the connection is closed.
    pub fn step(&mut self) -> bool {
        if self.state == State::Closed {
            return false;
        }
        let Some(line) = self.queue.pop() else {
            return false;
        };
        self.handle(&line);
        true
    }

    /// Bytes to write to the stream, one JSON object per line.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out)
    }

    /// `true` after a refused handshake: the host answers nothing more.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == State::Closed
    }

    /// Serves `input` until it ends or the handshake is refused, writing each
    /// answer as soon as it is ready. `input` and `output` are any byte
    /// stream: the host's stdio under SSH, a socket, or memory in tests.
    ///
    /// # Errors
    ///
    /// Reading or writing the stream failed.
    pub fn serve(&mut self, mut input: impl Read, mut output: impl Write) -> io::Result<()> {
        let mut chunk = vec![0; READ_CHUNK];
        loop {
            let n = match input.read(&mut chunk) {
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match chunk.get(..n) {
                Some(bytes) if n > 0 => self.receive(bytes),
                _ => self.finish(),
            }
            loop {
                output.write_all(&self.take_output())?;
                if !self.step() {
                    break;
                }
            }
            output.flush()?;
            if n == 0 || self.is_closed() {
                return Ok(());
            }
        }
    }

    /// Queues one decoded line or answers why it cannot be. `false` when the
    /// codec needs more bytes.
    fn take(&mut self, line: Result<Option<String>, LinesCodecError>) -> bool {
        match line {
            Ok(Some(line)) => {
                if self.queue.is_full() {
                    let id = request_id(&line);
                    let cap = self.queue.capacity();
                    self.answer(id, Err(RemoteError::Busy { cap }));
                } else {
                    let _ = self.queue.try_push(line);
                }
                true
            }
            Ok(None) => false,
            Err(LinesCodecError::MaxLineLengthExceeded) => {
                let cap = self.host.limits().max_message_bytes;
                self.answer(Id::Null, Err(RemoteError::MessageTooLarge { cap }));
                true
            }
            Err(LinesCodecError::Io(e)) => {
                self.answer(Id::Null, Err(RemoteError::Parse(e.to_string())));
                true
            }
        }
    }

    fn handle(&mut self, line: &str) {
        let request = match serde_json::from_str::<Request<'_>>(line) {
            Ok(request) => request,
            Err(e) => {
                match serde_json::from_str::<Value>(line) {
                    Err(_) => self.answer(Id::Null, Err(RemoteError::Parse(e.to_string()))),
                    // A notification expects no answer, and the host takes none yet.
                    Ok(v) if v.get("method").is_some() && v.get("id").is_none() => {}
                    Ok(_) => self.answer(request_id(line), Err(RemoteError::InvalidRequest)),
                }
                return;
            }
        };
        let params = request.params.as_deref();
        let result = match (self.state, request.method_name()) {
            (State::AwaitingInitialize, method::INITIALIZE) => self.initialize(params),
            (State::AwaitingInitialize, _) => Err(RemoteError::NotInitialized),
            (_, method::INITIALIZE) => Err(RemoteError::AlreadyInitialized),
            (_, name) => self.host.call(name, params),
        };
        if matches!(result, Err(RemoteError::UnsupportedVersion { .. })) {
            self.state = State::Closed;
        }
        self.answer(request.id, result);
    }

    fn initialize(&mut self, params: Option<&RawValue>) -> Result<Value, RemoteError> {
        let p: InitializeParams = host::parse(params)?;
        if p.protocol.major != PROTOCOL_MAJOR {
            return Err(RemoteError::UnsupportedVersion { got: p.protocol });
        }
        self.state = State::Ready;
        host::reply(InitializeResult {
            protocol: Version::CURRENT,
        })
    }

    /// Writes one answer; one that would pass the message cap is replaced by
    /// [`RemoteError::ReplyTooLarge`], so the UI never has to buffer more than
    /// the cap either.
    fn answer(&mut self, id: Id<'_>, result: Result<Value, RemoteError>) {
        let cap = self.host.limits().max_message_bytes;
        let mut line = encode(&id, result);
        if line.len() > cap {
            line = encode(&id, Err(RemoteError::ReplyTooLarge { cap }));
        }
        self.out.extend_from_slice(&line);
        self.out.push(b'\n');
    }
}

fn encode(id: &Id<'_>, result: Result<Value, RemoteError>) -> Vec<u8> {
    let payload = match result {
        Ok(value) => ResponsePayload::success(value),
        Err(e) => ResponsePayload::error(e.to_object()),
    };
    // Serializing a `Value` or an error object cannot fail: every map key is
    // a string.
    serde_json::to_vec(&Response::new(payload, id.clone())).unwrap_or_default()
}

/// The id of a line that will not be handled, so its answer still matches a
/// request on the UI side; `null` when there is none to find.
fn request_id(line: &str) -> Id<'static> {
    serde_json::from_str::<InvalidRequest<'_>>(line).map_or(Id::Null, |r| r.id.into_owned())
}
