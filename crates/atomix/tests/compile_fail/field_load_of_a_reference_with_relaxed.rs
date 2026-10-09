//! A pointer word's reference field is read through the word, and takes the reference's floor: a
//! `Relaxed` load of the field would hand it out before the writes that made what it points to
//! are seen, though a load of the word's tag field, which hands out its bits alone, takes every
//! ordering.

use atomix::ordering::Relaxed;
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

fn relaxed(published: &Atomic<Published>) -> &'static Config {
    let _ = published.fields().stale.load(Relaxed);
    published.fields().config.load(Relaxed)
}

fn main() {}
