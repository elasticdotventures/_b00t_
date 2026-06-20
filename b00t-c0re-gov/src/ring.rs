use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::types::HookNotification;

const RING_CAPACITY: usize = 1024;

pub struct HookRing {
    ring: Box<[UnsafeCell<MaybeUninit<HookNotification>>; RING_CAPACITY]>,
    head: AtomicU64, // Producer writes here
    tail: AtomicU64, // Consumer reads from here
}

impl HookRing {
    pub fn new() -> Self {
        let ring = Box::new(unsafe {
            // SAFETY: MaybeUninit is zero-cost, we initialize on push
            std::mem::zeroed()
        });
        HookRing {
            ring,
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
        }
    }

    /// Producer: push a notification. Returns false if ring is full.
    pub fn try_push(&self, notification: HookNotification) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let len = head.wrapping_sub(tail);

        if len >= RING_CAPACITY as u64 {
            return false; // Ring is full
        }

        let idx = (head as usize) & (RING_CAPACITY - 1);
        // SAFETY: We own the slot (between head and tail), no concurrent writer
        unsafe {
            let slot = &mut *self.ring[idx].get();
            slot.write(notification);
        }

        self.head.store(head.wrapping_add(1), Ordering::Release);
        true
    }

    /// Consumer: pop all available notifications. Non-blocking.
    pub fn drain(&self) -> Vec<HookNotification> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        let available = head.wrapping_sub(tail);

        if available == 0 {
            return Vec::new();
        }

        let count = available.min(RING_CAPACITY as u64) as usize;
        let mut result = Vec::with_capacity(count);

        for i in 0..count {
            let idx = ((tail as usize) + i) & (RING_CAPACITY - 1);
            // SAFETY: We own the slot for reading (between tail and head), no concurrent reader
            unsafe {
                let slot = &*self.ring[idx].get();
                result.push((*slot.as_ptr()).clone());
            }
        }

        self.tail
            .store(tail.wrapping_add(count as u64), Ordering::Release);
        result
    }

    /// Consumer: peek if notifications are available.
    pub fn has_pending(&self) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        head.wrapping_sub(tail) > 0
    }
}

unsafe impl Send for HookRing {}
unsafe impl Sync for HookRing {}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::types::{HookEvent, HookNotification};
    use uuid::Uuid;

    fn make_notif(id: u64) -> HookNotification {
        HookNotification {
            hook_id: Uuid::from_u64_pair(id, 0),
            event: HookEvent::Fired,
        }
    }

    #[test]
    fn test_push_and_drain() {
        let ring = HookRing::new();
        assert!(ring.try_push(make_notif(1)));
        assert!(ring.try_push(make_notif(2)));
        assert!(ring.try_push(make_notif(3)));

        let items = ring.drain();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_empty_drain() {
        let ring = HookRing::new();
        let items = ring.drain();
        assert!(items.is_empty());
    }

    #[test]
    fn test_has_pending() {
        let ring = HookRing::new();
        assert!(!ring.has_pending());
        ring.try_push(make_notif(99));
        assert!(ring.has_pending());
        ring.drain();
        assert!(!ring.has_pending());
    }

    #[test]
    fn test_full_ring() {
        let ring = HookRing::new();
        // Fill the ring to capacity
        for i in 0..RING_CAPACITY {
            assert!(ring.try_push(make_notif(i as u64)));
        }
        // Next push should fail
        assert!(!ring.try_push(make_notif(9999)));
    }

    #[test]
    fn test_wrapping_behavior() {
        let ring = HookRing::new();
        // Simulate wrapping: push and drain many times
        for cycle in 0..3 {
            for i in 0..100 {
                assert!(ring.try_push(make_notif((cycle * 100 + i) as u64)));
            }
            let items = ring.drain();
            assert_eq!(items.len(), 100);
        }
    }

    #[test]
    fn test_partial_drain() {
        let ring = HookRing::new();
        // Push some items, drain some, push more
        for i in 0..10 {
            ring.try_push(make_notif(i));
        }

        let first_batch = ring.drain();
        assert_eq!(first_batch.len(), 10);

        for i in 10..20 {
            ring.try_push(make_notif(i));
        }

        let second_batch = ring.drain();
        assert_eq!(second_batch.len(), 10);
    }

    #[test]
    fn test_fifo_order() {
        let ring = HookRing::new();
        for i in 0..50 {
            ring.try_push(make_notif(i));
        }
        let items = ring.drain();
        for (idx, item) in items.iter().enumerate() {
            let expected = Uuid::from_u64_pair(idx as u64, 0);
            assert_eq!(
                item.hook_id, expected,
                "FIFO order violated at index {}",
                idx
            );
        }
    }
}
