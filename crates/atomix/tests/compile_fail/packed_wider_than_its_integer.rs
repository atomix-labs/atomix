//! Five bytes take 40 bits, past the `u32` a `Packed` states for them: the build refuses it, naming
//! the bits they take.

use atomix::{Atomic, Packed};

fn main() {
    let _ = Atomic::new(Packed::<[u8; 5], u32>::new([0; 5]));
}
