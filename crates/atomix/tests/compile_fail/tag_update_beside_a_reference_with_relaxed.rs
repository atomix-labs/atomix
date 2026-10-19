//! A tag's `update` returns the word, the reference beside the tag with it: its read takes
//! `Acquire`, as a reference's load does.

use atomix::ordering::{Acquire, Relaxed};
use atomix::{Atom, Atomic};

/// What a trading service reads on every order, aligned to 4 by its `u32`: two low bits clear.
struct Config {
    depth: u32,
}

/// The config in force, and whether a newer one is on its way.
#[derive(Clone, Copy, Atom)]
struct Published {
    config: &'static Config,
    stale: bool,
}

/// The configuration before any is published.
static DEFAULT: Config = Config { depth: 10 };

fn main() {
    let published = Atomic::new(Published { config: &DEFAULT, stale: false });
    let _ = published.fields().stale.update(Relaxed, Acquire, |stale| !stale);
}
