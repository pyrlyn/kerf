// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's language-server client: JSON-RPC over the server's stdio, as
//! `docs/architecture.md` §5 lays out. The server is untrusted: message size,
//! queued messages and bytes, stderr and waiting time are all capped, and a
//! server that breaks the protocol is killed. Answers go to `kerf-semantic`,
//! which keeps tree-sitter as the base.

// Tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod framing;

use std::collections::VecDeque;
use std::io::{self, BufReader, BufWriter, Read};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

pub use framing::{read_message, write_message};
use serde_json::{Value, json};

/// Starts the server process. The client never calls `Command::spawn`
/// itself, so the Workspace Trust check and the sandbox wrap (T7,
/// `docs/architecture.md` §5.1) sit in one place: the implementation.
pub trait Spawn {
    /// Spawns `command`, whose stdio is already piped.
    ///
    /// # Errors
    ///
    /// Whatever refused the start, e.g. an untrusted folder.
    fn spawn(
        &mut self,
        command: &mut Command,
    ) -> Result<Child, Box<dyn std::error::Error + Send + Sync>>;
}

/// Caps for one server; the defaults are `docs/architecture.md` §5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest message body.
    pub max_message: usize,
    /// Messages waiting to be read.
    pub queued_messages: usize,
    /// Body bytes waiting to be read.
    pub queued_bytes: usize,
    /// Last stderr bytes kept for the log.
    pub stderr: usize,
    /// Wait for one answer.
    pub timeout: Duration,
    /// Wait for `shutdown` and exit before the process is killed.
    pub shutdown: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_message: 4 << 20,
            queued_messages: 256,
            queued_bytes: 4 << 20,
            stderr: 8 << 10,
            timeout: Duration::from_secs(10),
            shutdown: Duration::from_secs(2),
        }
    }
}

/// Why a server call failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// [`Spawn`] refused or failed.
    #[error("spawn: {0}")]
    Spawn(Box<dyn std::error::Error + Send + Sync>),
    /// Reading or writing the pipes failed.
    #[error("io: {0}")]
    Io(#[from] io::Error),
    /// A message over [`Limits::max_message`]; the server was killed.
    #[error("message of {len} bytes, the cap is {max}")]
    TooLarge {
        /// What the header claimed.
        len: usize,
        /// The cap.
        max: usize,
    },
    /// The server broke the base protocol; it was killed.
    #[error("protocol: {0}")]
    Protocol(&'static str),
    /// No answer within [`Limits::timeout`].
    #[error("{method} got no answer in {after:?}")]
    Timeout {
        /// The request.
        method: String,
        /// The wait.
        after: Duration,
    },
    /// The server answered with a JSON-RPC error.
    #[error("server error {code}: {message}")]
    Server {
        /// JSON-RPC error code.
        code: i64,
        /// Its message, cut to 1 KiB.
        message: String,
    },
    /// The server closed its output or exited.
    #[error("server exited")]
    Closed,
}

enum Inbound {
    Message(Value, usize),
    Closed(Error),
}

/// One running language server.
pub struct Client {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    inbound: Receiver<Inbound>,
    queued_bytes: Arc<AtomicUsize>,
    dropped: Arc<AtomicU64>,
    stderr: Arc<Mutex<VecDeque<u8>>>,
    next_id: i64,
    limits: Limits,
}

impl Client {
    /// Starts `command` through `spawner` and reads its output on two threads.
    ///
    /// # Errors
    ///
    /// [`Error::Spawn`], or [`Error::Io`] when the pipes are missing.
    pub fn start(
        spawner: &mut dyn Spawn,
        mut command: Command,
        limits: Limits,
    ) -> Result<Self, Error> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = spawner.spawn(&mut command).map_err(Error::Spawn)?;
        let (Some(stdin), Some(stdout), Some(err)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("server stdio is not piped").into());
        };
        let (tx, inbound) = mpsc::sync_channel(limits.queued_messages);
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let reader = Reader {
            tx,
            queued_bytes: queued_bytes.clone(),
            dropped: dropped.clone(),
            limits,
        };
        thread::spawn(move || reader.run(BufReader::new(stdout)));
        let stderr = Arc::new(Mutex::new(VecDeque::new()));
        let ring = stderr.clone();
        thread::spawn(move || keep_tail(err, &ring, limits.stderr));
        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            inbound,
            queued_bytes,
            dropped,
            stderr,
            next_id: 0,
            limits,
        })
    }

    /// Sends a request and waits for its answer. Server notifications that
    /// arrive meanwhile are skipped; server requests get "method not found".
    ///
    /// # Errors
    ///
    /// [`Error::Timeout`], [`Error::Server`], or what ended the server
    /// ([`Error::TooLarge`], [`Error::Protocol`], [`Error::Closed`]), after
    /// which the process is killed.
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, Error> {
        self.next_id += 1;
        let id = json!(self.next_id);
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        let deadline = Instant::now() + self.limits.timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let mut message = match self.inbound.recv_timeout(left) {
                Ok(Inbound::Message(m, len)) => {
                    self.queued_bytes.fetch_sub(len, Ordering::Relaxed);
                    m
                }
                Ok(Inbound::Closed(e)) => return Err(self.kill(e)),
                Err(RecvTimeoutError::Disconnected) => return Err(self.kill(Error::Closed)),
                Err(RecvTimeoutError::Timeout) => {
                    let after = self.limits.timeout;
                    return Err(Error::Timeout {
                        method: method.into(),
                        after,
                    });
                }
            };
            if message.get("method").is_some() {
                if let Some(theirs) = message.get("id") {
                    let error = json!({"code": -32601, "message": "not supported"});
                    self.send(&json!({"jsonrpc": "2.0", "id": theirs, "error": error}))?;
                }
                continue;
            }
            // Another id is a late answer to a request that timed out.
            if message.get("id") != Some(&id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                let code = error["code"].as_i64().unwrap_or(0);
                let text = error["message"].as_str().unwrap_or_default();
                let message = text.chars().take(1024).collect();
                return Err(Error::Server { code, message });
            }
            return Ok(message
                .get_mut("result")
                .map(Value::take)
                .unwrap_or_default());
        }
    }

    /// Sends a notification.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the server's input is closed.
    pub fn notify(&mut self, method: &str, params: Value) -> Result<(), Error> {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    /// `shutdown` and `exit`, then a kill once [`Limits::shutdown`] has passed.
    /// Returns whether the server exited on its own.
    pub fn shutdown(mut self) -> bool {
        self.limits.timeout = self.limits.shutdown;
        let deadline = Instant::now() + self.limits.shutdown;
        let _ = self.request("shutdown", Value::Null);
        let _ = self.notify("exit", Value::Null);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        self.kill(Error::Closed);
        false
    }

    /// Server notifications dropped because the queue was full.
    #[must_use]
    pub fn dropped_notifications(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// The last [`Limits::stderr`] bytes the server wrote to stderr.
    #[must_use]
    pub fn stderr_tail(&self) -> String {
        let ring = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&ring.iter().copied().collect::<Vec<_>>()).into_owned()
    }

    fn send(&mut self, message: &Value) -> Result<(), Error> {
        Ok(write_message(&mut self.stdin, message)?)
    }

    fn kill(&mut self, why: Error) -> Error {
        let _ = self.child.kill();
        let _ = self.child.wait();
        why
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // A client that goes away never leaves its server running.
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            self.kill(Error::Closed);
        }
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("pid", &self.child.id())
            .finish_non_exhaustive()
    }
}

/// The stdout thread: frames messages and queues them within the caps.
struct Reader {
    tx: SyncSender<Inbound>,
    queued_bytes: Arc<AtomicUsize>,
    dropped: Arc<AtomicU64>,
    limits: Limits,
}

impl Reader {
    fn run(self, mut out: BufReader<impl Read>) {
        loop {
            let message = match read_message(&mut out, self.limits.max_message) {
                Ok(Some(m)) => m,
                Ok(None) => return self.close(Error::Closed),
                Err(e) => return self.close(e),
            };
            let len = message.to_string().len();
            // Answers are never dropped: the client is waiting for one, and a
            // full queue only blocks this thread, which stops reading.
            if message.get("method").is_none() {
                self.queued_bytes.fetch_add(len, Ordering::Relaxed);
                if self.tx.send(Inbound::Message(message, len)).is_err() {
                    return;
                }
                continue;
            }
            if self.queued_bytes.load(Ordering::Relaxed) + len > self.limits.queued_bytes {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            self.queued_bytes.fetch_add(len, Ordering::Relaxed);
            match self.tx.try_send(Inbound::Message(message, len)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    self.queued_bytes.fetch_sub(len, Ordering::Relaxed);
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
    }

    fn close(self, why: Error) {
        let _ = self.tx.send(Inbound::Closed(why));
    }
}

/// The stderr thread: keeps the last `cap` bytes.
fn keep_tail(mut err: impl Read, ring: &Mutex<VecDeque<u8>>, cap: usize) {
    let mut chunk = [0; 1024];
    while let Ok(n @ 1..) = err.read(&mut chunk) {
        let mut ring = ring.lock().unwrap_or_else(PoisonError::into_inner);
        ring.extend(&chunk[..n]);
        let over = ring.len().saturating_sub(cap);
        ring.drain(..over);
    }
}
