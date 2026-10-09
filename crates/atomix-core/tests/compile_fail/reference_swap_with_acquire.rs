//! An `Acquire` swap of a reference sees what the reference it reads points to, but publishes
//! nothing of the one it writes: a read-modify-write of a reference takes `AcqRel` or `SeqCst`.

use atomix_core::Atomic;
use atomix_core::ordering::Acquire;

/// What the reference points to before the swap.
static DEFAULT: u64 = 7;

/// The reference.
static CONFIG: Atomic<&'static u64> = Atomic::new(&DEFAULT);

fn main() {
    let _ = CONFIG.swap(Box::leak(Box::new(8)), Acquire);
}
