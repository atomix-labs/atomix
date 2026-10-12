//! A range not inside the value's own reads as a narrowing where it widens: `narrow` refuses one,
//! at build time, as deranged's does, and `expand` takes a range that holds its own.

use atomix_core::RangedU8;

fn main() {
    let depth = RangedU8::<1, 10>::MAX;
    let _wider = depth.narrow::<0, 50>();
}
