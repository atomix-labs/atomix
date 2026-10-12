//! A bitmap allocator: each slot of an arena is free or taken by one bit, and a thread takes the
//! first free one it finds with one `lock bts`, or `ldseta`, whose result says whether it was free.

use core::array;

use atomix::AtomicU64;
use atomix::ordering::{Acquire, Relaxed, Release};

/// A slot: its word of the bitmap, and its bit in the word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Slot {
    /// The word.
    word: usize,
    /// The bit, from 0 to 63.
    bit: u32,
}

/// An allocator of 64 slots a word, in `WORDS` words.
#[derive(Debug)]
pub(crate) struct Bitmap<const WORDS: usize> {
    /// Each word's slots, a bit set where the slot is taken.
    words: [AtomicU64; WORDS],
}

impl<const WORDS: usize> Bitmap<WORDS> {
    /// A bitmap whose every slot is free.
    pub(crate) fn new() -> Self {
        Self { words: array::from_fn(|_| AtomicU64::new(0)) }
    }

    // ANCHOR: allocate
    /// Takes a free slot, or `None` where every slot is taken.
    pub(crate) fn allocate(&self) -> Option<Slot> {
        for (word, bits) in self.words.iter().enumerate() {
            // ORDERING: Relaxed: the search reads only which bit to try.
            let mut taken = bits.load(Relaxed);
            while taken != u64::MAX {
                let bit = taken.trailing_ones();
                // ORDERING: Acquire, so the slot's last holder's writes, which its free published,
                // are seen.
                if !bits.bit_set(bit, Acquire) {
                    return Some(Slot { word, bit });
                }
                taken = bits.load(Relaxed);
            }
        }
        None
    }

    /// Frees `slot`, which `allocate` took.
    pub(crate) fn free(&self, slot: Slot) {
        // ORDERING: Release, pairing with the next claim's Acquire: the holder's writes reach the
        // slot's next holder. `and`, as the bit before is known: `lock and`, or `ldclrl`.
        self.words[slot.word].and(!(1 << slot.bit), Release);
    }
    // ANCHOR_END: allocate
}
