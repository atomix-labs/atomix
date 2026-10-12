//! `Atom` for an array: each element in a slot as wide as its repr, element 0 lowest, so an array
//! of integers is stored as its own bytes on every target atomix builds for, an array of `bool`s
//! one bit each, and an array of two pointers as a double word.

use super::{Atom, AtomBitwise, FieldPatternsDecode};
use crate::atomic::Reach;
#[cfg(wide)]
use crate::primitive::DoubleWord;
#[cfg(wide)]
use crate::primitive::PointerRepr;
use crate::primitive::{Bitwise, CompareExchange, ExactBits, Primitive};
use crate::range::{PointeeAlignment, ReprRange, Tags, assert_aligned, mask};
use crate::validity::{Total, Validity, ZeroValid};

/// The repr of an array of `N` values whose repr is `Self`, each in a slot of `Self`'s width,
/// element 0 lowest, and how the array encodes and decodes: the table an array's repr comes from.
///
/// One impl for each repr and each `N` whose slots fit an atomic, since a primitive chosen by the
/// slots' width, a constant, would need `generic_const_exprs`. A value whose repr is a pointer
/// takes a word, so two of them take a double word.
///
/// # Safety
/// `join` packs each element's repr in its slot, element 0's with `tags` set as its own
/// `to_tagged_repr` sets them, and the bits above the slots clear, within `range` of the elements'
/// range; `split` decodes exactly the reprs `join` gives, each slot as its element, by the repr
/// alone, and `split_unchecked` what `split` decodes; `Validity` is what the slots promise of the
/// repr, given each element's; `TopReach` is `Reach<true>` only where the last slot ends at the
/// repr's top bit; and `HOLDS_TAGS` is true only where `join` sets `tags` in element 0's repr.
#[doc(hidden)]
#[expect(unsafe_code, reason = "an array's `Atom` impl trusts the table's encoding")]
pub impl(crate) const unsafe trait Slots<const N: usize>:
    [const] Primitive
{
    /// The primitive the slots fill: the narrowest integer that holds `N` slots, or a double word
    /// of two pointers.
    type Repr: const Primitive + CompareExchange;
    /// The validity of an array of `N` values whose validity is `V`.
    type Validity<V: const Validity>: const Validity;
    /// `Reach<true>` where the last slot ends at the repr's top bit, so an add to it carries out.
    type TopReach;
    /// Whether element 0's repr takes the tags of a word that holds the array, as one pointer, or
    /// the first of two, does.
    const HOLDS_TAGS: bool;
    /// The range of an array whose elements' reprs lie in `element`.
    fn range(element: ReprRange<Self>) -> ReprRange<Self::Repr>;
    /// The repr of `elements`, element 0's `tags` set, and the bits found set under them.
    fn join<T: [const] Atom<Repr = Self>>(elements: [T; N], tags: Tags) -> (Self::Repr, usize);
    /// The elements `repr` holds, or `None` where it is no array's.
    fn split<T: [const] Atom<Repr = Self>>(repr: Self::Repr) -> Option<[T; N]>;
    /// The elements `repr` holds, without `split`'s check.
    ///
    /// # Safety
    /// `repr` decodes: `split` returns `Some` for it.
    unsafe fn split_unchecked<T: [const] Atom<Repr = Self>>(repr: Self::Repr) -> [T; N];
}

// One element is its own repr, whole: the array takes the element's range, validity and tags.
//
// SAFETY: `join` is element 0's `to_tagged_repr`, and `split` its `from_repr`, which decodes the
// reprs that gives and no other; the validity is the element's, as every repr is one element's;
// the one slot ends at the repr's top bit; and element 0 takes the tags.
#[expect(unsafe_code, reason = "the table's encoding")]
const unsafe impl<R: const Primitive + CompareExchange> Slots<1> for R {
    type Repr = R;
    type Validity<V: const Validity> = V;
    type TopReach = Reach<true>;
    const HOLDS_TAGS: bool = true;
    #[inline]
    fn range(element: ReprRange<R>) -> ReprRange<R> {
        element
    }
    #[inline]
    fn join<T: [const] Atom<Repr = R>>([element]: [T; 1], tags: Tags) -> (R, usize) {
        element.to_tagged_repr(tags)
    }
    #[inline]
    fn split<T: [const] Atom<Repr = R>>(repr: R) -> Option<[T; 1]> {
        match T::from_repr(repr) {
            Some(element) => Some([element]),
            None => None,
        }
    }
    #[expect(unsafe_code, reason = "decodes the element of a repr that decodes")]
    #[inline]
    unsafe fn split_unchecked<T: [const] Atom<Repr = R>>(repr: R) -> [T; 1] {
        // SAFETY: the caller's repr decodes, so as the one element.
        [unsafe { T::from_repr_unchecked(repr) }]
    }
}

/// The repr of `elements`, each a value whose repr is bits, in a slot of `E::BITS` bits, element 0
/// lowest, the bits above the slots clear.
#[inline]
const fn join_slots<
    E: [const] ExactBits,
    T: [const] Atom<Repr = E>,
    R: [const] ExactBits,
    const N: usize,
>(
    elements: [T; N],
) -> R {
    let mut bits = 0;
    let mut offset = 0_u32;
    let mut rest = elements.as_slice();
    while let [element, after @ ..] = rest {
        bits |= element.to_repr().to_bits().unbounded_shl(offset);
        offset = offset.wrapping_add(E::BITS);
        rest = after;
    }
    R::from_bits(bits)
}

/// The elements of `repr`, each a value whose repr is bits in its slot, or `None` where a bit
/// above the slots is set or a slot does not decode.
#[inline]
const fn split_slots<
    E: [const] ExactBits,
    T: [const] Atom<Repr = E>,
    R: [const] ExactBits,
    const N: usize,
>(
    repr: R,
) -> Option<[T; N]> {
    let mut slots = repr.to_bits();
    if slots & !mask(E::BITS.wrapping_mul(N.wrapping_cast())) != 0 {
        return None;
    }
    let Some(first) = T::from_repr(E::from_bits(slots)) else {
        return None;
    };
    let mut elements = [first; N];
    let mut rest = elements.as_mut_slice();
    while let [element, after @ ..] = rest {
        let Some(decoded) = T::from_repr(E::from_bits(slots)) else {
            return None;
        };
        *element = decoded;
        slots = slots.unbounded_shr(E::BITS);
        rest = after;
    }
    Some(elements)
}

/// The elements of `repr`, as [`split_slots`] decodes them, without its checks.
///
/// # Safety
/// `repr` decodes: `split_slots` returns `Some` for it.
#[expect(unsafe_code, reason = "decodes each slot of a repr that decodes")]
#[inline]
const unsafe fn split_slots_unchecked<
    E: [const] ExactBits,
    T: [const] Atom<Repr = E>,
    R: [const] ExactBits,
    const N: usize,
>(
    repr: R,
) -> [T; N] {
    let mut slots = repr.to_bits();
    // SAFETY: the caller's repr decodes, so each slot does, as its element.
    let mut elements = [unsafe { T::from_repr_unchecked(E::from_bits(slots)) }; N];
    let mut rest = elements.as_mut_slice();
    while let [element, after @ ..] = rest {
        // SAFETY: as above.
        *element = unsafe { T::from_repr_unchecked(E::from_bits(slots)) };
        slots = slots.unbounded_shr(E::BITS);
        rest = after;
    }
    elements
}

/// Implements `Slots` for an element repr stored as bits, the integer its slots fill, and whether
/// they fill it, `Full`, or leave bits above, `Padded`, for each count.
macro_rules! slots {
    ($($element:ty => $repr:ty, $fill:ident: $($count:literal)+;)+) => {$($(
        // SAFETY: `join` packs each element's bits in its slot, element `i` at `i` slots up, the
        // bits above clear, within `of_slots` of the elements' range, and sets `tags` as
        // `Tags::set_in` sets them, which a value of bits takes none of; `split` refuses a bit
        // above the slots and decodes each slot alone, by the repr alone, and `split_unchecked`
        // decodes each unchecked; a repr decodes where each slot does, padded bits clear, as
        // `Elements` and `PaddedElements` say; and the last slot ends at the top bit exactly where
        // the slots fill the repr.
        #[expect(unsafe_code, reason = "the table's encoding")]
        const unsafe impl Slots<$count> for $element {
            type Repr = $repr;
            type Validity<V: const Validity> = slots!(@validity $fill V);
            type TopReach = Reach<{ slots!(@fills $fill) }>;
            const HOLDS_TAGS: bool = false;
            #[inline]
            fn range(element: ReprRange<$element>) -> ReprRange<$repr> {
                ReprRange::of_slots(element, $count)
            }
            #[inline]
            fn join<T: [const] Atom<Repr = $element>>(
                elements: [T; $count], tags: Tags,
            ) -> ($repr, usize) {
                tags.set_in(join_slots(elements))
            }
            #[inline]
            fn split<T: [const] Atom<Repr = $element>>(repr: $repr) -> Option<[T; $count]> {
                split_slots(repr)
            }
            #[expect(unsafe_code, reason = "decodes each slot of a repr that decodes")]
            #[inline]
            unsafe fn split_unchecked<T: [const] Atom<Repr = $element>>(
                repr: $repr,
            ) -> [T; $count] {
                // SAFETY: the caller's repr decodes.
                unsafe { split_slots_unchecked(repr) }
            }
        }
    )+)+};
    (@validity Full $validity:ident) => { <$validity as Validity>::Elements };
    (@validity Padded $validity:ident) => { <$validity as Validity>::PaddedElements };
    (@fills Full) => { true };
    (@fills Padded) => { false };
}

slots! {
    bool => u8, Full: 8;
    bool => u8, Padded: 2 3 4 5 6 7;
    bool => u16, Full: 16;
    bool => u16, Padded: 9 10 11 12 13 14 15;
    bool => u32, Full: 32;
    bool => u32, Padded: 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31;
    bool => u64, Full: 64;
    bool => u64, Padded: 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63;
    u8 => u16, Full: 2;
    u8 => u32, Full: 4;
    u8 => u32, Padded: 3;
    u8 => u64, Full: 8;
    u8 => u64, Padded: 5 6 7;
    i8 => u16, Full: 2;
    i8 => u32, Full: 4;
    i8 => u32, Padded: 3;
    i8 => u64, Full: 8;
    i8 => u64, Padded: 5 6 7;
    u16 => u32, Full: 2;
    u16 => u64, Full: 4;
    u16 => u64, Padded: 3;
    i16 => u32, Full: 2;
    i16 => u64, Full: 4;
    i16 => u64, Padded: 3;
    u32 => u64, Full: 2;
    i32 => u64, Full: 2;
}

#[cfg(wide)]
slots! {
    bool => u128, Full: 128;
    bool => u128, Padded: 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79 80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95 96 97 98 99 100 101 102 103 104 105 106 107 108 109 110 111 112 113 114 115 116 117 118 119 120 121 122 123 124 125 126 127;
    u8 => u128, Full: 16;
    u8 => u128, Padded: 9 10 11 12 13 14 15;
    i8 => u128, Full: 16;
    i8 => u128, Padded: 9 10 11 12 13 14 15;
    u16 => u128, Full: 8;
    u16 => u128, Padded: 5 6 7;
    i16 => u128, Full: 8;
    i16 => u128, Padded: 5 6 7;
    u32 => u128, Full: 4;
    u32 => u128, Padded: 3;
    i32 => u128, Full: 4;
    i32 => u128, Padded: 3;
    u64 => u128, Full: 2;
    i64 => u128, Full: 2;
    usize => u128, Full: 2;
    isize => u128, Full: 2;
}

// Two pointers take a double word, as a pair of them does: element 0's word takes the tags, and the
// array decodes where each word does, its zero where each element's.
//
// SAFETY: `join` is each element's repr in its word, element 0's `to_tagged_repr` with `tags`,
// within every repr, or every one but zero where an element's range leaves zero out, as its
// pointer then has a bit set; `split` decodes exactly where each word decodes as its element, by
// the repr alone, and `split_unchecked` each unchecked; the validity promises zero decodes only
// where each element's does; the second slot ends below no top bit an add reaches; and element 0
// takes the tags.
#[cfg(wide)]
#[expect(unsafe_code, reason = "the table's encoding")]
const unsafe impl<R: const PointerRepr> Slots<2> for R {
    type Repr = DoubleWord<R, R>;
    type Validity<V: const Validity> =
        <V as Validity>::ZeroValidityWith<<V as Validity>::ZeroValidityWith<ZeroValid>>;
    type TopReach = Reach<false>;
    const HOLDS_TAGS: bool = true;
    #[inline]
    fn range(element: ReprRange<R>) -> ReprRange<DoubleWord<R, R>> {
        if element.contains(0) { ReprRange::FULL } else { ReprRange::NON_ZERO }
    }
    #[inline]
    fn join<T: [const] Atom<Repr = R>>([first, second]: [T; 2], tags: Tags) -> (Self::Repr, usize) {
        let (first, first_misaligned) = first.to_tagged_repr(tags);
        let (second, second_misaligned) = second.to_tagged_repr(Tags::EMPTY);
        (DoubleWord { first, second }, first_misaligned | second_misaligned)
    }
    #[inline]
    fn split<T: [const] Atom<Repr = R>>(repr: Self::Repr) -> Option<[T; 2]> {
        match (T::from_repr(repr.first), T::from_repr(repr.second)) {
            (Some(first), Some(second)) => Some([first, second]),
            _ => None,
        }
    }
    #[expect(unsafe_code, reason = "decodes each word of a repr that decodes")]
    #[inline]
    unsafe fn split_unchecked<T: [const] Atom<Repr = R>>(repr: Self::Repr) -> [T; 2] {
        // SAFETY: the caller's repr decodes, so its first word decodes as an element.
        let first = unsafe { T::from_repr_unchecked(repr.first) };
        // SAFETY: and its second.
        let second = unsafe { T::from_repr_unchecked(repr.second) };
        [first, second]
    }
}

/// Stored as `N` slots of the element's repr, element 0 lowest: an array of integers as its own
/// bytes, the repr an integer as wide as they are, so `[u8; 4]` is a `u32` and every operation
/// `u32`'s instructions; an array of `bool`s one bit each; and an array of two pointers a
/// `DoubleWord`, as a pair of them is. An array of one element is that element's repr, whole.
///
/// It has every repr that decodes where each element's does and the slots fill the repr, so
/// `[u8; 4]` is [`Total`]; where they leave bits above, as `[u8; 3]` in a `u32`, those bits are
/// clear. An atomic holds 128 bits at most, so an array of more is refused, as one of no elements
/// is.
///
/// The bitwise operations combine each element with its own, where it has them and the slots fill
/// the repr; there is no add, whose carry would cross from one element into the next, and no
/// maximum or minimum, since an array orders by its first element and the repr by its last, at the
/// top. Each element is a place of its own, as [`Atomic::element`](crate::Atomic::element) says.
// SAFETY: the table keeps each promise of the encoding, as `Slots`' contract says, and the impl
// passes each on: the repr lies in `range`, decodes as the array, and decodes by the repr alone,
// as `from_repr_unchecked` does unchecked; `to_repr` refuses an element misaligned for no tag, as
// a pointer's own impl does; the validity is the table's, of the elements'; `TAG_WIDTH` and
// `POINTEE_ALIGNMENT` are element 0's where its repr takes a word's tags, else none; and each
// element may cross threads, as its own impl promises.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl<T: [const] Atom, const N: usize> Atom for [T; N]
where
    T::Repr: const Slots<N>,
{
    type Repr = <T::Repr as Slots<N>>::Repr;
    type Validity = <T::Repr as Slots<N>>::Validity<T::Validity>;
    const REPRS: ReprRange<Self::Repr> = <T::Repr as Slots<N>>::range(T::REPRS);
    const TAG_WIDTH: u32 = if <T::Repr as Slots<N>>::HOLDS_TAGS { T::TAG_WIDTH } else { 0 };
    const POINTEE_ALIGNMENT: PointeeAlignment = if <T::Repr as Slots<N>>::HOLDS_TAGS {
        T::POINTEE_ALIGNMENT
    } else {
        PointeeAlignment::NONE
    };
    #[inline]
    #[track_caller]
    fn to_repr(self) -> Self::Repr {
        let (repr, misaligned) = <T::Repr as Slots<N>>::join(self, Tags::EMPTY);
        assert_aligned::<Self>(misaligned);
        repr
    }
    #[inline]
    fn to_tagged_repr(self, tags: Tags) -> (Self::Repr, usize) {
        <T::Repr as Slots<N>>::join(self, tags)
    }
    #[inline]
    fn from_repr(repr: Self::Repr) -> Option<Self> {
        <T::Repr as Slots<N>>::split(repr)
    }
    #[expect(unsafe_code, reason = "decodes a repr that decodes")]
    #[inline]
    unsafe fn from_repr_unchecked(repr: Self::Repr) -> Self {
        // SAFETY: the caller's repr decodes.
        unsafe { <T::Repr as Slots<N>>::split_unchecked(repr) }
    }
}

/// And, or, xor and not on the slots combine each element with its own, where every repr of each
/// decodes and the slots fill the repr.
// `do_not_recommend`, so a padded array reports `AtomBitwise`'s message, not its validity's
// mismatch.
#[diagnostic::do_not_recommend]
impl<T: AtomBitwise, const N: usize> AtomBitwise for [T; N] where
    Self: Atom<Validity = Total, Repr: Bitwise>
{
}

// A padded array of elements whose every repr decodes, as `[u8; 3]` in a `u32`, is no
// `AtomBitwise`, whose `not` would set the bits above its slots, but its field is the slots alone.
//
// SAFETY: the array's range lies within its slots, as `of_slots` builds it, so a field of it takes
// the slots' bits or fewer, and each pattern of them leaves every slot a repr of its element, each
// of which decodes, as `AtomBitwise`'s `Total` says, and the bits above the slots clear: a repr
// of the array that decodes. The validity of an array of `AtomBitwise` elements is `ZeroValid`
// only where its slots leave bits above.
#[expect(unsafe_code, reason = "a padded array's field takes every pattern of its slots")]
unsafe impl<T: AtomBitwise, const N: usize> FieldPatternsDecode<[T; N]> for ZeroValid where
    [T; N]: Atom<Validity = Self>
{
}
