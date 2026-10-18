// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The event loop's side of completion: hand a request to the worker thread,
//! poll for its answer, and drop it the moment the text changes.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use kerf_core::{EditEvent, TextBuffer};

use crate::client::Client;
use crate::context::Context;
use crate::error::Error;

/// One request, tagged with the generation it was made for.
struct Job {
    generation: u64,
    context: Context,
}

type Answer = (u64, Result<String, Error>);

/// Owns the worker thread and at most one live request.
///
/// Every [`request`](Self::request), [`cancel`](Self::cancel) and
/// [`on_edit`](Self::on_edit) starts a new generation. The worker skips a
/// queued job whose generation is no longer current, and [`poll`](Self::poll)
/// drops an answer whose generation is no longer current, so a suggestion for
/// text that has since changed never reaches the editor. A request already
/// on the wire cannot be aborted with a blocking client; it ends at the
/// latest at the timeout and its answer is discarded.
#[derive(Debug)]
pub struct Completer {
    jobs: Sender<Job>,
    answers: Receiver<Answer>,
    current: Arc<AtomicU64>,
    pending: bool,
    max_prefix_bytes: usize,
    max_suffix_bytes: usize,
}

impl Completer {
    /// Start the worker thread for `client`.
    pub fn new(client: Client) -> Result<Self, Error> {
        let (jobs, job_rx) = mpsc::channel();
        let (answer_tx, answers) = mpsc::channel();
        let current = Arc::new(AtomicU64::new(0));
        let max_prefix_bytes = client.config().max_prefix_bytes;
        let max_suffix_bytes = client.config().max_suffix_bytes;
        let worker_current = Arc::clone(&current);
        thread::Builder::new()
            .name("kerf-complete".into())
            .spawn(move || work(&client, &job_rx, &answer_tx, &worker_current))
            .map_err(|e| Error::Worker(e.to_string()))?;
        Ok(Self {
            jobs,
            answers,
            current,
            pending: false,
            max_prefix_bytes,
            max_suffix_bytes,
        })
    }

    /// Ask for a completion at byte offset `cursor`. Replaces any request
    /// still pending.
    pub fn request(&mut self, buffer: &TextBuffer, cursor: usize) -> Result<(), Error> {
        self.cancel();
        let context =
            Context::around(buffer, cursor, self.max_prefix_bytes, self.max_suffix_bytes)?;
        let job = Job {
            generation: self.current.load(Ordering::Acquire),
            context,
        };
        self.jobs
            .send(job)
            .map_err(|_| Error::Worker("the worker thread has exited".into()))?;
        self.pending = true;
        Ok(())
    }

    /// Forget the pending request, if any.
    pub fn cancel(&mut self) {
        self.current.fetch_add(1, Ordering::AcqRel);
        self.pending = false;
    }

    /// Any edit makes the context of a pending request stale, wherever it
    /// lands: a suggestion is only valid for the exact text it was made for.
    pub fn on_edit(&mut self, _edit: &EditEvent) {
        self.cancel();
    }

    /// `true` while a request is out and not yet answered or cancelled.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    /// The answer to the pending request, once it has arrived. Never blocks.
    pub fn poll(&mut self) -> Option<Result<String, Error>> {
        loop {
            match self.answers.try_recv() {
                Ok((generation, answer)) => {
                    if self.pending && generation == self.current.load(Ordering::Acquire) {
                        self.pending = false;
                        return Some(answer);
                    }
                }
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) if self.pending => {
                    self.pending = false;
                    return Some(Err(Error::Worker("the worker thread has exited".into())));
                }
                Err(TryRecvError::Disconnected) => return None,
            }
        }
    }
}

/// The worker loop. Ends when the [`Completer`] is dropped.
fn work(client: &Client, jobs: &Receiver<Job>, answers: &Sender<Answer>, current: &AtomicU64) {
    while let Ok(mut job) = jobs.recv() {
        // Typing queues a job per keystroke while one is on the wire; only
        // the newest is worth sending, so the server sees one at a time.
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        if job.generation != current.load(Ordering::Acquire) {
            continue;
        }
        let answer = client.complete(&job.context);
        if answers.send((job.generation, answer)).is_err() {
            return;
        }
    }
}
