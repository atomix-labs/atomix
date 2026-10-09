//! A pointer word over a reference takes the reference's repr, and with it its orderings' floor: a
//! `Relaxed` load of the word would hand out a reference to what another thread wrote, before
//! those writes are seen.

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

fn relaxed(published: &Atomic<Published>) -> Published {
    published.load(Relaxed)
}

fn main() {}
