//! The listings of the recipe A Lock Word, whose lock is the lock-word example's.

use std::thread;

use atomix::ordering::Relaxed;
use atomix::{AtomicU64, RangedU64};

use super::ROUNDS;
use super::lock_word::{LockState, LockWord, OwnerId};

#[test]
fn a_lock_keeps_its_holders_apart() {
    let (lock, count) = (LockWord::new(), AtomicU64::new(0));
    thread::scope(|scope| {
        for owner in 3..7 {
            let owner = OwnerId(RangedU64::new(owner).expect("from 3 up"));
            let (lock, count) = (&lock, &count);
            scope.spawn(move || {
                for _ in 0..ROUNDS {
                    let _guard = lock.lock(owner).expect("no holder panics");
                    count.store(count.load(Relaxed).wrapping_add(1), Relaxed);
                }
            });
        }
    });
    assert_eq!(count.load(Relaxed), 4 * ROUNDS, "no two holders overlapped");
}

#[test]
// ANCHOR: poison
fn a_holder_that_panics_poisons_the_lock() {
    let lock = LockWord::new();
    let (first, second) = (OwnerId(RangedU64::MIN), OwnerId(RangedU64::MIN.saturating_add(1)));
    let guard = lock.try_lock(first).expect("a new lock is free");
    assert_eq!(lock.try_lock(second).err(), Some(LockState::Held(first)), "the holder named");
    drop(guard);
    thread::scope(|scope| {
        let holder = scope.spawn(|| {
            let _guard = lock.lock(second).expect("the first holder freed it");
            panic!("the holder fails, its guard dropped by the unwind");
        });
        assert!(holder.join().is_err(), "the holder panicked");
    });
    assert_eq!(lock.lock(first).err(), Some(LockState::Poisoned), "and the lock is poisoned");
}
// ANCHOR_END: poison
