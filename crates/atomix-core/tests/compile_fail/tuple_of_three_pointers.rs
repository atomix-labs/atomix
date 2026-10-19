//! Two pointers take a double word, the widest atomic: a third has no word to go in.

use core::ptr::NonNull;

use atomix_core::Atomic;

fn main() {
    let mut value = 0_u64;
    let pointer = NonNull::from(&mut value);
    let _ = Atomic::new((pointer, pointer, pointer));
}
