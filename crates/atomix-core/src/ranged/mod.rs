//! The ranged integers, [`RangedU8`] to [`RangedIsize`]: each an integer held to its range.

use core::cmp::Ordering;
use core::fmt::{
    self, Binary, Debug, Display, Formatter, LowerExp, LowerHex, Octal, UpperExp, UpperHex,
};
use core::hash::{Hash, Hasher};
use core::iter::Step;
use core::marker::StructuralPartialEq;
use core::mem::transmute_neo;
use core::num::{NonZero, TryFromIntError};
use core::pattern_type;
use core::str::FromStr;

use crate::errors::{ParseRangeError, RangeError};

/// Calls `$macro!($name($int))` for each ranged integer, `$name`, over its integer, `$int`, or,
/// with `pairs`, `$macro!($source($source_integer) => $target($target_integer))` for each pair of
/// two of them: the one list of the twelve, which defines them, converts each to each other, and
/// gives them each integration's traits.
macro_rules! each_ranged_integer {
    (@list [$($pairs:ident)?] $macro:ident) => {
        each_ranged_integer! {
            @each [$($pairs)?] $macro
            RangedU8(u8) RangedU16(u16) RangedU32(u32) RangedU64(u64) RangedU128(u128)
            RangedUsize(usize) RangedI8(i8) RangedI16(i16) RangedI32(i32) RangedI64(i64)
            RangedI128(i128) RangedIsize(isize)
        }
    };
    (@each [] $macro:ident $($name:ident($int:ident))+) => {
        $($macro!($name($int));)+
    };
    (@each [pairs] $macro:ident $($list:tt)+) => {
        each_ranged_integer!(@pairs $macro [] $($list)+);
    };
    // Each ranged integer as the target of each one before or after it in the list.
    (
        @pairs $macro:ident [$($before:tt)*] $target:ident($target_integer:ident) $($after:tt)*
    ) => {
        each_ranged_integer!(@sources $macro $target($target_integer) $($before)* $($after)*);
        each_ranged_integer!(@pairs $macro [$($before)* $target($target_integer)] $($after)*);
    };
    (@pairs $macro:ident [$($before:tt)*]) => {};
    (
        @sources $macro:ident $target:ident($target_integer:ident)
        $($source:ident($source_integer:ident))*
    ) => {
        $($macro!($source($source_integer) => $target($target_integer));)*
    };
    (pairs $macro:ident) => {
        each_ranged_integer!(@list [pairs] $macro);
    };
    ($macro:ident) => {
        each_ranged_integer!(@list [] $macro);
    };
}

// For the integrations, which name it by its path.
#[cfg(any(
    feature = "arbitrary",
    feature = "bytemuck",
    feature = "deranged-05",
    feature = "serde"
))]
pub(crate) use each_ranged_integer;

/// Defines a ranged integer, `$name`, over its integer, `$int`, one section at a time.
macro_rules! ranged {
    ($name:ident($int:ident)) => {
        ranged!(@type $name $int);
        ranged!(@construct $name $int);
        ranged!(@convert_within $name $int);
        ranged!(@arithmetic $name $int);
        // The traits are written over `get()`: the field's pattern type implements only `Clone`
        // and `Copy`, so no derive, std's or derive_more's, can read it. Each is `const` where
        // `NonZero`'s is.
        ranged!(@compare $name $int);
        ranged!(
            @format $name $int: Debug, Display, Binary, Octal, LowerHex, UpperHex, LowerExp, UpperExp
        );
        ranged!(@convert $name $int);
        ranged!(@step $name $int);
    };

    // The type, its docs, and the niche its range leaves an `Option`.
    (@type $name:ident $int:ident) => {
        #[doc = concat!("A `", stringify!($int), "` held to `MIN..=MAX`.")]
        ///
        /// Built as core's [`NonZero`] is: the private field is a pattern type, so the range is a
        /// validity invariant, which Miri checks, and where the range leaves an integer out, an
        /// `Option` keeps its `None` there, in the integer's own width. As with `NonZero`, an
        /// operation is infallible only where the range is closed under it: [`min`](Ord::min),
        /// [`max`](Ord::max) and [`clamp`](Ord::clamp) are, and arithmetic is `checked_` or
        /// `saturating_`. There is no `Default`, since zero may lie outside the range. Its
        /// comparisons, conversions and steps are `const`, as `NonZero`'s are.
        ///
        /// It converts to its integer with `From`, and with `TryFrom` from its integer, from any
        /// `NonZero` and from any other ranged integer, and to any `NonZero`, each checked, as core
        /// converts integers of two types: whether a value is lost depends on the ranges, which no
        /// trait bound states without `generic_const_exprs`. Within its integer,
        /// [`expand`](Self::expand) moves it to a range that holds its own,
        /// [`narrow`](Self::narrow) to one inside its own, checking the value, and
        /// [`to_range`](Self::to_range) to any other, checking it too; and
        /// [`to_non_zero`](Self::to_non_zero) and [`from_non_zero`](Self::from_non_zero) convert
        /// where no value is lost. `expand`, `narrow` and those two refuse the build where the
        /// ranges do not fit so: `cargo build` reports it, and `cargo check` only where a constant
        /// makes the call, since it evaluates constants but builds no function.
        ///
        /// Wherever its integer is an [`Atom`](crate::Atom), so is it: an [`Atomic`](crate::Atomic)
        /// stores it as its integer, and an `Option`'s `None` as an integer outside the range, so
        /// an atomic of a full range's `Option` is refused. It has [`AtomOrd`](crate::AtomOrd), as
        /// the larger or smaller of two values in the range is in it, but neither
        /// [`AtomAdd`](crate::AtomAdd) nor [`AtomBitwise`](crate::AtomBitwise): a sum or an or of
        /// two can leave it.
        ///
        #[doc = concat!(
            "`MAX` defaults to [`", stringify!($int), "::MAX`]. ",
            "A `MIN` above `MAX` names no type: rustc refuses it where it is used, at ",
            "`cargo build`, and at `cargo check` only where a constant uses it, in a message that ",
            "quotes the bounds but names no file or line, so search the code for them. ",
            "A crate that uses the type needs no `#![feature]`."
        )]
        ///
        /// # Examples
        /// ```
        /// # extern crate atomix_core as atomix;
        #[doc = concat!("use atomix::{ParseRangeError, ", stringify!($name), "};")]
        ///
        /// /// How many levels of an order book a feed sends, 1 to 10.
        #[doc = concat!("type Depth = ", stringify!($name), "<1, 10>;")]
        ///
        /// let depth = Depth::new(4).expect("4 levels is from 1 to 10");
        /// assert_eq!(depth.get(), 4, "the integer back");
        #[doc = concat!(
            "assert_eq!(size_of::<Option<Depth>>(), size_of::<", stringify!($int), ">(), ",
            "\"an `Option` in the integer's width\");"
        )]
        /// assert_eq!(depth.checked_add(7), None, "11 levels, past 10");
        /// assert_eq!(depth.saturating_add(7), Depth::MAX, "or 10, saturating");
        /// assert_eq!("10".parse::<Depth>()?, Depth::MAX, "and 10 parsed from text");
        /// let refused = Depth::try_from(11).expect_err("11 is past 10");
        /// assert_eq!(refused.to_string(), "range error: 11 is not in 1..=10", "refused by range");
        #[doc = concat!("# Ok::<(), ParseRangeError<", stringify!($int), ">>(())")]
        /// ```
        ///
        /// [`NonZero`]: core::num::NonZero
        #[repr(transparent)]
        #[derive(Copy)]
        #[derive_const(Clone)]
        pub struct $name<const MIN: $int, const MAX: $int = { $int::MAX }>(
            /// The integer, as a pattern type whose values are those from `MIN` to `MAX`.
            pattern_type!($int is MIN..=MAX),
        );

        const _: () = assert!(
            size_of::<Option<$name<1>>>() == size_of::<$int>(),
            "an `Option` keeps its `None` outside the range, in the integer's width"
        );
    };

    // Building a value, and reading its integer back.
    (@construct $name:ident $int:ident) => {
        impl<const MIN: $int, const MAX: $int> $name<MIN, MAX> {
            /// The smallest value, `MIN`.
            pub const MIN: Self = Self::new_saturating(MIN);
            /// The largest value, `MAX`.
            pub const MAX: Self = Self::new_saturating(MAX);

            /// The value of `integer`, or `None` where it lies outside the range.
            #[expect(unsafe_code, reason = "builds the value once its range is checked")]
            #[inline]
            #[must_use]
            pub const fn new(integer: $int) -> Option<Self> {
                if MIN <= integer && integer <= MAX {
                    // SAFETY: `integer` lies from `MIN` to `MAX`, just checked.
                    Some(unsafe { Self::new_unchecked(integer) })
                } else {
                    None
                }
            }

            /// The value of `integer`, unchecked.
            ///
            /// # Safety
            /// `integer` lies from `MIN` to `MAX`.
            #[expect(unsafe_code, reason = "a pattern type is built only by a transmute")]
            #[inline]
            #[must_use]
            pub const unsafe fn new_unchecked(integer: $int) -> Self {
                debug_assert!(MIN <= integer && integer <= MAX, "an integer outside `MIN..=MAX`");
                // SAFETY: `Self` is a transparent pattern type over the integer, so of its size,
                // and the caller's `integer` lies in the pattern's range, `MIN` to `MAX`.
                unsafe { transmute_neo::<$int, Self>(integer) }
            }

            /// The value in the range nearest `integer`: `MIN` below it, `MAX` above it.
            #[expect(unsafe_code, reason = "builds the value once it is moved into the range")]
            #[inline]
            #[must_use]
            pub const fn new_saturating(integer: $int) -> Self {
                let nearest = if integer < MIN {
                    MIN
                } else if integer > MAX {
                    MAX
                } else {
                    integer
                };
                // SAFETY: `nearest` lies from `MIN` to `MAX`, since `MIN` is at most `MAX` wherever
                // `Self` exists: rustc refuses a pattern type whose range wraps.
                unsafe { Self::new_unchecked(nearest) }
            }

            /// The integer, from `MIN` to `MAX`.
            #[expect(unsafe_code, reason = "a pattern type is read only by a transmute")]
            #[inline]
            #[must_use]
            pub const fn get(self) -> $int {
                // SAFETY: `Self` is a transparent pattern type over the integer, so of its size,
                // and each of its values is one of the integer's.
                unsafe { transmute_neo::<Self, $int>(self) }
            }
        }
    };

    // Between ranges of the integer, and to and from its `NonZero` where no value is lost: each a
    // method, since no trait bound states a range's bounds without `generic_const_exprs`, and each
    // but `to_range` refuses the build where the ranges do not fit.
    (@convert_within $name:ident $int:ident) => {
        impl<const MIN: $int, const MAX: $int> $name<MIN, MAX> {
            /// The value in `NEW_MIN..=NEW_MAX`, a range that holds `MIN..=MAX`.
            ///
            /// A range that does not hold it fails `cargo build`, and `cargo check` only where a
            /// constant makes the call; [`narrow`](Self::narrow) and [`to_range`](Self::to_range)
            /// check the value instead.
            /// None of the three is a `From` or a `TryFrom`: no bound states that one range holds
            /// another without `generic_const_exprs`, and a conversion between two ranges of one
            /// integer would overlap core's `From<T> for T` where they match.
            ///
            /// # Examples
            /// ```
            /// # extern crate atomix_core as atomix;
            #[doc = concat!("use atomix::", stringify!($name), ";")]
            ///
            /// // How many levels of an order book a feed sends, and how many a book keeps.
            #[doc = concat!("let depth = ", stringify!($name), "::<1, 10>::new(4).expect(\"4 is from 1 to 10\");")]
            #[doc = concat!("let kept: ", stringify!($name), "<0, 50> = depth.expand();")]
            /// assert_eq!(kept.get(), 4, "the same levels, in the wider range");
            /// assert_eq!(kept.narrow::<1, 3>(), None, "and none in a range that misses them");
            /// ```
            #[expect(unsafe_code, reason = "builds the value in a range the build checks holds it")]
            #[inline]
            #[must_use]
            pub const fn expand<const NEW_MIN: $int, const NEW_MAX: $int>(
                self,
            ) -> $name<NEW_MIN, NEW_MAX> {
                const {
                    assert!(
                        NEW_MIN <= MIN && MAX <= NEW_MAX,
                        "a ranged integer expands only to a range that holds its own: call \
                         `to_range`, which checks the value, or `narrow` for a range inside it"
                    );
                }
                // SAFETY: the value lies from `MIN` to `MAX`, which `NEW_MIN..=NEW_MAX` holds, as
                // the assertion above checks.
                unsafe { $name::new_unchecked(self.get()) }
            }

            /// The value in `NEW_MIN..=NEW_MAX`, a range inside `MIN..=MAX`, or `None` where it
            /// lies outside that range.
            ///
            /// A range not inside fails `cargo build`, and `cargo check` only where a constant
            /// makes the call, as deranged's `narrow` does. [`expand`](Self::expand) takes a range
            /// that holds `MIN..=MAX`, and [`to_range`](Self::to_range) any range; `expand`'s
            /// example shows `narrow` too.
            #[inline]
            #[must_use]
            pub const fn narrow<const NEW_MIN: $int, const NEW_MAX: $int>(
                self,
            ) -> Option<$name<NEW_MIN, NEW_MAX>> {
                const {
                    assert!(
                        MIN <= NEW_MIN && NEW_MAX <= MAX,
                        "a ranged integer narrows only to a range inside its own: call `expand` \
                         for one that holds it, or `to_range`, which checks the value, for any \
                         other"
                    );
                }
                $name::new(self.get())
            }

            /// The value in `NEW_MIN..=NEW_MAX`, any range of the integer, or `None` where it lies
            /// outside that range.
            ///
            /// It takes a range that only overlaps `MIN..=MAX`, which [`expand`](Self::expand) and
            /// [`narrow`](Self::narrow) each refuse. Where the new range holds `MIN..=MAX` or lies
            /// inside it, call `expand` or `narrow`, and the build checks that it does.
            ///
            /// # Examples
            /// ```
            /// # extern crate atomix_core as atomix;
            #[doc = concat!("use atomix::", stringify!($name), ";")]
            ///
            /// // The levels of an order book a feed sends, and the deeper levels a book keeps.
            #[doc = concat!("let depth = ", stringify!($name), "::<1, 10>::new(7).expect(\"7 is from 1 to 10\");")]
            #[doc = concat!("let kept: Option<", stringify!($name), "<5, 20>> = depth.to_range();")]
            /// assert_eq!(kept.map(|levels| levels.get()), Some(7), "7 is from 5 to 20 too");
            #[doc = concat!("let shallow = ", stringify!($name), "::<1, 10>::MIN;")]
            /// assert_eq!(shallow.to_range::<5, 20>(), None, "and 1 is not");
            /// ```
            #[inline]
            #[must_use]
            pub const fn to_range<const NEW_MIN: $int, const NEW_MAX: $int>(
                self,
            ) -> Option<$name<NEW_MIN, NEW_MAX>> {
                $name::new(self.get())
            }

            /// The value as a `NonZero`, where the range holds no zero; any other range fails
            /// `cargo build`, and `cargo check` only where a constant makes the call, and converts
            /// with `NonZero::try_from`.
            ///
            /// No `From` says so, since no bound states that a range misses zero without
            /// `generic_const_exprs`.
            #[expect(unsafe_code, reason = "builds a nonzero once the build checks the range misses zero")]
            #[inline]
            #[must_use]
            pub const fn to_non_zero(self) -> NonZero<$int> {
                const {
                    assert!(
                        Self::new(0).is_none(),
                        "only a range that holds no zero converts to `NonZero` unchecked: call \
                         `NonZero::try_from`"
                    );
                }
                // SAFETY: the value lies in the range, which holds no zero, as the assertion above
                // checks.
                unsafe { NonZero::new_unchecked(self.get()) }
            }

            /// The value of `non_zero`, where the range holds every nonzero integer; any other
            /// range fails `cargo build`, and `cargo check` only where a constant makes the call,
            /// and converts with `try_from`.
            ///
            /// No `From` says so, as for [`to_non_zero`](Self::to_non_zero).
            #[expect(unsafe_code, reason = "builds the value once the build checks the range holds it")]
            #[inline]
            #[must_use]
            pub const fn from_non_zero(non_zero: NonZero<$int>) -> Self {
                const {
                    assert!(
                        Self::new(NonZero::<$int>::MIN.get()).is_some()
                            && Self::new(NonZero::<$int>::MAX.get()).is_some(),
                        "only a range that holds every nonzero integer converts from `NonZero` \
                         unchecked: call `try_from`"
                    );
                }
                // SAFETY: the range holds the smallest and the largest nonzero integer, as the
                // assertion above checks, so each one between, `non_zero` among them.
                unsafe { Self::new_unchecked(non_zero.get()) }
            }
        }
    };

    // Arithmetic, which can leave the range, so checked or saturating.
    (@arithmetic $name:ident $int:ident) => {
        impl<const MIN: $int, const MAX: $int> $name<MIN, MAX> {
            /// Adds `rhs`, or `None` where the sum lies outside the range.
            #[inline]
            #[must_use]
            pub const fn checked_add(self, rhs: $int) -> Option<Self> {
                self.get().checked_add(rhs).and_then(Self::new)
            }

            /// Subtracts `rhs`, or `None` where the difference lies outside the range.
            #[inline]
            #[must_use]
            pub const fn checked_sub(self, rhs: $int) -> Option<Self> {
                self.get().checked_sub(rhs).and_then(Self::new)
            }

            /// Multiplies by `rhs`, or `None` where the product lies outside the range.
            #[inline]
            #[must_use]
            pub const fn checked_mul(self, rhs: $int) -> Option<Self> {
                self.get().checked_mul(rhs).and_then(Self::new)
            }

            /// Adds `rhs`, saturating at `MIN` and `MAX`.
            #[inline]
            #[must_use]
            pub const fn saturating_add(self, rhs: $int) -> Self {
                Self::new_saturating(self.get().saturating_add(rhs))
            }

            /// Subtracts `rhs`, saturating at `MIN` and `MAX`.
            #[inline]
            #[must_use]
            pub const fn saturating_sub(self, rhs: $int) -> Self {
                Self::new_saturating(self.get().saturating_sub(rhs))
            }

            /// Multiplies by `rhs`, saturating at `MIN` and `MAX`.
            #[inline]
            #[must_use]
            pub const fn saturating_mul(self, rhs: $int) -> Self {
                Self::new_saturating(self.get().saturating_mul(rhs))
            }
        }
    };

    // Equality, order and hashing, the integer's, within one range.
    (@compare $name:ident $int:ident) => {
        const impl<const MIN: $int, const MAX: $int> PartialEq for $name<MIN, MAX> {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.get() == other.get()
            }
        }

        const impl<const MIN: $int, const MAX: $int> Eq for $name<MIN, MAX> {}

        // A constant is a pattern only where its `PartialEq` is structural, as a derived one is.
        impl<const MIN: $int, const MAX: $int> StructuralPartialEq for $name<MIN, MAX> {}

        const impl<const MIN: $int, const MAX: $int> PartialOrd for $name<MIN, MAX> {
            #[inline]
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        const impl<const MIN: $int, const MAX: $int> Ord for $name<MIN, MAX> {
            #[inline]
            fn cmp(&self, other: &Self) -> Ordering {
                self.get().cmp(&other.get())
            }
        }

        impl<const MIN: $int, const MAX: $int> Hash for $name<MIN, MAX> {
            #[inline]
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.get().hash(state);
            }
        }
    };

    // Each formatting trait, the integer's.
    (@format $name:ident $int:ident: $($format:ident),+) => {$(
        impl<const MIN: $int, const MAX: $int> $format for $name<MIN, MAX> {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                $format::fmt(&self.get(), formatter)
            }
        }
    )+};

    // To the integer, and from it or from text, checked against the range.
    (@convert $name:ident $int:ident) => {
        const impl<const MIN: $int, const MAX: $int> From<$name<MIN, MAX>> for $int {
            #[inline]
            fn from(value: $name<MIN, MAX>) -> Self {
                value.get()
            }
        }

        const impl<const MIN: $int, const MAX: $int> TryFrom<$int> for $name<MIN, MAX> {
            type Error = RangeError<$int>;

            #[inline]
            fn try_from(integer: $int) -> Result<Self, Self::Error> {
                Self::new(integer).ok_or(RangeError::new(integer, MIN, MAX))
            }
        }

        const impl<const MIN: $int, const MAX: $int> TryFrom<NonZero<$int>> for $name<MIN, MAX> {
            type Error = RangeError<$int>;

            #[inline]
            fn try_from(non_zero: NonZero<$int>) -> Result<Self, Self::Error> {
                Self::try_from(non_zero.get())
            }
        }

        const impl<const MIN: $int, const MAX: $int> TryFrom<$name<MIN, MAX>> for NonZero<$int> {
            type Error = TryFromIntError;

            #[inline]
            fn try_from(value: $name<MIN, MAX>) -> Result<Self, Self::Error> {
                Self::try_from(value.get())
            }
        }

        const impl<const MIN: $int, const MAX: $int> FromStr for $name<MIN, MAX> {
            type Err = ParseRangeError<$int>;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                match $int::from_str(text) {
                    Ok(integer) => match Self::try_from(integer) {
                        Ok(value) => Ok(value),
                        Err(refused) => Err(ParseRangeError::Range(refused)),
                    },
                    Err(refused) => Err(ParseRangeError::Integer(refused)),
                }
            }
        }
    };

    // Steps through the range, so `MIN..=MAX` iterates; a step past either end saturates there.
    (@step $name:ident $int:ident) => {
        const impl<const MIN: $int, const MAX: $int> Step for $name<MIN, MAX> {
            #[inline]
            fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
                Step::steps_between(&start.get(), &end.get())
            }

            #[inline]
            fn forward_checked(start: Self, count: usize) -> Option<Self> {
                Step::forward_checked(start.get(), count).and_then(Self::new)
            }

            #[inline]
            fn backward_checked(start: Self, count: usize) -> Option<Self> {
                Step::backward_checked(start.get(), count).and_then(Self::new)
            }

            #[inline]
            fn forward_overflowing(start: Self, count: usize) -> (Self, bool) {
                match Self::forward_checked(start, count) {
                    Some(end) => (end, false),
                    None => (Self::MAX, true),
                }
            }

            #[inline]
            fn backward_overflowing(start: Self, count: usize) -> (Self, bool) {
                match Self::backward_checked(start, count) {
                    Some(end) => (end, false),
                    None => (Self::MIN, true),
                }
            }

            #[inline]
            fn forward(start: Self, count: usize) -> Self {
                Self::forward_overflowing(start, count).0
            }

            #[inline]
            fn backward(start: Self, count: usize) -> Self {
                Self::backward_overflowing(start, count).0
            }
        }
    };
}

/// Converts the ranged integer `$source` of `$source_integer` to `$target` of `$target_integer`,
/// each to the other's `NonZero` too: `TryFrom`, checked, as core converts integers of two types,
/// since whether a value is lost depends on the ranges. A refused value comes back as it was,
/// beside the target's range.
macro_rules! convert_between {
    ($source:ident($source_integer:ident) => $target:ident($target_integer:ident)) => {
        const impl<
            const SOURCE_MIN: $source_integer,
            const SOURCE_MAX: $source_integer,
            const MIN: $target_integer,
            const MAX: $target_integer,
        > TryFrom<$source<SOURCE_MIN, SOURCE_MAX>> for $target<MIN, MAX>
        {
            type Error = RangeError<$source_integer, $target_integer>;

            #[inline]
            fn try_from(value: $source<SOURCE_MIN, SOURCE_MAX>) -> Result<Self, Self::Error> {
                let integer = value.get();
                // An integer no `$target_integer` holds lies outside every range of it.
                if let Ok(converted) = <$target_integer>::try_from(integer)
                    && let Some(value) = Self::new(converted)
                {
                    Ok(value)
                } else {
                    Err(RangeError::new(integer, MIN, MAX))
                }
            }
        }

        const impl<const MIN: $target_integer, const MAX: $target_integer>
            TryFrom<NonZero<$source_integer>> for $target<MIN, MAX>
        {
            type Error = RangeError<$source_integer, $target_integer>;

            #[inline]
            fn try_from(non_zero: NonZero<$source_integer>) -> Result<Self, Self::Error> {
                let integer = non_zero.get();
                if let Ok(converted) = <$target_integer>::try_from(integer)
                    && let Some(value) = Self::new(converted)
                {
                    Ok(value)
                } else {
                    Err(RangeError::new(integer, MIN, MAX))
                }
            }
        }

        const impl<const MIN: $source_integer, const MAX: $source_integer>
            TryFrom<$source<MIN, MAX>> for NonZero<$target_integer>
        {
            type Error = TryFromIntError;

            #[inline]
            fn try_from(value: $source<MIN, MAX>) -> Result<Self, Self::Error> {
                match NonZero::try_from(value.get()) {
                    Ok(non_zero) => {
                        with_try_from_int_error(NonZero::<$target_integer>::try_from(non_zero))
                    },
                    Err(refused) => Err(refused),
                }
            }
        }
    };
}

/// `result`, its refusal as core's `TryFromIntError`: a conversion of `NonZero`s that cannot fail,
/// whose error is `!`, and one that can read alike.
#[inline]
const fn with_try_from_int_error<T, E: [const] Into<TryFromIntError>>(
    result: Result<T, E>,
) -> Result<T, TryFromIntError> {
    match result {
        Ok(value) => Ok(value),
        Err(refused) => Err(refused.into()),
    }
}

each_ranged_integer!(ranged);
each_ranged_integer!(pairs convert_between);

#[cfg(test)]
mod tests;
