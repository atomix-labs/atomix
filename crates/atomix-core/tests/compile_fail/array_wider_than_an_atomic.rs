//! An array takes a slot of its element's repr each, and an atomic holds 128 bits at most:
//! seventeen bytes are past it.

use atomix_core::Atomic;

fn main() {
    let _ = Atomic::new([0_u8; 17]);
}
