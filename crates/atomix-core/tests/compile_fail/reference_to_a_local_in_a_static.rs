//! An atomic of a `'static` reference takes no reference to a local, which would dangle once the
//! local's frame returns.

use atomix_core::Atomic;
use atomix_core::ordering::Release;

/// What the reference points to before the store.
static DEFAULT: u64 = 7;

/// The reference.
static CONFIG: Atomic<&'static u64> = Atomic::new(&DEFAULT);

fn main() {
    let local = 8_u64;
    CONFIG.store(&local, Release);
}
