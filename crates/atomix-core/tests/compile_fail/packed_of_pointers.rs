//! A `Packed` lays its elements' bits side by side, and a pointer's provenance is no bits: a
//! `Packed` of references or of pointers is refused.

use core::ptr::NonNull;

use atomix_core::{Atom, Packed};

/// Compiles only where `T` is a value an atomic holds.
const fn is_an_atom<T: Atom>() {}

fn main() {
    is_an_atom::<Packed<[&'static u32; 2], u64>>();
    is_an_atom::<Packed<[NonNull<u32>; 2], u64>>();
}
