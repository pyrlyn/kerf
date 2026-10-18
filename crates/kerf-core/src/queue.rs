//! Bounded FIFO queue.
//!
//! Every producer-filled queue in Kerf has a hard cap: plugin and LSP output is
//! untrusted input, so a queue must answer `Full` rather than grow without
//! bound or block forever. Nothing is dropped silently — a rejected item is
//! handed back inside the error.

use std::collections::VecDeque;

use thiserror::Error;

/// A producer hit the queue's hard capacity.
///
/// The caller keeps the item (it is not stored and not dropped by the queue)
/// and is expected to retry after a drain, or to surface the back-pressure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("queue is full")]
pub struct QueueFull;

/// FIFO queue with a hard capacity.
///
/// `try_push` answers [`QueueFull`] once `capacity` items are stored; the
/// structure never grows past that count. Capacity `0` is legal and answers
/// `Full` on every push — useful as a permanently closed lane.
pub struct BoundedQueue<T> {
    items: VecDeque<T>,
    capacity: usize,
}

impl<T> BoundedQueue<T> {
    /// Creates a queue that accepts at most `capacity` items.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Pushes at the back, or answers [`QueueFull`] without storing `item`.
    pub fn try_push(&mut self, item: T) -> Result<(), QueueFull> {
        if self.is_full() {
            return Err(QueueFull);
        }
        self.items.push_back(item);
        Ok(())
    }

    /// Pops from the front (arrival order).
    pub fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    /// Number of items currently stored.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether the next push would answer [`QueueFull`].
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Hard cap this queue was created with.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundedQueue, QueueFull};

    #[test]
    fn try_push_answers_full_and_holds_capacity() {
        let mut q = BoundedQueue::new(3);
        for i in 0..3 {
            assert_eq!(q.try_push(i), Ok(()));
            assert_eq!(q.len(), i + 1);
            assert!(q.len() <= q.capacity());
        }
        assert!(q.is_full());
        assert_eq!(q.try_push(3), Err(QueueFull));
        assert_eq!(q.len(), 3);
    }

    #[test]
    fn full_queue_keeps_accepted_items() {
        let mut q = BoundedQueue::new(2);
        assert_eq!(q.try_push("a"), Ok(()));
        assert_eq!(q.try_push("b"), Ok(()));
        assert_eq!(q.try_push("c"), Err(QueueFull));
        assert_eq!(q.pop(), Some("a"));
        assert_eq!(q.pop(), Some("b"));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn pop_is_fifo() {
        let mut q = BoundedQueue::new(8);
        for i in 0..5 {
            assert_eq!(q.try_push(i), Ok(()));
        }
        for i in 0..5 {
            assert_eq!(q.pop(), Some(i));
        }
        assert!(q.is_empty());
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn capacity_one_edge() {
        let mut q = BoundedQueue::new(1);
        assert!(q.is_empty());
        assert!(!q.is_full());
        assert_eq!(q.try_push(10), Ok(()));
        assert!(q.is_full());
        assert_eq!(q.try_push(11), Err(QueueFull));
        assert_eq!(q.pop(), Some(10));
        assert_eq!(q.try_push(12), Ok(()));
        assert_eq!(q.pop(), Some(12));
    }

    #[test]
    fn burst_below_capacity_is_backpressured_not_dropped() {
        const CAP: usize = 4;
        const BURST: usize = 20;
        let mut q = BoundedQueue::new(CAP);
        let mut accepted = 0usize;
        let mut rejected = 0usize;
        for i in 0..BURST {
            match q.try_push(i) {
                Ok(()) => accepted += 1,
                Err(QueueFull) => rejected += 1,
            }
            assert!(q.len() <= CAP);
        }
        assert_eq!(accepted, CAP);
        assert_eq!(rejected, BURST - CAP);
        for i in 0..CAP {
            assert_eq!(q.pop(), Some(i));
        }
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn zero_capacity_always_full() {
        let mut q = BoundedQueue::<u8>::new(0);
        assert!(q.is_full());
        assert_eq!(q.try_push(1), Err(QueueFull));
        assert_eq!(q.pop(), None);
        assert_eq!(q.capacity(), 0);
    }
}
