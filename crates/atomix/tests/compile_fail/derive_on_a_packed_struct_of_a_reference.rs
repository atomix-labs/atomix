//! A packed struct's fields share an integer's bits, and a reference's repr is no integer's: its
//! bits forge no reference, so it packs beside no other field.

use atomix::Atom;

#[derive(Clone, Copy, Atom)]
struct Current {
    config: &'static u64,
    armed: bool,
}

fn main() {}
