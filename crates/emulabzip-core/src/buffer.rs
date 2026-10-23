//! A bounded buffer for streams that can outpace their reader (logs, samples).

use std::collections::VecDeque;

/// Keeps the newest `capacity` items and remembers how many were dropped, so the UI can say
/// "12,345 older lines were discarded" instead of silently losing data.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    items: VecDeque<T>,
    capacity: usize,
    dropped: u64,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "a ring buffer needs room for at least one item");
        RingBuffer { items: VecDeque::with_capacity(capacity.min(4096)), capacity, dropped: 0 }
    }

    pub fn push(&mut self, item: T) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
            self.dropped += 1;
        }
        self.items.push_back(item);
    }

    pub fn extend(&mut self, items: impl IntoIterator<Item = T>) {
        for item in items {
            self.push(item);
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// How many items were evicted since the buffer was created or cleared.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.items.iter()
    }

    pub fn last(&self) -> Option<&T> {
        self.items.back()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.dropped = 0;
    }

    /// Removes and returns everything, oldest first.
    pub fn drain(&mut self) -> Vec<T> {
        self.items.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_items() {
        let mut b = RingBuffer::new(3);
        b.extend(1..=5);
        assert_eq!(b.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
        assert_eq!(b.dropped(), 2);
        assert_eq!(b.last(), Some(&5));
    }

    #[test]
    fn clear_resets_the_drop_counter() {
        let mut b = RingBuffer::new(1);
        b.extend([1, 2]);
        b.clear();
        assert!(b.is_empty());
        assert_eq!(b.dropped(), 0);
    }

    #[test]
    fn drain_empties_the_buffer() {
        let mut b = RingBuffer::new(4);
        b.extend([1, 2]);
        assert_eq!(b.drain(), vec![1, 2]);
        assert!(b.is_empty());
    }
}
