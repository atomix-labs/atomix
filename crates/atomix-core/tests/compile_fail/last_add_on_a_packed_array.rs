//! A `Packed`'s last element adds in place nowhere, even where its elements fill the integer it
//! states: whether they do is a constant, not a type a bound can name, so its `last()` has no
//! `fetch_add`.

use atomix_core::ordering::Relaxed;
use atomix_core::{Atomic, Packed};

fn main() {
    let counts = Atomic::new(Packed::<[u16; 2], u32>::new([0; 2]));
    let _ = counts.last().fetch_add(1, Relaxed);
}
