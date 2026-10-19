//! A `Packed` of no elements has no field to take its range from: the build refuses it.

use atomix_core::{Atomic, Packed};

fn main() {
    let _ = Atomic::new(Packed::<[u8; 0], u8>::new([]));
}
