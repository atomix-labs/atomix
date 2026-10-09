//! An exclusive reference is not `Copy`: two loads of it would be two exclusive references to the
//! same place.

use atomix_core::Atomic;

fn main() {
    let mut value = 7_u64;
    let _ = Atomic::new(&mut value);
}
