//! Three `u64`s take 192 bits, past the 128 an atomic holds.

use atomix_core::Atomic;

fn main() {
    let _ = Atomic::new((1_u64, 2_u64, 3_u64));
}
