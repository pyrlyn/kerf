//! Kerf's core: bounded queues, the single-threaded event loop and the
//! edit-event shape the buffer and the semantic layer share.
//!
//! No UI here. The text buffer (T2.1) and the semantic layer (T3) sit beside
//! this crate; this half of the core only guarantees that events never grow a
//! queue past its hard cap and never interleave handlers.
//!
//! - [`queue`] — [`BoundedQueue`](queue::BoundedQueue) and back-pressure.
//! - [`event`] — [`EditEvent`](event::EditEvent), pre-edit byte coordinates.
//! - [`event_loop`] — [`EventLoop`](event_loop::EventLoop): inbox, outbox,
//!   one event at a time.

pub mod event;
pub mod event_loop;
pub mod queue;

pub use event::EditEvent;
pub use event_loop::{DEFAULT_CAPACITY, EventLoop};
pub use queue::{BoundedQueue, QueueFull};
