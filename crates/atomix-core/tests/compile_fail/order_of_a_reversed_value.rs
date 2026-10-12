//! `Reverse` orders its values backwards and its repr forwards, so an atomic maximum would keep the
//! least: a reversed value has no `AtomOrd`.

use core::cmp::Reverse;

use atomix_core::AtomOrd;

/// Compiles only where `T`'s reprs order as its values.
const fn orders_as_its_reprs<T: AtomOrd>() {}

fn main() {
    orders_as_its_reprs::<Reverse<u32>>();
}
