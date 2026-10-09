//! A `Relaxed` store of a reference would publish it before the writes that made what it points
//! to, so another thread could read through it while they race.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

/// What the reference points to before the store.
static DEFAULT: u64 = 7;

/// The reference.
static CONFIG: Atomic<&'static u64> = Atomic::new(&DEFAULT);

fn main() {
    CONFIG.store(Box::leak(Box::new(8)), Relaxed);
}
