//! `Atom` for core's ranges that are `Copy`: one of two bounds is stored as the array of its
//! bounds, the start lowest, `Range<u32>` a `u64`; one of one bound, as that bound.

use core::ops::{RangeTo, RangeToInclusive as LegacyRangeToInclusive};
use core::range::{Range, RangeFrom, RangeInclusive, RangeToInclusive};

use super::Atom;
use super::scalar::wrappers;
use crate::range::{PointeeAlignment, ReprRange, Tags};

/// Implements `Atom` for a range of two bounds, stored as the array of them, the start first.
macro_rules! two_bounds {
    ($($range:ident { $start:ident, $end:ident })+) => {$(
        // SAFETY: the repr, its range, its tags, the validity and the threads rule are those of the
        // array of the bounds, which `from_repr` decodes, unchecked as checked, and takes apart.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl<T: [const] Atom> Atom for $range<T>
        where
            [T; 2]: [const] Atom,
        {
            type Repr = <[T; 2] as Atom>::Repr;
            type Validity = <[T; 2] as Atom>::Validity;
            const REPRS: ReprRange<Self::Repr> = <[T; 2] as Atom>::REPRS;
            const TAG_WIDTH: u32 = <[T; 2] as Atom>::TAG_WIDTH;
            const POINTEE_ALIGNMENT: PointeeAlignment = <[T; 2] as Atom>::POINTEE_ALIGNMENT;
            #[inline]
            fn to_repr(self) -> Self::Repr {
                [self.$start, self.$end].to_repr()
            }
            #[inline]
            fn to_tagged_repr(self, tags: Tags) -> (Self::Repr, usize) {
                [self.$start, self.$end].to_tagged_repr(tags)
            }
            #[inline]
            fn from_repr(repr: Self::Repr) -> Option<Self> {
                match <[T; 2] as Atom>::from_repr(repr) {
                    Some([$start, $end]) => Some(Self { $start, $end }),
                    None => None,
                }
            }
            #[inline]
            unsafe fn from_repr_unchecked(repr: Self::Repr) -> Self {
                // SAFETY: the caller's repr decodes as the range, so as the array of its bounds.
                let [$start, $end] = unsafe { <[T; 2] as Atom>::from_repr_unchecked(repr) };
                Self { $start, $end }
            }
        }
    )+};
}

two_bounds! {
    Range { start, end }
    RangeInclusive { start, last }
}

// A range of one bound is that bound, and takes none of its capabilities: no range combines with
// another range, nor orders.
wrappers! {
    RangeFrom(range => range.start, start => RangeFrom { start }): [];
    RangeToInclusive(range => range.last, last => RangeToInclusive { last }): [];
    RangeTo(range => range.end, end => RangeTo { end }): [];
    LegacyRangeToInclusive(range => range.end, end => LegacyRangeToInclusive { end }): [];
}
