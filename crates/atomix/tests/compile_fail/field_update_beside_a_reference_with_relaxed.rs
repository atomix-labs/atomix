//! A tag field's `update` returns the whole word, the reference among it, so its exchange's read
//! acquires: `Relaxed` would hand the reference out before the writes that made what it points to
//! are seen. Its write puts back the reference it read, so `Acquire` compiles with no `Release`.

use atomix::ordering::{Acquire, Relaxed};
use atomix::{Atom, Atomic};

/// A configuration a service reads on every order, aligned to 4: two low bits clear.
struct Config {
    depth: u32,
}

/// The configuration in force, and whether it is stale.
#[derive(Clone, Copy, Atom)]
struct Published {
    config: &'static Config,
    stale: bool,
}

fn mark_stale(published: &Atomic<Published>) -> Published {
    let _ = published.fields().stale.update(Acquire, Acquire, |_| true);
    published.fields().stale.update(Relaxed, Acquire, |_| true)
}

fn main() {}
