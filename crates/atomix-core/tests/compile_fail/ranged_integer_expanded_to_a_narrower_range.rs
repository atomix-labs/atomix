//! A range that does not hold the value's own could lose the value: `expand` refuses one, at
//! build time, rather than check it, which `narrow` and `to_range` do.

use atomix_core::RangedU8;

fn main() {
    let depth = RangedU8::<1, 10>::MAX;
    let _shallow: RangedU8<1, 5> = depth.expand();
}
