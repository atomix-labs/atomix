//! An array of no elements takes no slot, so no primitive holds it: the unit is the value that
//! holds nothing.

use atomix_core::Atomic;

fn main() {
    let _ = Atomic::new([0_u8; 0]);
}
