use b00t_c0re_gov::ring::HookRing;
use b00t_c0re_gov::types::{HookEvent, HookNotification};
use std::sync::Arc;
use std::thread;
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
fn test_full_ring() {
    let ring = HookRing::new();
    // Fill the ring to capacity
    for i in 0..1024 {
        assert!(ring.try_push(make_notif(i as u64)));
    }
    // Next push should fail
    assert!(!ring.try_push(make_notif(9999)));
}

#[test]
fn test_empty_ring() {
    let ring = HookRing::new();
    let items = ring.drain();
    assert!(items.is_empty());
    assert!(!ring.has_pending());
}

#[test]
fn test_wrapping_behavior() {
    let ring = HookRing::new();
    // Push and drain many times to exercise wrapping
    for cycle in 0..10 {
        for i in 0..100 {
            assert!(ring.try_push(make_notif((cycle * 100 + i) as u64)));
        }
        let items = ring.drain();
        assert_eq!(items.len(), 100);
    }
}

#[test]
fn test_fifo_order() {
    let ring = HookRing::new();
    for i in 0..100 {
        ring.try_push(make_notif(i));
    }
    let items = ring.drain();
    for (idx, item) in items.iter().enumerate() {
        let expected = Uuid::from_u64_pair(idx as u64, 0);
        assert_eq!(item.hook_id, expected, "FIFO order violated at index {}", idx);
    }
}

#[test]
fn test_partial_drain() {
    let ring = HookRing::new();
    for i in 0..10 {
        ring.try_push(make_notif(i));
    }

    let first = ring.drain();
    assert_eq!(first.len(), 10);

    for i in 10..20 {
        ring.try_push(make_notif(i));
    }

    let second = ring.drain();
    assert_eq!(second.len(), 10);
}

#[test]
fn test_concurrent_producer_consumer() {
    let ring = Arc::new(HookRing::new());
    let ring_producer = ring.clone();
    let ring_consumer = ring.clone();

    let producer = thread::spawn(move || {
        for i in 0..500 {
            let mut pushed = false;
            while !pushed {
                pushed = ring_producer.try_push(make_notif(i));
                if !pushed {
                    // Ring full, yield briefly
                    thread::yield_now();
                }
            }
        }
    });

    let consumer = thread::spawn(move || {
        let mut received = 0u64;
        let mut last_id: Option<u64> = None;
        while received < 500 {
            let items = ring_consumer.drain();
            for item in items {
                // Check monotonic order
                if let Some(last) = last_id {
                    // Extract the u64 from the Uuid
                    let (high, _low) = item.hook_id.as_u64_pair();
                    let cur = high; // We used from_u64_pair(id, 0)
                    assert!(cur > last, "Non-monotonic: {} <= {}", cur, last);
                }
                let (high, _low) = item.hook_id.as_u64_pair();
                last_id = Some(high);
                received += 1;
            }
            if received < 500 {
                thread::yield_now();
            }
        }
        assert_eq!(received, 500);
    });

    producer.join().expect("producer panicked");
    consumer.join().expect("consumer panicked");
}
