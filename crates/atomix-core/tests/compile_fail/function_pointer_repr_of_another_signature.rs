//! No repr one signature gave decodes as another, whose call would take arguments it does not: a
//! function pointer's repr names its signature.

use atomix_core::Atom;

fn narrow(value: u8) -> u8 {
    value
}

fn main() {
    let repr = (narrow as fn(u8) -> u8).to_repr();
    let _ = <fn(u64) -> u64 as Atom>::from_repr(repr);
}
