//! A `u128` payload and `Bound`'s two-bit tag take 130 bits, more than any atomic holds.

use core::ops::Bound;

use atomix_core::Atomic;

fn main() {
    let _ = Atomic::new(Bound::Included(1_u128));
}
