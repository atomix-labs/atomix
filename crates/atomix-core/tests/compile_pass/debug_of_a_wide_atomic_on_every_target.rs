//! `Debug` derives over a 128-bit atomic and an atomic of two pointers wherever the target holds
//! 16 bytes atomically, LSE2 or AVX or neither: each prints what a load reads, or else the
//! compare-exchange `load_rmw` is.

use core::ptr::NonNull;

use atomix_core::{Atomic, AtomicU128};

/// The best bid and ask, and the volume traded at them.
#[derive(Debug)]
struct Book {
    _best: Atomic<(Option<NonNull<u64>>, Option<NonNull<u64>>)>,
    _volume: AtomicU128,
}

fn main() {
    let book = Book { _best: Atomic::new((None, None)), _volume: AtomicU128::new(0) };
    assert_eq!(format!("{book:?}"), "Book { _best: (None, None), _volume: 0 }", "every field");
}
