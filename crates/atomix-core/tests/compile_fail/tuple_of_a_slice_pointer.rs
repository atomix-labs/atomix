//! A pointer to a slice is two words, its pointer and its length: a tuple's pointer field is one.

use core::ptr::NonNull;

use atomix_core::Atomic;

fn main() {
    let slice: &mut [u8] = &mut [1, 2, 3];
    let _ = Atomic::new((NonNull::from(slice), true));
}
