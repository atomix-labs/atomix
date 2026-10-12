//! A carry out of a field below the top would reach the field above it: a field adds in place only
//! where it ends at the repr's top bit, its `TopField`.

use atomix::ordering::Relaxed;
use atomix::{Atom, Atomic};

/// A count, then a count above it at the top.
#[derive(Clone, Copy, Atom)]
struct Counts {
    low: u32,
    high: u32,
}

fn main() {
    let counts = Atomic::new(Counts { low: 1, high: 1 });
    counts.fields().high.fetch_add(1, Relaxed);
    counts.fields().low.fetch_add(1, Relaxed);
}
