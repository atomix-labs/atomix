//! A `Relaxed` load of a reference would hand out a reference to what another thread wrote, before
//! those writes are seen: reading through it would race with them.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

/// What a load hands out a reference to.
static DEFAULT: u64 = 7;

/// The reference.
static CONFIG: Atomic<&'static u64> = Atomic::new(&DEFAULT);

fn main() {
    let _ = CONFIG.load(Relaxed);
}
