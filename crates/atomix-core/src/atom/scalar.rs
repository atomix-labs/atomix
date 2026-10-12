//! `Atom` for core's value types.

use core::ascii::Char as AsciiChar;
use core::cmp::{Ordering, Reverse};
use core::convert::Infallible;
use core::fmt::{Alignment as FormatAlignment, Error as FormatError};
use core::marker::{PhantomData, PhantomPinned};
use core::mem::{Alignment, ManuallyDrop, transmute};
use core::num::{FpCategory, NonZero, Saturating, Wrapping};
use core::ops::RangeFull;

use super::{Atom, AtomAdd, AtomBitwise, AtomOrd};
use crate::range::{PointeeAlignment, ReprRange, Tags};
use crate::validity::{Total, TotalZeroNiche, ZeroNiche, ZeroValid};

/// Implements `Atom` for integers, and the capabilities listed.
macro_rules! integers {
    (@one [$($capability:ident),*] $int:ty) => {
        // SAFETY: an integer is its own repr and every repr an integer, so its range is every
        // repr, and an integer may cross threads.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl Atom for $int {
            type Repr = Self;
            type Validity = Total;
            const REPRS: ReprRange<Self> = ReprRange::FULL;
            #[inline]
            fn to_repr(self) -> Self {
                self
            }
            #[inline]
            fn from_repr(repr: Self) -> Option<Self> {
                Some(repr)
            }
            #[inline]
            unsafe fn from_repr_unchecked(repr: Self) -> Self {
                repr
            }
        }
        $(impl $capability for $int {})*
    };
    ($capabilities:tt $($int:ty),+ $(,)?) => {$(
        integers!(@one $capabilities $int);
    )+};
}

integers! {
    [AtomAdd, AtomOrd, AtomBitwise]
    u8, u16, u32, u64, usize, i8, i16, i32, i64, isize,
}

// SAFETY: a `bool` is its own repr and every repr a `bool`, so its range is every repr, and a
// `bool` may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for bool {
    type Repr = Self;
    type Validity = Total;
    const REPRS: ReprRange<Self> = ReprRange::FULL;
    #[inline]
    fn to_repr(self) -> Self {
        self
    }
    #[inline]
    fn from_repr(repr: Self) -> Option<Self> {
        Some(repr)
    }
    #[inline]
    unsafe fn from_repr_unchecked(repr: Self) -> Self {
        repr
    }
}

impl AtomBitwise for bool {}

// The capabilities above, checked at compile time.
const _: () = {
    /// Compiles only for a value with every capability.
    const fn has_every_capability<T: AtomAdd + AtomOrd + AtomBitwise>() {}
    has_every_capability::<u8>();
    has_every_capability::<u16>();
    has_every_capability::<u32>();
    has_every_capability::<u64>();
    has_every_capability::<usize>();
    has_every_capability::<i8>();
    has_every_capability::<i16>();
    has_every_capability::<i32>();
    has_every_capability::<i64>();
    has_every_capability::<isize>();
};

/// Implements `Atom` and `AtomOrd` for the nonzero integers.
macro_rules! non_zero {
    ($($int:ty),+ $(,)?) => {$(
        // SAFETY: the repr is the integer, never zero, so within every repr but zero; `new`
        // decodes exactly the nonzero ones; a nonzero integer may cross threads.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl Atom for NonZero<$int> {
            type Repr = $int;
            type Validity = TotalZeroNiche;
            const REPRS: ReprRange<$int> = ReprRange::NON_ZERO;
            #[inline]
            fn to_repr(self) -> $int {
                self.get()
            }
            #[inline]
            fn from_repr(repr: $int) -> Option<Self> {
                Self::new(repr)
            }
            #[inline]
            unsafe fn from_repr_unchecked(repr: $int) -> Self {
                // SAFETY: the caller's repr decodes, so it is not zero.
                unsafe { Self::new_unchecked(repr) }
            }
        }
        impl AtomOrd for NonZero<$int> {}
    )+};
}

non_zero!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

// SAFETY: the repr is the scalar value, from 0 to `char::MAX`; `from_u32` decodes exactly the
// scalar values, refusing the surrogates; a `char` may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for char {
    type Repr = u32;
    type Validity = ZeroValid;
    const REPRS: ReprRange<u32> = ReprRange::new(0, 0x10_FFFF);
    #[inline]
    fn to_repr(self) -> u32 {
        u32::from(self)
    }
    #[inline]
    fn from_repr(repr: u32) -> Option<Self> {
        Self::from_u32(repr)
    }
    #[inline]
    unsafe fn from_repr_unchecked(repr: u32) -> Self {
        // SAFETY: the caller's repr decodes, so it is a scalar value.
        unsafe { Self::from_u32_unchecked(repr) }
    }
}

impl AtomOrd for char {}

// SAFETY: the repr is the scalar value, from 1 to `char::MAX`, never zero; `from_u32` and `new`
// decode exactly the nonzero scalar values, refusing zero and the surrogates, and their unchecked
// twins decode each of those alike; a `char` may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for NonZero<char> {
    type Repr = u32;
    type Validity = ZeroNiche;
    const REPRS: ReprRange<u32> = ReprRange::new(1, 0x10_FFFF);
    #[inline]
    fn to_repr(self) -> u32 {
        u32::from(self.get())
    }
    #[inline]
    fn from_repr(repr: u32) -> Option<Self> {
        match char::from_u32(repr) {
            Some(scalar) => Self::new(scalar),
            None => None,
        }
    }
    #[inline]
    unsafe fn from_repr_unchecked(repr: u32) -> Self {
        // SAFETY: the caller's repr decodes, so it is a scalar value.
        let scalar = unsafe { char::from_u32_unchecked(repr) };
        // SAFETY: the caller's repr decodes, so it is not zero.
        unsafe { Self::new_unchecked(scalar) }
    }
}

impl AtomOrd for NonZero<char> {}

// SAFETY: the repr is the ordering's discriminant, which its `#[repr(i8)]` documents as -1, 0 and
// 1, a range through zero; `from_repr` decodes exactly those, by the repr alone, each as its
// ordering, as the unchecked transmute does; an ordering holds no data, so it may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for Ordering {
    type Repr = i8;
    type Validity = ZeroValid;
    const REPRS: ReprRange<i8> = ReprRange::from_signed(-1, 1);
    #[inline]
    fn to_repr(self) -> i8 {
        match self {
            Self::Less => -1,
            Self::Equal => 0,
            Self::Greater => 1,
        }
    }
    #[inline]
    fn from_repr(repr: i8) -> Option<Self> {
        match repr {
            -1 => Some(Self::Less),
            0 => Some(Self::Equal),
            1 => Some(Self::Greater),
            _ => None,
        }
    }
    // The default, `from_repr`'s unwrap, leaves a range check and a select after `fetch_max`'s
    // `ldsmaxalb`, which `cmp_ordering_fetch_max`'s pin refuses.
    #[inline]
    unsafe fn from_repr_unchecked(repr: i8) -> Self {
        // SAFETY: the caller's repr decodes, so it is -1, 0 or 1, a discriminant of `Ordering`'s
        // documented `#[repr(i8)]`.
        unsafe { transmute::<i8, Self>(repr) }
    }
}

/// `Less`, `Equal` and `Greater` order as their discriminants, -1, 0 and 1.
impl AtomOrd for Ordering {}

/// Implements `Atom` for floats, as their bits.
macro_rules! floats {
    ($($float:ty: $bits:ty),+ $(,)?) => {$(
        // SAFETY: the repr is the float's bits, which span the repr; every bit pattern is a
        // float; a float may cross threads.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl Atom for $float {
            type Repr = $bits;
            type Validity = Total;
            const REPRS: ReprRange<$bits> = ReprRange::FULL;
            #[inline]
            fn to_repr(self) -> $bits {
                self.to_bits()
            }
            #[inline]
            fn from_repr(repr: $bits) -> Option<Self> {
                Some(<$float>::from_bits(repr))
            }
            #[inline]
            unsafe fn from_repr_unchecked(repr: $bits) -> Self {
                <$float>::from_bits(repr)
            }
        }
    )+};
}

floats!(f16: u16, f32: u32, f64: u64);
#[cfg(wide)]
floats!(f128: u128);

/// Implements `Atom` for zero-width values: one value, whose repr is zero.
macro_rules! zero_width {
    ($([$($param:tt)*] $kind:ty => $value:expr);+ $(;)?) => {$(
        // SAFETY: the one value is repr zero, and only zero decodes; it holds no data, so it may
        // cross threads.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl<$($param)*> Atom for $kind {
            type Repr = u8;
            type Validity = ZeroValid;
            const REPRS: ReprRange<u8> = ReprRange::new(0, 0);
            #[inline]
            fn to_repr(self) -> u8 {
                0
            }
            #[inline]
            fn from_repr(repr: u8) -> Option<Self> {
                if repr == 0 { Some($value) } else { None }
            }
            #[inline]
            unsafe fn from_repr_unchecked(_: u8) -> Self {
                $value
            }
        }
    )+};
}

zero_width! {
    [] () => ();
    [T: ?Sized] PhantomData<T> => PhantomData;
    [] PhantomPinned => PhantomPinned;
    [] RangeFull => RangeFull;
    [] FormatError => FormatError;
}

// SAFETY: no value exists, so `to_repr` returns no repr, and `from_repr` decodes none, which
// `Partial` allows; no value crosses threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for Infallible {
    type Repr = u8;
    const REPRS: ReprRange<u8> = ReprRange::new(0, 0);
    #[inline]
    fn to_repr(self) -> u8 {
        match self {}
    }
    #[inline]
    fn from_repr(_: u8) -> Option<Self> {
        None
    }
}

/// The last of `indices`, a fieldless enum's: the greatest, as they count up from zero.
const fn last_index(indices: &[u8]) -> u128 {
    match indices {
        [.., last] => u128::from(*last),
        [] => 0,
    }
}

/// Implements `Atom` for a fieldless enum of core's, each variant its index, counting up from zero.
macro_rules! fieldless {
    ($($kind:ty { $($variant:ident = $index:literal),+ })+) => {$(
        // SAFETY: the repr is the variant's index, from zero to the last's; `from_repr` decodes
        // exactly those, by the repr alone, each as its variant, zero among them; a variant holds
        // no data, so it may cross threads.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl Atom for $kind {
            type Repr = u8;
            type Validity = ZeroValid;
            const REPRS: ReprRange<u8> = ReprRange::new(0, last_index(&[$($index),+]));
            #[inline]
            fn to_repr(self) -> u8 {
                match self {
                    $(Self::$variant => $index,)+
                }
            }
            #[inline]
            fn from_repr(repr: u8) -> Option<Self> {
                match repr {
                    $($index => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    )+};
}

fieldless! {
    FpCategory { Nan = 0, Infinite = 1, Zero = 2, Subnormal = 3, Normal = 4 }
    FormatAlignment { Left = 0, Right = 1, Center = 2 }
}

// SAFETY: the repr is the alignment's base-2 logarithm, below a word's bits; `from_repr` decodes
// exactly those, by the repr alone, zero among them, as an alignment of one; an alignment holds
// no pointer, so it may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for Alignment {
    type Repr = u8;
    type Validity = ZeroValid;
    const REPRS: ReprRange<u8> = ReprRange::new(0, u128::from(usize::BITS - 1));
    #[inline]
    fn to_repr(self) -> u8 {
        self.log2().wrapping_cast()
    }
    #[inline]
    fn from_repr(repr: u8) -> Option<Self> {
        match 1_usize.checked_shl(u32::from(repr)) {
            Some(alignment) => Self::new(alignment),
            None => None,
        }
    }
}

/// Alignments order as their logarithms do.
impl AtomOrd for Alignment {}

// SAFETY: the repr is the character's code, below 128; `from_u8` decodes exactly those, by the
// repr alone, zero among them, as `from_u8_unchecked` decodes them unchecked; a character holds no
// pointer, so it may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for AsciiChar {
    type Repr = u8;
    type Validity = ZeroValid;
    const REPRS: ReprRange<u8> = ReprRange::new(0, 0x7F);
    #[inline]
    fn to_repr(self) -> u8 {
        self.to_u8()
    }
    #[inline]
    fn from_repr(repr: u8) -> Option<Self> {
        Self::from_u8(repr)
    }
    #[inline]
    unsafe fn from_repr_unchecked(repr: u8) -> Self {
        // SAFETY: the caller's repr decodes, so it lies below 128.
        unsafe { Self::from_u8_unchecked(repr) }
    }
}

/// Characters order as their codes do.
impl AtomOrd for AsciiChar {}

/// Implements `Atom` for a wrapper stored as the value it wraps, and the capabilities it keeps.
/// `$get` reads the value from `$outer`, the wrapper, and `$new` wraps `$value` again.
macro_rules! wrappers {
    ($(
        $wrapper:ident($outer:ident => $get:expr, $value:ident => $new:expr):
            [$($capability:ident),*];
    )+) => {$(
        // SAFETY: the repr, its range, its tags, the validity and the threads rule are the wrapped
        // value's.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl<T: [const] Atom> Atom for $wrapper<T> {
            type Repr = T::Repr;
            type Validity = T::Validity;
            const REPRS: ReprRange<T::Repr> = T::REPRS;
            const TAG_WIDTH: u32 = T::TAG_WIDTH;
            const POINTEE_ALIGNMENT: PointeeAlignment = T::POINTEE_ALIGNMENT;
            #[inline]
            fn to_repr(self) -> T::Repr {
                let $outer = self;
                $get.to_repr()
            }
            #[inline]
            fn to_tagged_repr(self, tags: Tags) -> (T::Repr, usize) {
                let $outer = self;
                $get.to_tagged_repr(tags)
            }
            #[inline]
            fn from_repr(repr: T::Repr) -> Option<Self> {
                match T::from_repr(repr) {
                    Some($value) => Some($new),
                    None => None,
                }
            }
            #[inline]
            unsafe fn from_repr_unchecked(repr: T::Repr) -> Self {
                // SAFETY: the caller's repr decodes as the wrapper, so as the wrapped value.
                let $value = unsafe { T::from_repr_unchecked(repr) };
                $new
            }
        }
        $(impl<T: $capability> $capability for $wrapper<T> {})*
    )+};
}

pub(super) use wrappers;

// `Saturating` keeps no `AtomAdd`: the repr's add wraps, the value's saturates. `Reverse` keeps no
// `AtomOrd`, as its order is the reverse of the repr's. `ManuallyDrop` keeps every one: it changes
// what a drop does, which a `Copy` value has none of.
wrappers! {
    Wrapping(wrapper => wrapper.0, value => Wrapping(value)): [AtomAdd, AtomOrd, AtomBitwise];
    Saturating(wrapper => wrapper.0, value => Saturating(value)): [AtomOrd, AtomBitwise];
    Reverse(wrapper => wrapper.0, value => Reverse(value)): [AtomBitwise];
    ManuallyDrop(wrapper => ManuallyDrop::into_inner(wrapper), value => ManuallyDrop::new(value)):
        [AtomAdd, AtomOrd, AtomBitwise];
}

// The 128-bit integers and their `NonZero`s, where a 16-byte compare-exchange exists. atomix
// has no 128-bit add, bitwise operation, max or min, so they take no `AtomAdd` or `AtomBitwise`,
// and `AtomOrd` brings them no `fetch_max` or `fetch_min`: `update` is each one's loop.
#[cfg(wide)]
integers! { [AtomOrd] u128, i128 }
#[cfg(wide)]
non_zero!(u128, i128);
