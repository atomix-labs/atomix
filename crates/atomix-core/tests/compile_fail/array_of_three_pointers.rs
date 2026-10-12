//! Two pointers take a double word, the widest atomic: a third has no word to go in.

use core::ptr::NonNull;

use atomix_core::Atomic;

fn main() {
    let mut value = 0_u64;
    let _ = Atomic::new([NonNull::from(&mut value); 3]);
}
