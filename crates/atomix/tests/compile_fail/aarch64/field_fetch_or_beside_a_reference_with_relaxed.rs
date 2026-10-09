//! A tag field's `fetch_or` returns the whole word, the reference among it, so its read acquires:
//! `Relaxed` would hand the reference out before the writes that made what it points to are seen.
//! Its write puts back the reference it read, so `Acquire` compiles with no `Release`; `or`, which
//! returns nothing, takes `Relaxed`. `x86_64` has no `fetch_or` at all.

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
    published.fields().stale.or(true, Relaxed);
    let _ = published.fields().stale.fetch_or(true, Acquire);
    published.fields().stale.fetch_or(true, Relaxed)
}

fn main() {}
