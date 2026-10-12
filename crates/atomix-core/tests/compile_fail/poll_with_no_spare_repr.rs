//! `Pending` takes a spare repr, as `None` does, and a `u32` spends every repr on its values.

use core::task::Poll;

use atomix_core::Atomic;

fn main() {
    let _ = Atomic::new(Poll::Ready(5_u32));
}
