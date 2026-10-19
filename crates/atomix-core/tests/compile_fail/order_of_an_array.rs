//! An array has no maximum or minimum: `ldumax` compares the whole word, which orders an array by
//! its last element first, not as `Ord` orders arrays, element 0 first.

use atomix_core::AtomOrd;

/// Compiles only where `T`'s reprs order as its values.
const fn orders_as_its_reprs<T: AtomOrd>() {}

fn main() {
    orders_as_its_reprs::<[u8; 4]>();
}
