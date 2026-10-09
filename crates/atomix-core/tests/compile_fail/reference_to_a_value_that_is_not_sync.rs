//! A reference crosses threads with its atomic, so what it points to must be `Sync`: a `Cell`,
//! changed through a shared reference without a lock, is not.

use core::cell::Cell;

use atomix_core::Atomic;

fn main() {
    let cell = Cell::new(7_u64);
    let _ = Atomic::new(&cell);
}
