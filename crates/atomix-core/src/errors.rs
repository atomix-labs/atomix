//! Why a ranged integer refused an integer or a text, one type for each question:
//!
//! | Type                | Asks                                 | Returned By |
//! | ------------------- | ------------------------------------ | ----------- |
//! | [`RangeError`]      | is the integer in the range?         | `try_from`  |
//! | [`ParseRangeError`] | is the text an integer in the range? | `from_str`  |
//!
//! A `RangeError` from another ranged integer, or from a `NonZero` of another integer, holds that
//! integer, of its own type, beside the range, of the range's: neither type need hold the other.
//!
//! `Display` and `Error` are written by hand: atomix-core depends on no crate outside loom's
//! model, so no derive writes them.

use core::any::type_name;
use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter};
use core::num::ParseIntError;
use core::ops::RangeInclusive;

/// Why an integer `I` is no ranged integer of `B`: it lies outside the range.
///
/// `I` is `B` where the integer was the range's own type, as `try_from` of one is.
#[derive(Debug, Copy, Hash)]
#[derive_const(Clone, PartialEq, Eq)]
pub struct RangeError<I, B = I> {
    /// The integer refused.
    integer: I,
    /// The range's smallest integer, `MIN`.
    min: B,
    /// The range's largest integer, `MAX`.
    max: B,
}

impl<I: Copy, B: Copy> RangeError<I, B> {
    /// The refusal of `integer`, which lies outside `min..=max`.
    pub(crate) const fn new(integer: I, min: B, max: B) -> Self {
        Self { integer, min, max }
    }

    /// The integer refused.
    #[inline]
    #[must_use]
    pub const fn integer(self) -> I {
        self.integer
    }

    /// The range the integer lies outside, `MIN..=MAX`.
    #[inline]
    #[must_use]
    pub const fn bounds(self) -> RangeInclusive<B> {
        RangeInclusive::new(self.min, self.max)
    }
}

impl<I: Display, B: Display> Display for RangeError<I, B> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let Self { integer, min, max } = self;
        write!(formatter, "range error: {integer} is not in {min}..={max}")
    }
}

impl<I: Debug + Display, B: Debug + Display> Error for RangeError<I, B> {}

/// Why a text is no ranged integer: it is no `T`, or a `T` outside the range.
///
/// An enum, so a match takes the cause whole: core's [`ParseIntError`], or the [`RangeError`] with
/// the integer refused. [`source`](Error::source) returns it too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseRangeError<T> {
    /// The text is no `T`: it is empty, holds a character that is no digit, or names an integer
    /// past `T`'s own bounds.
    Integer(ParseIntError),
    /// The text is a `T` outside the range.
    Range(RangeError<T>),
}

impl<T> Display for ParseRangeError<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(_) => {
                write!(formatter, "parse range error: the text is no `{}`", type_name::<T>())
            },
            Self::Range(_) => {
                formatter.write_str("parse range error: the integer is outside the range")
            },
        }
    }
}

impl<T: Debug + Display + 'static> Error for ParseRangeError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Integer(cause) => Some(cause),
            Self::Range(cause) => Some(cause),
        }
    }
}
