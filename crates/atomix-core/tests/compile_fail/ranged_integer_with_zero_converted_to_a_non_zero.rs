//! A range that holds zero could hand `NonZero` a zero: `to_non_zero` refuses one, at build time,
//! rather than check it, which `NonZero::try_from` does.

use atomix_core::RangedU8;

fn main() {
    let _count = RangedU8::<0, 10>::MAX.to_non_zero();
}
