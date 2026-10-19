//! arbitrary's `Arbitrary`: an atomic as its value, a ranged integer inside its range, and a
//! `Packed` as its array.

use arbitrary::{Arbitrary, MaxRecursionReached, Result, Unstructured};

use crate::atom::{Atom, Packed};
use crate::atomic::Atomic;
use crate::ranged::{self, each_ranged_integer};

/// Generates a `T`, and holds it in a new atomic.
///
/// Each method is `T`'s, so an atomic reads the bytes its value reads, and hints as many.
impl<'a, T: Atom + Arbitrary<'a>> Arbitrary<'a> for Atomic<T> {
    #[inline]
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        T::arbitrary(input).map(Self::from)
    }

    #[inline]
    fn arbitrary_take_rest(input: Unstructured<'a>) -> Result<Self> {
        T::arbitrary_take_rest(input).map(Self::from)
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        T::size_hint(depth)
    }

    #[inline]
    fn try_size_hint(depth: usize) -> Result<(usize, Option<usize>), MaxRecursionReached> {
        T::try_size_hint(depth)
    }
}

/// Generates an array, and packs it.
///
/// Each method is the array's, so a `Packed` reads the bytes its array reads, and hints as many.
impl<'a, A: Arbitrary<'a>, R> Arbitrary<'a> for Packed<A, R> {
    #[inline]
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        A::arbitrary(input).map(Self::new)
    }

    #[inline]
    fn arbitrary_take_rest(input: Unstructured<'a>) -> Result<Self> {
        A::arbitrary_take_rest(input).map(Self::new)
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        A::size_hint(depth)
    }

    #[inline]
    fn try_size_hint(depth: usize) -> Result<(usize, Option<usize>), MaxRecursionReached> {
        A::try_size_hint(depth)
    }
}

/// Implements `Arbitrary` for a ranged integer, `$name`, over `$int`.
macro_rules! ranged {
    ($name:ident($int:ident)) => {
        /// Generates an integer from `MIN` to `MAX`, and reads as few bytes as span the range.
        ///
        /// Every integer of the range is reached, and an empty input gives `MIN`. The size hint is
        /// exact: the bytes that hold `MAX - MIN`, so none where the range holds one integer.
        impl<'a, const MIN: $int, const MAX: $int> Arbitrary<'a> for ranged::$name<MIN, MAX> {
            #[inline]
            fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
                // `int_in_range` reads the bytes that span the range, then takes their
                // remainder by its length, so it stays inside; `new_saturating` keeps it
                // there without trusting arbitrary to.
                input.int_in_range(MIN..=MAX).map(Self::new_saturating)
            }

            #[inline]
            fn size_hint(_depth: usize) -> (usize, Option<usize>) {
                // The bytes `int_in_range` reads: the integer's, less the leading bytes that
                // `MAX - MIN` leaves zero.
                let bytes = size_of::<$int>()
                    .strict_sub(MAX.abs_diff(MIN).leading_zeros().strict_div(8).strict_cast());
                (bytes, Some(bytes))
            }
        }
    };
}

each_ranged_integer!(ranged);
