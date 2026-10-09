//! A packed struct's fields share an integer's bits, and a function pointer's repr is no
//! integer's: its bits forge no function, so it packs beside no other field.

use atomix::Atom;

#[derive(Clone, Copy, Atom)]
struct Callback {
    hook: fn(u8) -> u8,
    armed: bool,
}

fn main() {}
