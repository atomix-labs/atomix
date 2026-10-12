//! `Atom` for core's enums of payloads stored as bits: `Result`, `ControlFlow` and `Bound`.
//!
//! Each keeps its payload's repr from bit 0, in a slot as wide as the wider payload's repr, and its
//! variant's index, its tag, above it: `Result<u32, u8>` is a `u64`, `Ok` holding tag 0, so a repr
//! of zero is `Ok` of the payload's zero.
//!
//! A generic impl chooses its repr from types alone, so a hidden table gives the wider of two
//! payloads' kinds, and the integer that holds it beside a tag. A payload stored as a pointer has
//! no entry: an enum of the same variants derives a pointer enum, tagged in the pointer's low bits.

use core::ops::{Bound, ControlFlow};

use super::Atom;
use super::tuple::{Integer, TupleFieldRepr};
use crate::primitive::{CompareExchange, ExactBits, Primitive};
use crate::range::{ReprRange, mask};
use crate::validity::Validity;

/// The wider of a payload kind of this width and `K`.
#[doc(hidden)]
pub impl(crate) trait Wider<K> {
    /// The wider kind.
    type Kind;
}

/// Implements `Wider` for each pair of widths, each row a width and the pairs it makes.
macro_rules! wider_kinds {
    ($($first:literal: $($second:literal => $wider:literal),+;)+) => {$($(
        impl Wider<Integer<$second>> for Integer<$first> {
            type Kind = Integer<$wider>;
        }
    )+)+};
}

wider_kinds! {
    1: 1 => 1, 8 => 8, 16 => 16, 32 => 32, 64 => 64, 128 => 128;
    8: 1 => 8, 8 => 8, 16 => 16, 32 => 32, 64 => 64, 128 => 128;
    16: 1 => 16, 8 => 16, 16 => 16, 32 => 32, 64 => 64, 128 => 128;
    32: 1 => 32, 8 => 32, 16 => 32, 32 => 32, 64 => 64, 128 => 128;
    64: 1 => 64, 8 => 64, 16 => 64, 32 => 64, 64 => 64, 128 => 128;
    128: 1 => 128, 8 => 128, 16 => 128, 32 => 128, 64 => 128, 128 => 128;
}

/// The integer that holds a payload of this kind below a tag of `TAG_BITS` bits: the narrowest
/// with room for both, where one exists.
#[doc(hidden)]
pub impl(crate) const trait EnumShape<const TAG_BITS: u32> {
    /// The integer.
    type Repr: const Primitive + CompareExchange + const ExactBits;
    /// The payload's slot: the bits below the tag.
    const WIDTH: u32;
}

/// Implements `EnumShape` for each payload width and its integer, beside a tag of one bit or two.
macro_rules! enum_shapes {
    ($($width:literal => $repr:ty;)+) => {$(
        const impl EnumShape<1> for Integer<$width> {
            type Repr = $repr;
            const WIDTH: u32 = $width;
        }
        const impl EnumShape<2> for Integer<$width> {
            type Repr = $repr;
            const WIDTH: u32 = $width;
        }
    )+};
}

enum_shapes! {
    1 => u8;
    8 => u16;
    16 => u32;
    32 => u64;
}
#[cfg(wide)]
enum_shapes! {
    64 => u128;
}

/// The repr of variant `tag`, its payload's `bits` below the tag at `width`.
#[inline]
const fn join_variant<R: [const] ExactBits>(tag: u128, width: u32, bits: u128) -> R {
    R::from_bits(tag << width | bits)
}

/// The tag and the payload's bits of `repr`, whose payload's slot is `width` bits.
#[inline]
const fn split_variant<R: [const] ExactBits>(repr: R, width: u32) -> (u128, u128) {
    let bits = repr.to_bits();
    (bits >> width, bits & mask(width))
}

/// The `T` whose repr's bits are `bits`, or `None` where a bit above its repr is set, or where they
/// decode as no `T`.
#[inline]
const fn payload<T: [const] Atom<Repr: [const] ExactBits>>(bits: u128) -> Option<T> {
    if bits >> <T::Repr as Primitive>::BITS == 0 {
        T::from_repr(<T::Repr as Primitive>::from_bits(bits))
    } else {
        None
    }
}

/// The `T` whose repr's bits are `bits`, unchecked.
///
/// # Safety
/// `bits` are a repr of `T` that decodes.
#[expect(unsafe_code, reason = "decodes a payload the caller's repr holds")]
#[inline]
const unsafe fn payload_unchecked<T: [const] Atom<Repr: [const] ExactBits>>(bits: u128) -> T {
    // SAFETY: the caller's bits are a repr of `T` that decodes.
    unsafe { T::from_repr_unchecked(<T::Repr as Primitive>::from_bits(bits)) }
}

/// Implements `Atom` for an enum of two variants, each holding a payload, tagged 0 and 1 in order.
macro_rules! two_variants {
    ($(
        $enumeration:ident<$($param:ident),+>:
            $first:ident $first_variant:ident, $second:ident $second_variant:ident;
    )+) => {$(
        // SAFETY: the repr is the variant's payload's repr from bit 0, its bits above clear, and
        // the tag above the slot of the wider payload's repr, the bits above it clear, which the
        // table gives an integer of; `REPRS` runs from zero to the second variant's last repr;
        // `from_repr` decodes exactly a tag of 0 or 1 above a payload that decodes as its
        // variant's type, its bits above clear, by the repr alone, as `from_repr_unchecked` does
        // unchecked; zero is the first variant's of its payload's zero, so it decodes where that
        // does; each payload may cross threads, as its own impl promises.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl<$($param),+> Atom for $enumeration<$($param),+>
        where
            $first: [const] Atom<Repr: [const] TupleFieldRepr + [const] ExactBits>,
            $second: [const] Atom<Repr: [const] TupleFieldRepr + [const] ExactBits>,
            <$first::Repr as TupleFieldRepr>::Kind:
                Wider<<$second::Repr as TupleFieldRepr>::Kind, Kind: const EnumShape<1>>,
        {
            type Repr = <WiderKind<$first, $second> as EnumShape<1>>::Repr;
            type Validity = <$first::Validity as Validity>::ZeroValidity;
            const REPRS: ReprRange<Self::Repr> = ReprRange::new(
                0,
                1 << <WiderKind<$first, $second> as EnumShape<1>>::WIDTH
                    | mask(<WiderKind<$first, $second> as EnumShape<1>>::WIDTH),
            );
            #[inline]
            fn to_repr(self) -> Self::Repr {
                let width = <WiderKind<$first, $second> as EnumShape<1>>::WIDTH;
                match self {
                    Self::$first_variant(value) => join_variant(0, width, value.to_repr().to_bits()),
                    Self::$second_variant(value) => join_variant(1, width, value.to_repr().to_bits()),
                }
            }
            #[inline]
            fn from_repr(repr: Self::Repr) -> Option<Self> {
                let width = <WiderKind<$first, $second> as EnumShape<1>>::WIDTH;
                match split_variant(repr, width) {
                    (0, bits) => match payload(bits) {
                        Some(value) => Some(Self::$first_variant(value)),
                        None => None,
                    },
                    (1, bits) => match payload(bits) {
                        Some(value) => Some(Self::$second_variant(value)),
                        None => None,
                    },
                    _ => None,
                }
            }
            #[expect(unsafe_code, reason = "decodes the payload of a repr that decodes")]
            #[inline]
            unsafe fn from_repr_unchecked(repr: Self::Repr) -> Self {
                let width = <WiderKind<$first, $second> as EnumShape<1>>::WIDTH;
                let (tag, bits) = split_variant(repr, width);
                if tag == 0 {
                    // SAFETY: the caller's repr decodes, and its tag is 0, so its payload decodes
                    // as the first variant's.
                    Self::$first_variant(unsafe { payload_unchecked(bits) })
                } else {
                    // SAFETY: the caller's repr decodes, and its tag is 1, so its payload decodes as
                    // the second variant's.
                    Self::$second_variant(unsafe { payload_unchecked(bits) })
                }
            }
        }
    )+};
}

/// The kind of a payload of `T`'s.
type PayloadKind<T> = <<T as Atom>::Repr as TupleFieldRepr>::Kind;

/// The wider of `A`'s and `B`'s payload kinds.
type WiderKind<A, B> = <PayloadKind<A> as Wider<PayloadKind<B>>>::Kind;

two_variants! {
    Result<T, E>: T Ok, E Err;
    ControlFlow<B, C>: C Continue, B Break;
}

/// `Included`'s tag: each variant's is its index.
const INCLUDED: u128 = 0;
/// `Excluded`'s.
const EXCLUDED: u128 = 1;
/// `Unbounded`'s, which holds no payload.
const UNBOUNDED: u128 = 2;

// SAFETY: the repr is the payload's repr from bit 0, its bits above clear, and the tag, two bits,
// above the payload's slot, the bits above it clear, which the table gives an integer of; `REPRS`
// runs from zero to `Unbounded`'s, the last; `from_repr` decodes exactly a tag of 0 or 1 above a
// payload that decodes as a `T`, its bits above clear, and `Unbounded`'s tag above no bits, by the
// repr alone, as `from_repr_unchecked` does unchecked; zero is `Included` of the payload's zero,
// so it decodes where that does; a `T` may cross threads, as its own impl promises.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl<T> Atom for Bound<T>
where
    T: [const] Atom<Repr: [const] TupleFieldRepr + [const] ExactBits>,
    <T::Repr as TupleFieldRepr>::Kind: const EnumShape<2>,
{
    type Repr = <PayloadKind<T> as EnumShape<2>>::Repr;
    type Validity = <T::Validity as Validity>::ZeroValidity;
    const REPRS: ReprRange<Self::Repr> =
        ReprRange::new(0, UNBOUNDED << <PayloadKind<T> as EnumShape<2>>::WIDTH);
    #[inline]
    fn to_repr(self) -> Self::Repr {
        let width = <PayloadKind<T> as EnumShape<2>>::WIDTH;
        match self {
            Self::Included(value) => join_variant(INCLUDED, width, value.to_repr().to_bits()),
            Self::Excluded(value) => join_variant(EXCLUDED, width, value.to_repr().to_bits()),
            Self::Unbounded => join_variant(UNBOUNDED, width, 0),
        }
    }
    #[inline]
    fn from_repr(repr: Self::Repr) -> Option<Self> {
        match split_variant(repr, <PayloadKind<T> as EnumShape<2>>::WIDTH) {
            (INCLUDED, bits) => match payload(bits) {
                Some(value) => Some(Self::Included(value)),
                None => None,
            },
            (EXCLUDED, bits) => match payload(bits) {
                Some(value) => Some(Self::Excluded(value)),
                None => None,
            },
            (UNBOUNDED, 0) => Some(Self::Unbounded),
            _ => None,
        }
    }
    #[expect(unsafe_code, reason = "decodes the payload of a repr that decodes")]
    #[inline]
    unsafe fn from_repr_unchecked(repr: Self::Repr) -> Self {
        match split_variant(repr, <PayloadKind<T> as EnumShape<2>>::WIDTH) {
            // SAFETY: the caller's repr decodes, and its tag is `Included`'s, so its payload
            // decodes as a `T`.
            (INCLUDED, bits) => Self::Included(unsafe { payload_unchecked(bits) }),
            // SAFETY: the caller's repr decodes, and its tag is `Excluded`'s, so its payload
            // decodes as a `T`.
            (EXCLUDED, bits) => Self::Excluded(unsafe { payload_unchecked(bits) }),
            _ => Self::Unbounded,
        }
    }
}
