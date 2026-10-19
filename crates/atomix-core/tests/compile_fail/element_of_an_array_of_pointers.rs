//! An element place reads its bits from the word, and a pointer's provenance is no bits: an array
//! of pointers is loaded whole.

use core::ptr::NonNull;

use atomix_core::Atomic;
use atomix_core::ordering::Acquire;

fn main() {
    let mut value = 0_u64;
    let pointers = Atomic::new([NonNull::from(&mut value); 1]);
    let _ = pointers.element(0).load(Acquire);
}
