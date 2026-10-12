//! An atomic is a place, not a value: one inside another is refused with a message of its own,
//! not `Atom`'s advice to derive it.

use atomix::{Atomic, AtomicU64};

/// Takes an atomic that would hold another's place.
fn read(_: &Atomic<AtomicU64>) {}

fn main() {}
