//! Code generic over an atomic's value calls `and` without an `AtomBitwise` bound: it is refused
//! with `AtomBitwise`'s message, though its operand is a `usize` mask, which a raw pointer takes.

use atomix_core::ordering::Relaxed;
use atomix_core::{Atom, Atomic, AtomicU64};

fn clear_lowest_bit<T: Atom>(word: &Atomic<T>) {
    word.and(!1_usize, Relaxed);
}

fn main() {
    clear_lowest_bit(&AtomicU64::new(0b11));
}
