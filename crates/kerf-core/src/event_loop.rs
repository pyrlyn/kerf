//! Single-threaded event loop over bounded queues.
//!
//! The loop owns its inbox and outbox and drains them one event at a time, in
//! arrival order. Producers get back-pressure ([`QueueFull`]) instead of an
//! unbounded buffer — the same rule as every other Kerf queue, because events
//! can come from untrusted plugin and LSP output.

use crate::queue::{BoundedQueue, QueueFull};

/// Default inbox/outbox capacity when the caller does not pick one.
///
/// Matches wasm-plugin-host's event lane (`research.md`): deep enough for a
/// burst of key events, shallow enough that a stalled consumer shows up as
/// `Full` rather than as unbounded growth.
pub const DEFAULT_CAPACITY: usize = 256;

/// Single-threaded event loop with a bounded inbox and a bounded outbox.
///
/// `EventLoop<E>` is generic over the event type so the core can carry
/// [`EditEvent`](crate::event::EditEvent)s later without the loop knowing the
/// buffer. Processing is strictly one event at a time: `poll` never re-enters
/// a handler, and FIFO order is preserved on both queues.
pub struct EventLoop<E> {
    inbox: BoundedQueue<E>,
    outbox: BoundedQueue<E>,
}

impl<E> EventLoop<E> {
    /// Creates a loop whose inbox and outbox each hold `capacity` events.
    ///
    /// Capacity `0` is legal and yields a permanently closed inbox.
    pub fn new(capacity: usize) -> Self {
        Self {
            inbox: BoundedQueue::new(capacity),
            outbox: BoundedQueue::new(capacity),
        }
    }

    /// Creates a loop with [`DEFAULT_CAPACITY`] on both queues.
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Offers one event to the inbox. Answers [`QueueFull`] when full.
    pub fn try_send(&mut self, ev: E) -> Result<(), QueueFull> {
        self.inbox.try_push(ev)
    }

    /// Takes the oldest inbox event (drain one, in arrival order).
    ///
    /// Named `next_event` rather than `next` so it is not confused with
    /// `Iterator::next`.
    pub fn next_event(&mut self) -> Option<E> {
        self.inbox.pop()
    }

    /// Offers one event to the outbox (handler output). Answers [`QueueFull`].
    pub fn try_emit(&mut self, ev: E) -> Result<(), QueueFull> {
        self.outbox.try_push(ev)
    }

    /// Takes the oldest outbox event (drain one, in arrival order).
    pub fn drain_output(&mut self) -> Option<E> {
        self.outbox.pop()
    }

    /// Runs `handler` on at most one inbox event.
    ///
    /// The handler owns the event and may `try_emit` into the outbox or
    /// `try_send` further work. Only one handler runs at a time — the loop
    /// never interleaves handlers. Returns `false` when the inbox was empty.
    pub fn poll<F>(&mut self, handler: F) -> bool
    where
        F: FnOnce(E, &mut Self),
    {
        match self.inbox.pop() {
            Some(ev) => {
                handler(ev, self);
                true
            }
            None => false,
        }
    }

    /// Number of events waiting in the inbox.
    pub fn inbox_len(&self) -> usize {
        self.inbox.len()
    }

    /// Number of events waiting in the outbox.
    pub fn outbox_len(&self) -> usize {
        self.outbox.len()
    }

    /// Hard capacity of the inbox (and of the outbox — they share it).
    pub fn capacity(&self) -> usize {
        self.inbox.capacity()
    }

    /// Whether the inbox would answer [`QueueFull`] on the next send.
    pub fn is_inbox_full(&self) -> bool {
        self.inbox.is_full()
    }

    /// Whether the outbox would answer [`QueueFull`] on the next emit.
    pub fn is_outbox_full(&self) -> bool {
        self.outbox.is_full()
    }
}

impl<E> Default for EventLoop<E> {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_CAPACITY, EventLoop};
    use crate::queue::QueueFull;

    #[test]
    fn default_capacity_matches_named_constant() {
        let mut ev_loop = EventLoop::<u8>::default();
        assert_eq!(ev_loop.capacity(), DEFAULT_CAPACITY);
        assert_eq!(ev_loop.try_send(1), Ok(()));
    }

    #[test]
    fn try_send_backpressures_at_capacity() {
        let mut ev_loop = EventLoop::new(3);
        for i in 0..3 {
            assert_eq!(ev_loop.try_send(i), Ok(()));
        }
        assert!(ev_loop.is_inbox_full());
        assert_eq!(ev_loop.try_send(3), Err(QueueFull));
        assert_eq!(ev_loop.inbox_len(), 3);
    }

    #[test]
    fn next_event_is_fifo() {
        let mut ev_loop = EventLoop::new(8);
        for i in 0..5 {
            assert_eq!(ev_loop.try_send(i), Ok(()));
        }
        for i in 0..5 {
            assert_eq!(ev_loop.next_event(), Some(i));
        }
        assert_eq!(ev_loop.next_event(), None);
    }

    #[test]
    fn capacity_one_edge() {
        let mut ev_loop = EventLoop::new(1);
        assert_eq!(ev_loop.try_send("a"), Ok(()));
        assert_eq!(ev_loop.try_send("b"), Err(QueueFull));
        assert_eq!(ev_loop.next_event(), Some("a"));
        assert_eq!(ev_loop.try_send("c"), Ok(()));
        assert_eq!(ev_loop.next_event(), Some("c"));
    }

    #[test]
    fn burst_of_n_with_cap_below_n() {
        const CAP: usize = 5;
        const N: usize = 17;
        let mut ev_loop = EventLoop::new(CAP);
        let mut accepted = 0usize;
        for i in 0..N {
            if let Ok(()) = ev_loop.try_send(i) {
                accepted += 1;
            }
            assert!(ev_loop.inbox_len() <= CAP);
        }
        assert_eq!(accepted, CAP);
        for i in 0..CAP {
            assert_eq!(ev_loop.next_event(), Some(i));
        }
        assert_eq!(ev_loop.next_event(), None);
    }

    #[test]
    fn full_inbox_keeps_accepted_events() {
        let mut ev_loop = EventLoop::new(2);
        assert_eq!(ev_loop.try_send("kept-1"), Ok(()));
        assert_eq!(ev_loop.try_send("kept-2"), Ok(()));
        assert_eq!(ev_loop.try_send("rejected"), Err(QueueFull));
        assert_eq!(ev_loop.next_event(), Some("kept-1"));
        assert_eq!(ev_loop.next_event(), Some("kept-2"));
        assert_eq!(ev_loop.next_event(), None);
    }

    #[test]
    fn poll_runs_one_handler_at_a_time() {
        let mut ev_loop = EventLoop::new(4);
        for i in 0..3 {
            assert_eq!(ev_loop.try_send(i), Ok(()));
        }
        let mut seen = Vec::new();
        while ev_loop.poll(|ev, _loop| {
            seen.push(ev);
        }) {
            // one event per poll; the loop never interleaves handlers
        }
        assert_eq!(seen, vec![0, 1, 2]);
    }

    #[test]
    fn poll_handler_can_emit_into_outbox() {
        let mut ev_loop = EventLoop::new(4);
        assert_eq!(ev_loop.try_send(10), Ok(()));
        assert!(ev_loop.poll(|ev, lp| {
            assert_eq!(lp.try_emit(ev + 1), Ok(()));
        }));
        assert_eq!(ev_loop.drain_output(), Some(11));
        assert_eq!(ev_loop.drain_output(), None);
    }

    #[test]
    fn outbox_is_bounded_too() {
        let mut ev_loop = EventLoop::new(1);
        assert_eq!(ev_loop.try_emit("a"), Ok(()));
        assert!(ev_loop.is_outbox_full());
        assert_eq!(ev_loop.try_emit("b"), Err(QueueFull));
        assert_eq!(ev_loop.drain_output(), Some("a"));
    }
}
