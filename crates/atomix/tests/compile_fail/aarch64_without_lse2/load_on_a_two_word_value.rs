//! Without LSE2, a 16-byte load is a compare-exchange, which writes the cache line, so two words
//! have no `load`, and the refusal names `load_rmw`.

use core::ptr::NonNull;

use atomix::ordering::Acquire;
use atomix::{Atom, Atomic};

/// A stack's head: the top value, and a version in a word of its own.
#[derive(Clone, Copy, Atom)]
struct Head {
    top: Option<NonNull<u64>>,
    version: u64,
}

fn main() {
    let head = Atomic::new(Head { top: None, version: 0 });
    let _ = head.load(Acquire);
}
