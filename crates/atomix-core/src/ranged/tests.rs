//! Each of the twelve ranged integers at the edges of its range and of its integer, each refusal
//! compared whole; its comparisons, conversions and steps in a constant; and in a debug build,
//! `new_unchecked`'s check of its contract.

use core::cmp::Ordering;
use core::iter::Step;
use core::num::NonZero;
use core::str::FromStr;

use super::{
    RangedI8, RangedI16, RangedI32, RangedI64, RangedI128, RangedIsize, RangedU8, RangedU16,
    RangedU32, RangedU64, RangedU128, RangedUsize,
};
use crate::errors::{ParseRangeError, RangeError};

#[test]
fn each_ranged_integer_takes_its_range_and_refuses_the_integers_past_it() {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            type Depth = $name<1, 10>;
            let name = stringify!($name);
            assert_eq!(Depth::new(1).map(Depth::get), Some(1), "{name}: 1 is in 1..=10");
            assert_eq!(Depth::new(10).map(Depth::get), Some(10), "{name}: and 10");
            assert_eq!(Depth::try_from(0), Err(RangeError::new(0, 1, 10)), "{name}: 0 is not");
            assert_eq!(Depth::try_from(11), Err(RangeError::new(11, 1, 10)), "{name}: nor 11");
            let largest = Depth::try_from($int::MAX);
            assert_eq!(largest, Err(RangeError::new($int::MAX, 1, 10)), "{name}: nor its largest");
        }};
    }
    each_ranged_integer!(check);
}

#[test]
fn each_ranged_integer_bounds_its_full_range_by_the_integers_own() {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            type Full = $name<{ $int::MIN }>;
            let name = stringify!($name);
            assert_eq!(Full::MIN.get(), $int::MIN, "{name}: `MIN` is the integer's smallest");
            assert_eq!(Full::MAX.get(), $int::MAX, "{name}: `MAX` defaults to its largest");
            assert_eq!(Full::new($int::MIN), Some(Full::MIN), "{name}: `new` takes the smallest");
            assert_eq!(Full::new($int::MAX), Some(Full::MAX), "{name}: the largest too");
        }};
    }
    each_ranged_integer!(check);
}

#[test]
fn each_ranged_integer_saturates_at_its_bounds() {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            type Depth = $name<1, 10>;
            let name = stringify!($name);
            assert_eq!(Depth::new_saturating($int::MIN), Depth::MIN, "{name}: up to 1");
            assert_eq!(Depth::new_saturating($int::MAX), Depth::MAX, "{name}: down to 10");
            assert_eq!(Depth::MAX.saturating_add($int::MAX), Depth::MAX, "{name}: a sum past it");
            assert_eq!(Depth::MIN.saturating_sub($int::MAX), Depth::MIN, "{name}: a difference");
            assert_eq!(Depth::MAX.saturating_mul($int::MAX), Depth::MAX, "{name}: a product");
            assert_eq!(Depth::MAX.checked_add($int::MAX), None, "{name}: none past the integer");
        }};
    }
    each_ranged_integer!(check);
}

#[test]
fn each_ranged_integer_parses_its_range_and_refuses_the_rest() {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            type Depth = $name<1, 10>;
            let name = stringify!($name);
            assert_eq!("10".parse(), Ok(Depth::MAX), "{name}: 10 is in 1..=10");
            let past = Err(ParseRangeError::Range(RangeError::new(11, 1, 10)));
            assert_eq!("11".parse::<Depth>(), past, "{name}: 11 is not");
            let malformed = $int::from_str("1x").map_err(ParseRangeError::Integer);
            assert_eq!("1x".parse::<Depth>().map(Depth::get), malformed, "{name}: nor `1x`");
        }};
    }
    each_ranged_integer!(check);
}

// Each ranged integer compares, converts and steps in a constant.
const _: () = {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            type Depth = $name<1, 10>;
            let (three, four) = (Depth::MIN.saturating_add(2), Depth::MIN.saturating_add(3));
            assert!(three == three.clone() && three != four, "equal to its clone, in const");
            assert!(three < four && matches!(three.cmp(&four), Ordering::Less), "and ordered");
            assert!($int::from(four) == 4, "to its integer");
            assert!(Depth::try_from(11).is_err(), "from one, checked");
            assert!(matches!(Depth::from_str("10"), Ok(Depth::MAX)), "from text, checked");
            assert!(matches!(Step::forward_checked(four, 6), Some(Depth::MAX)), "and stepped");
            assert!(Step::backward_checked(four, 4).is_none(), "but never past `MIN`");
        }};
    }
    each_ranged_integer!(check);
};

// Each ranged integer expands and narrows in a constant.
const _: () = {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            let depth = $name::<1, 10>::MIN.saturating_add(3);
            let kept: $name<0, 50> = depth.expand();
            assert!(kept.get() == 4, "expanded, the value kept, in const");
            assert!(depth.narrow::<5, 10>().is_none(), "narrowed past it, none");
            assert!(depth.narrow::<1, 4>() == Some($name::<1, 4>::MAX), "else the value");
        }};
    }
    each_ranged_integer!(check);
};

#[test]
fn each_ranged_integer_moves_to_any_range_checking_the_value() {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            type Depth = $name<1, 10>;
            let name = stringify!($name);
            let seven = Depth::MIN.saturating_add(6);
            let kept = seven.to_range::<5, 20>().map($name::get);
            assert_eq!(kept, Some(7), "{name}: 7 lies in 1..=10 and in 5..=20");
            assert_eq!(Depth::MIN.to_range::<5, 20>(), None, "{name}: 1 lies below 5");
            assert_eq!($name::<5, 20>::MAX.to_range::<1, 10>(), None, "{name}: 20 lies above 10");
            let (four, five) = (Depth::MIN.saturating_add(3), Depth::MIN.saturating_add(4));
            let bottom = five.to_range::<5, 20>().map($name::get);
            assert_eq!(bottom, Some(5), "{name}: 5, the new range's bottom");
            let top = Depth::MAX.to_range::<5, 10>().map($name::get);
            assert_eq!(top, Some(10), "{name}: 10, its top");
            assert_eq!(four.to_range::<5, 20>(), None, "{name}: but not 4, one below it");
            assert_eq!(Depth::MAX.to_range::<0, 9>(), None, "{name}: nor 10, one above 9");
            let wider = seven.to_range::<0, 50>().map($name::get);
            assert_eq!(wider, Some(7), "{name}: a range that holds it, as `expand` takes");
            assert_eq!(seven.to_range::<20, 30>(), None, "{name}: and none in a range apart");
        }};
    }
    each_ranged_integer!(check);
}

// Each ranged integer moves to a range that only overlaps its own in a constant.
const _: () = {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            let seven = $name::<1, 10>::MIN.saturating_add(6);
            let kept = seven.to_range::<5, 20>();
            assert!(matches!(kept, Some(levels) if levels.get() == 7), "7 kept, in const");
            assert!($name::<1, 10>::MIN.to_range::<5, 20>().is_none(), "and 1 refused");
        }};
    }
    each_ranged_integer!(check);
};

// Each ranged integer converts to and from its `NonZero` in a constant.
const _: () = {
    macro_rules! check {
        ($name:ident($int:ident)) => {{
            type Depth = $name<1, 10>;
            type Full = $name<{ NonZero::<$int>::MIN.get() }>;
            let four = NonZero::<$int>::new(4).expect("4 is not zero");
            let eleven = NonZero::<$int>::new(11).expect("11 is not zero");
            let depth = Depth::MIN.saturating_add(3);
            assert!(Depth::try_from(four) == Ok(depth), "from a nonzero, in const");
            let refused = Err(RangeError::new(11, 1, 10));
            assert!(Depth::try_from(eleven) == refused, "or refused, whole");
            let Ok(ten) = NonZero::<$int>::try_from(Depth::MAX) else {
                panic!("to a nonzero, checked");
            };
            assert!(ten.get() == 10, "the value kept");
            assert!(NonZero::<$int>::try_from($name::<0, 10>::MIN).is_err(), "refusing zero");
            assert!(Depth::MAX.to_non_zero().get() == 10, "unchecked, with no zero in range");
            assert!(Full::from_non_zero(four).get() == 4, "and from one into every nonzero");
        }};
    }
    each_ranged_integer!(check);
};

// Each ranged integer converts to and from every other, and every other's `NonZero`, in a constant.
const _: () = {
    macro_rules! check {
        ($source:ident($source_integer:ident) => $target:ident($target_integer:ident)) => {{
            type Source = $source<1, 100>;
            type Target = $target<1, 10>;
            let (five, fifty) = (Source::MIN.saturating_add(4), Source::MIN.saturating_add(49));
            let target_five = Target::MIN.saturating_add(4);
            assert!(Target::try_from(five) == Ok(target_five), "from another, in const");
            let refused = Err(RangeError::<$source_integer, $target_integer>::new(50, 1, 10));
            assert!(Target::try_from(fifty) == refused, "or refused, whole");
            let non_zero = NonZero::<$source_integer>::new(5).expect("5 is not zero");
            assert!(Target::try_from(non_zero) == Ok(target_five), "from another's nonzero");
            let Ok(non_zero_five) = NonZero::<$target_integer>::try_from(five) else {
                panic!("to another's nonzero, checked");
            };
            assert!(non_zero_five.get() == 5, "the value kept");
            let zero = $source::<0, 10>::MIN;
            assert!(NonZero::<$target_integer>::try_from(zero).is_err(), "refusing zero");
        }};
    }
    each_ranged_integer!(pairs check);
};

#[test]
fn a_value_no_integer_of_the_range_holds_comes_back_as_it_was() {
    let wide = RangedU16::<0, 300>::MAX;
    let refused = Err(RangeError::new(300_u16, 0_u8, 255));
    assert_eq!(RangedU8::<0, 255>::try_from(wide), refused, "300, past every `u8`");
    let negative = RangedI8::<-5, 5>::MIN;
    let refused = Err(RangeError::new(-5_i8, 0_u8, 10));
    assert_eq!(RangedU8::<0, 10>::try_from(negative), refused, "and -5, below every `u8`");
}

// A range error compares and clones in a constant.
const _: () = {
    use core::marker::Destruct;

    /// Whether `refused` equals its clone, run as `const`: through a bound, so no lint rewrites the
    /// clone of a `Copy` value into the value.
    const fn is_its_clone<E: [const] Clone + [const] PartialEq + [const] Destruct>(
        refused: &E,
    ) -> bool {
        refused.clone() == *refused
    }
    let refused = RangeError::new(11_u8, 1, 10);
    assert!(is_its_clone(&refused), "equal to its clone, in const");
    assert!(refused != RangeError::new(12, 1, 10), "and to no other refusal");
};

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "an integer outside `MIN..=MAX`")]
fn new_unchecked_outside_the_range_panics_in_a_debug_build() {
    // SAFETY: the `cfg` is this crate's own, so `new_unchecked`'s `debug_assert!` is built, and it
    // panics on 0, below 1, before anything is built from it.
    #[expect(unsafe_code, reason = "the unchecked constructor, given 0, below its range")]
    let _depth = unsafe { RangedU8::<1, 10>::new_unchecked(0) };
}
