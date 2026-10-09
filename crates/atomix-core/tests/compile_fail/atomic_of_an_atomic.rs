//! An atomic is a place, not a value, and no `Atom`: one inside another is refused, as a type, and
//! inside an `Option`.

use atomix_core::{Atomic, AtomicU64};

/// The atomic as a type, inside an atomic.
fn read(_: &Atomic<AtomicU64>) {}

/// And inside an `Option` inside an atomic.
static SLOT: Atomic<Option<AtomicU64>> = Atomic::new(None);

fn main() {}
