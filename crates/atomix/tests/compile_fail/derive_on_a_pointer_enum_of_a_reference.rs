//! A pointer enum's variants share one pointer, which its decode builds of the bits of a raw one:
//! a reference, which no bits forge, keeps tags in a pointer word, but in no pointer enum.

use atomix::Atom;

/// A node of a list.
struct Node;

#[derive(Clone, Copy, Atom)]
enum Next {
    End,
    Node(&'static Node),
}

fn main() {}
