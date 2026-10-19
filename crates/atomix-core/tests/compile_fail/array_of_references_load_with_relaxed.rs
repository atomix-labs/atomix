//! An array of references keeps a reference's floor, as every value that holds one does: a
//! `Relaxed` load would hand out references to writes not yet seen.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

/// What a load hands out a reference to.
static DEFAULT: u64 = 7;

fn main() {
    let configs = Atomic::new([&DEFAULT]);
    let _ = configs.load(Relaxed);
}
