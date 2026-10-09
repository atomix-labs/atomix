//! No alignment of code leaves a function pointer's low bits clear, so it keeps no tag there: a
//! word of one beside a tag is refused, naming the word of their own the tags take.

use atomix::Atom;

#[derive(Clone, Copy, Atom)]
struct Callback {
    hook: fn(u8) -> u8,
    armed: bool,
}

fn main() {}
