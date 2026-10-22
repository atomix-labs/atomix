//! A sequence lock over atomics: a writer changes several words, and readers read them whole
//! without writing anything, retrying where a write overlapped the read.
//!
//! Each word is an atomic, so a read that overlaps a write reads each word whole, old or new,
//! never a torn one or undefined behaviour; the sequence, odd while a write is under way, tells the
//! reader to retry.

use atomix::hint::spin_loop;
use atomix::ordering::{Acquire, Relaxed, Release, StoreStore};
use atomix::{AtomicU64, fence};

/// Two words a writer changes together.
#[derive(Debug)]
pub(crate) struct SequenceLock {
    /// How many times a write began or ended: odd while one is under way.
    sequence: AtomicU64,
    /// The words.
    words: [AtomicU64; 2],
}

impl SequenceLock {
    /// A lock holding `words`.
    pub(crate) fn new(words: [u64; 2]) -> Self {
        Self { sequence: AtomicU64::new(0), words: words.map(AtomicU64::new) }
    }

    // ANCHOR: write
    /// Writes `words`, waiting first for any write under way.
    pub(crate) fn write(&self, words: [u64; 2]) {
        let begin = |sequence: u64| sequence.is_multiple_of(2).then(|| sequence.wrapping_add(1));
        // ORDERING: Acquire on the exchange that makes the sequence odd, so this write follows the
        // last one's; Relaxed on the reads before it, which only wait.
        let sequence = loop {
            match self.sequence.try_update(Acquire, Relaxed, begin) {
                Ok(even) => break even,
                Err(_odd) => spin_loop(),
            }
        };
        // ORDERING: StoreStore, pairing with the Acquire fence in `read`: the odd sequence before
        // any word, so a reader that reads a word this write stores then reads the sequence moved
        // on. On `aarch64` it is `dmb ishst`, which keeps the order in the hardware alone; on
        // `x86_64` and under loom, a Release fence, no instruction on `x86_64`.
        fence(StoreStore);
        for (word, value) in self.words.iter().zip(words) {
            word.store(value, Relaxed);
        }
        // ORDERING: Release, publishing the words to the reader that reads the sequence even again.
        self.sequence.store(sequence.wrapping_add(2), Release);
    }
    // ANCHOR_END: write

    // ANCHOR: read
    /// Reads the words a write left, retrying while a write is under way.
    pub(crate) fn read(&self) -> [u64; 2] {
        loop {
            // ORDERING: Acquire, so the words the write that left this sequence stored are read.
            let before = self.sequence.load(Acquire);
            // ORDERING: Relaxed: each word is read whole, and the fence below orders the reads.
            let words = self.words.each_ref().map(|word| word.load(Relaxed));
            // ORDERING: Acquire, pairing with the write's StoreStore fence: a word a later write
            // stored makes the load below read that write's odd sequence, or a later one.
            fence(Acquire);
            // ORDERING: Relaxed: the fence above orders it after the words.
            if before.is_multiple_of(2) && self.sequence.load(Relaxed) == before {
                return words;
            }
            spin_loop();
        }
    }
    // ANCHOR_END: read

    /// `read`, but with its first load `Relaxed`, so the words may be older than the sequence:
    /// the lock's model must report a torn read.
    #[cfg(loom)]
    pub(crate) fn read_with_a_relaxed_first_load(&self) -> [u64; 2] {
        loop {
            // ORDERING: Relaxed where `read` has Acquire, the bug the model must find: it pairs
            // with no write's Release.
            let before = self.sequence.load(Relaxed);
            // ORDERING: as in `read`.
            let words = self.words.each_ref().map(|word| word.load(Relaxed));
            fence(Acquire);
            if before.is_multiple_of(2) && self.sequence.load(Relaxed) == before {
                return words;
            }
            spin_loop();
        }
    }

    /// `read`, but without its fence, so a word a later write stored need not move the sequence
    /// the second load reads: the lock's model must report a torn read.
    #[cfg(loom)]
    pub(crate) fn read_without_its_fence(&self) -> [u64; 2] {
        loop {
            // ORDERING: as in `read`, with no fence after the words, the bug the model must find.
            let before = self.sequence.load(Acquire);
            let words = self.words.each_ref().map(|word| word.load(Relaxed));
            if before.is_multiple_of(2) && self.sequence.load(Relaxed) == before {
                return words;
            }
            spin_loop();
        }
    }
}
