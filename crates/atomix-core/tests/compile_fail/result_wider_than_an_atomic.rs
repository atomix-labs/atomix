//! A `u128` payload and its tag take 129 bits, more than any atomic holds.

use atomix_core::Atomic;

fn main() {
    let _ = Atomic::new(Ok::<u128, u8>(1));
}
