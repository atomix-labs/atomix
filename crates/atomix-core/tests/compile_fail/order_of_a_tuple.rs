//! A tuple has no maximum or minimum: `ldumax` compares the whole word, which orders a tuple by its
//! last field first, not as `Ord` orders tuples, field 0 first.

use atomix_core::AtomOrd;

/// Compiles only where `T`'s reprs order as its values.
const fn orders_as_its_reprs<T: AtomOrd>() {}

fn main() {
    orders_as_its_reprs::<(u16, u16)>();
}
