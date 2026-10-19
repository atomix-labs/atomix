//! An array has no add: a carry out of one element would land in the next, so `fetch_add` needs
//! `AtomAdd`, which an array lacks. Its last element adds alone, through `last()`, where the
//! slots fill the repr.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn main() {
    let counts = Atomic::new([1_u16, 2]);
    let _ = counts.last().fetch_add(1, Relaxed);
    let _ = counts.fetch_add(1 << 16, Relaxed);
}
