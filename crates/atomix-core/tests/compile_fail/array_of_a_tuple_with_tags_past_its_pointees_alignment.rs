//! An array encodes each element with the tags it passes, as a word holding the element would, so
//! a tuple whose byte needs more low bits than its pointer's pointee leaves clear is refused in an
//! array too.

use core::ptr::NonNull;

use atomix_core::Atomic;

/// A node aligned to eight bytes.
#[repr(align(8))]
struct Node;

fn main() {
    let node = Box::leak(Box::new(Node));
    let _ = Atomic::new([(NonNull::from(node), 7_u8)]);
}
