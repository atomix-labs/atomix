//! A range short of some nonzero integer could not hold every `NonZero`: `from_non_zero` refuses
//! one, at build time, rather than check it, which `try_from` does.

use core::num::NonZero;

use atomix_core::RangedU8;

fn main() {
    let _depth = RangedU8::<1, 10>::from_non_zero(NonZero::<u8>::MIN);
}
