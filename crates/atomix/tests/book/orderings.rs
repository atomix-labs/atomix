//! The listings of the chapter Orderings.

#[test]
// ANCHOR: publish
fn a_release_store_publishes_what_came_before_it() {
    use std::thread;

    use atomix::hint::spin_loop;
    use atomix::ordering::{Acquire, Relaxed, Release};
    use atomix::{AtomicBool, AtomicU64};

    let (price, posted) = (AtomicU64::new(0), AtomicBool::new(false));
    thread::scope(|scope| {
        scope.spawn(|| {
            price.store(10_050, Relaxed);
            // ORDERING: Release, pairing with the reader's Acquire load, so it sees the price.
            posted.store(true, Release);
        });
        scope.spawn(|| {
            // ORDERING: Acquire, pairing with the writer's Release store.
            while !posted.load(Acquire) {
                spin_loop();
            }
            assert_eq!(price.load(Relaxed), 10_050, "the price written before the flag");
        });
    });
}
// ANCHOR_END: publish

#[test]
// ANCHOR: fences
fn fences_order_the_relaxed_accesses_around_them() {
    use std::thread;

    use atomix::hint::spin_loop;
    use atomix::ordering::{Acquire, Relaxed, Release};
    use atomix::{AtomicBool, AtomicU64, fence};

    let (price, posted) = (AtomicU64::new(0), AtomicBool::new(false));
    thread::scope(|scope| {
        scope.spawn(|| {
            price.store(10_050, Relaxed);
            // ORDERING: Release, pairing with the reader's Acquire fence across the flag, which
            // both threads access Relaxed.
            fence(Release);
            posted.store(true, Relaxed);
        });
        scope.spawn(|| {
            while !posted.load(Relaxed) {
                spin_loop();
            }
            // ORDERING: Acquire, pairing with the writer's Release fence.
            fence(Acquire);
            assert_eq!(price.load(Relaxed), 10_050, "the price written before the fence");
        });
    });
}
// ANCHOR_END: fences

#[test]
// ANCHOR: into_core
fn an_ordering_converts_into_the_core_ordering_of_its_name() {
    use core::sync::atomic::{AtomicU32, Ordering};

    use atomix::ordering::{Acquire, LoadOrdering};

    /// Reads a counter other code shares as core's atomic, in the ordering the caller names: the
    /// bound admits a load's orderings alone, so core's `load` cannot panic on the one it gets.
    fn read<O: LoadOrdering>(counter: &AtomicU32, order: O) -> u32 {
        counter.load(order.into())
    }

    let counter = AtomicU32::new(7);
    assert_eq!(read(&counter, Acquire), 7, "read with core's `Acquire`");
    assert_eq!(Ordering::from(Acquire), Ordering::Acquire, "which is what `Acquire` converts to");
}
// ANCHOR_END: into_core
