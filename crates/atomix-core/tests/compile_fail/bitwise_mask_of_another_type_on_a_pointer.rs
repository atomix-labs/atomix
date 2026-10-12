//! A raw pointer's mask is a `usize`, as core's `AtomicPtr` takes: a mask of
//! another type is refused.

use core::ptr;

use atomix_core::AtomicPtr;
use atomix_core::ordering::Relaxed;

fn main() {
    let next = AtomicPtr::new(ptr::null_mut::<u64>());
    next.or(0b1_u32, Relaxed);
}
