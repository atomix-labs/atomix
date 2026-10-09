//! A mask could clear a `NonNull`'s every address bit, to null: only a raw pointer, which may be
//! null, takes a `usize` mask; a `NonNull` keeps its tags in a derived pointer word.

use core::ptr::NonNull;

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn main() {
    let next = Atomic::new(NonNull::<u64>::dangling());
    next.and(!0b1_usize, Relaxed);
}
