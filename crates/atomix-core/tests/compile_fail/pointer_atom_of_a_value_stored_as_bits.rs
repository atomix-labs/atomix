//! A tag shares a pointer's low bits: a value stored as an integer is no `PointerAtom`, and generic
//! code that asks for one refuses it.

use atomix_core::PointerAtom;

/// Compiles only where `T` is stored as a pointer.
fn stored_as_a_pointer<T: PointerAtom>() {}

fn main() {
    stored_as_a_pointer::<*mut u64>();
    stored_as_a_pointer::<u64>();
}
