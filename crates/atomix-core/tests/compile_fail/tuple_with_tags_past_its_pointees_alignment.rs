//! A byte beside a pointer shares the pointer's word, in the eight low bits its pointee's alignment
//! must leave clear: an 8-aligned node leaves three, so the build refuses it, naming the fix.

use core::ptr::NonNull;

use atomix_core::Atomic;

/// A node aligned to eight bytes.
#[repr(align(8))]
struct Node;

fn main() {
    let node = Box::leak(Box::new(Node));
    let _ = Atomic::new((NonNull::from(node), 7_u8));
}
