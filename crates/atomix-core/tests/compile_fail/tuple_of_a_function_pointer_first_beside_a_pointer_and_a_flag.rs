//! Beside two pointers, a tuple's flag takes the first one's low bits, and a function's code leaves
//! none clear: the build refuses it, naming the fix, a pointer to data first.

use core::ptr::NonNull;

use atomix_core::Atomic;

/// A node aligned to eight bytes.
#[repr(align(8))]
struct Node;

/// What handles a node's messages.
fn handle(_: u64) {}

fn main() {
    static NODE: Node = Node;
    let _ = Atomic::new((handle as fn(u64), NonNull::from(&NODE), true));
}
