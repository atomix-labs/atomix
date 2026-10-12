//! A lock word: whether a lock is free, poisoned, or held, and by whom, in the one `u64` an owner's
//! id takes anyway.
//!
//! The lock's states take the integers below the first id, so taking the lock is one
//! compare-exchange, and the word names its holder.

use std::thread;

use atomix::ordering::{Acquire, Relaxed, Release};
use atomix::{Atom, Atomic, RangedU64, hint};

// ANCHOR: word
/// An owner's id, from 3, which leaves 0 to 2 for the lock's other states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
pub(crate) struct OwnerId(pub(crate) RangedU64<3>);

/// A lock's state: its unit variants take integers below the ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
pub(crate) enum LockState {
    /// No one holds it.
    Free,
    /// A holder panicked, and left what the lock guards half written.
    Poisoned,
    /// This owner holds it.
    Held(OwnerId),
}
// ANCHOR_END: word

/// A lock, as one word.
#[derive(Debug)]
pub(crate) struct LockWord {
    /// Whether the lock is free, poisoned, or held, and by whom.
    state: Atomic<LockState>,
}

/// A hold on a [`LockWord`], which frees it when dropped, or poisons it when a panic drops it.
#[derive(Debug)]
#[must_use = "the lock is freed as the guard drops"]
pub(crate) struct Guard<'a> {
    /// The lock held.
    lock: &'a LockWord,
}

impl LockWord {
    /// A free lock.
    #[cfg_attr(
        not(any(loom, test)),
        expect(
            clippy::missing_const_for_fn,
            reason = "loom's `Atomic::new`, which a model builds it with, is not `const`"
        )
    )]
    pub(crate) fn new() -> Self {
        Self { state: Atomic::new(LockState::Free) }
    }

    // ANCHOR: try_lock
    /// Takes the lock for `owner` where it is free, or returns the state that kept it.
    pub(crate) fn try_lock(&self, owner: OwnerId) -> Result<Guard<'_>, LockState> {
        // ORDERING: Acquire, so the last holder's writes, which its guard's Release published, are
        // seen; Relaxed where the lock is not free, as nothing it guards is read.
        self.state.compare_exchange(LockState::Free, LockState::Held(owner), Acquire, Relaxed)?;
        Ok(Guard { lock: self })
    }
    // ANCHOR_END: try_lock

    // ANCHOR: lock
    /// Takes the lock for `owner`, spinning while another holds it, or returns `Poisoned` where a
    /// holder panicked.
    pub(crate) fn lock(&self, owner: OwnerId) -> Result<Guard<'_>, LockState> {
        loop {
            match self.try_lock(owner) {
                Ok(guard) => return Ok(guard),
                Err(LockState::Poisoned) => return Err(LockState::Poisoned),
                // A strong exchange finds `Free` only where it takes the lock, so this waits on a
                // holder.
                Err(LockState::Free | LockState::Held(_)) => {
                    // ORDERING: Relaxed: the wait reads only whether to try again, which the
                    // exchange then does with Acquire.
                    while matches!(self.state.load(Relaxed), LockState::Held(_)) {
                        hint::spin_loop();
                    }
                },
            }
        }
    }
    // ANCHOR_END: lock
}

// ANCHOR: unlock
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let state = if thread::panicking() { LockState::Poisoned } else { LockState::Free };
        // ORDERING: Release, publishing the holder's writes to the next to take the lock.
        self.lock.state.store(state, Release);
    }
}
// ANCHOR_END: unlock
