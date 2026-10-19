//! `Atom` for a tuple of one to twelve values, laid out as `#[derive(Atom)]` lays out the struct of
//! its fields.
//!
//! Each field stored as bits, an integer or a `bool`, sits in a slot of its repr's width, side by
//! side from bit 0 in declaration order, as an array's elements do. A field stored as a pointer
//! takes a word of its own, its provenance kept, and the others lie in its low bits, above its own
//! tags, where they take 29 bits or fewer, as a pointer word's tags do, else in an integer word
//! beside it. A function pointer's code leaves no low bit clear, so beside one the others take an
//! integer word whatever their bits, as a derived struct's take with `#[atom(repr = u128)]`. Two
//! pointers take two words, the others in the first one's low bits.
//!
//! A generic impl chooses its repr from types alone, so a hidden table folds the kinds of the
//! fields' reprs, a width, a pointer to data or a function pointer, into the tuple's shape: the
//! bits its integer fields take, and its pointers. Each field's place is a constant.

use core::any::type_name;
use core::marker::PhantomData;
use core::ptr;

use super::{Atom, AtomBitwise, FieldPatternsDecode};
use crate::atomic::{
    AtomicField, Field, FieldPath, HasPackedField, Join, ProjectFields, Reach, ReachesTop,
    project_field,
};
use crate::message::{Message, refuse};
#[cfg(wide)]
use crate::primitive::Code;
use crate::primitive::{Bitwise, CompareExchange, Data, ExactBits, PointerRepr, Primitive};
use crate::range::{PackedField, PackedLayout, PointeeAlignment, ReprRange, Tags, assert_aligned};
use crate::validity::{Total, Validity, ZeroValid};

/// The primitive a tuple field is stored as: bits, an integer or a `bool`, or a thin pointer.
#[doc(hidden)]
pub impl(crate) const trait TupleFieldRepr: [const] Primitive {
    /// Its kind: [`Integer`] for bits, [`Pointer`] for a pointer.
    type Kind;
    /// Whether `Kind` is a [`Pointer`]: what a tuple's seats read, so each field sits where the
    /// fold of the kinds puts it.
    const IS_POINTER: bool;
    /// The pointer it holds, its provenance kept; null for bits, which hold none.
    fn pointer(self) -> *mut ();
    /// The repr whose `pointer` gave `pointer`; zero for bits, which no tuple rebuilds so.
    ///
    /// # Safety
    /// `pointer` is one `pointer` gave a repr of this type.
    #[expect(unsafe_code, reason = "a sealed repr is rebuilt only from what it gave")]
    unsafe fn from_pointer(pointer: *mut ()) -> Self;
}

/// A field stored as bits, an integer or a `bool`, `WIDTH` bits wide.
#[doc(hidden)]
#[derive(Debug)]
pub struct Integer<const WIDTH: u32>;

/// A field stored as the pointer `R`.
#[doc(hidden)]
#[derive(Debug)]
pub struct Pointer<R>(PhantomData<fn() -> R>);

/// Implements `TupleFieldRepr` for the integers and `bool`, by width.
macro_rules! integer_reprs {
    ($($width:literal: $($int:ty),+;)+) => {$($(
        const impl TupleFieldRepr for $int {
            type Kind = Integer<$width>;
            const IS_POINTER: bool = false;
            #[inline]
            fn pointer(self) -> *mut () {
                ptr::null_mut()
            }
            #[expect(unsafe_code, reason = "the trait's contract, which bits need none of")]
            #[inline]
            unsafe fn from_pointer(_: *mut ()) -> Self {
                Primitive::from_bits(0)
            }
        }
    )+)+};
}

integer_reprs! {
    1: bool;
    8: u8, i8;
    16: u16, i16;
    32: u32, i32;
    64: u64, i64, usize, isize;
}
#[cfg(wide)]
integer_reprs!(128: u128, i128;);

// Every pointer repr is a `Pointer`, whatever it points to, so a generic tuple of pointers, whose
// reprs a bound names alone, folds; only the integer fields beside one read its `Pointee`.
const impl<R: const PointerRepr> TupleFieldRepr for R {
    type Kind = Pointer<R>;
    const IS_POINTER: bool = true;
    #[inline]
    fn pointer(self) -> *mut () {
        self.into_raw()
    }
    #[expect(unsafe_code, reason = "rebuilds a pointer repr from what it gave")]
    #[inline]
    unsafe fn from_pointer(pointer: *mut ()) -> Self {
        // SAFETY: the caller's pointer is one `pointer`, which is `into_raw`, gave an `R`.
        unsafe { R::from_raw(pointer) }
    }
}

/// Where the integer fields beside a pointer to a `Self` lie: in its low bits, [`Tagged`], beside
/// [`Data`], whose alignment may leave them clear; in a word of their own, [`IntegerWord`], beside
/// [`Code`], which leaves none clear.
///
/// The pointer's type decides, never an alignment, so the layout reads none, as
/// [`Atom::POINTEE_ALIGNMENT`] says. Integers beside code take two words, which an atomic holds
/// only where it holds 16 bytes.
#[doc(hidden)]
pub impl(crate) trait IntegersBeside {
    /// The shape of the pointer `R` beside integer fields that take `BITS` bits, 29 or fewer.
    type Shape<R, const BITS: u32>;
}

impl IntegersBeside for Data {
    type Shape<R, const BITS: u32> = Tagged<R, BITS>;
}

#[cfg(wide)]
impl IntegersBeside for Code {
    type Shape<R, const BITS: u32> = IntegerWord<R, BITS>;
}

/// A tuple's shape so far: its fields are bits alone, which take `BITS` bits.
#[doc(hidden)]
#[derive(Debug)]
pub struct Integers<const BITS: u32>;

/// A tuple's shape so far: one pointer `R`, its integer fields' `BITS` bits in its low bits, as a
/// pointer word's tags: one word.
#[doc(hidden)]
#[derive(Debug)]
pub struct Tagged<R, const BITS: u32>(PhantomData<fn() -> R>);

/// A tuple's shape so far: one pointer `R`, beside integer fields that take `BITS` bits in a word
/// of their own.
#[doc(hidden)]
#[derive(Debug)]
pub struct IntegerWord<R, const BITS: u32>(PhantomData<fn() -> R>);

/// A tuple's shape: two pointers, `R` and `Q`, its integer fields' `BITS` bits in the first one's
/// low bits.
#[doc(hidden)]
#[derive(Debug)]
pub struct TwoPointers<R, Q, const BITS: u32>(PhantomData<fn() -> (R, Q)>);

/// The shape after a field of the kind `K`.
#[doc(hidden)]
pub impl(crate) trait Push<K> {
    /// The shape.
    type Next;
}

/// The shape a list of field reprs, `(A, (B, ()))`, folds into from the shape `S`.
#[doc(hidden)]
pub impl(crate) trait Fold<S> {
    /// The shape.
    type Shape;
}

impl<S> Fold<S> for () {
    type Shape = S;
}

impl<S: Push<H::Kind>, H: TupleFieldRepr, T: Fold<S::Next>> Fold<S> for (H, T) {
    type Shape = T::Shape;
}

/// The two pointer fields of a tuple, each its provenance kept; null where it has fewer.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct Pointers {
    /// The first.
    first: *mut (),
    /// The second.
    second: *mut (),
}

impl Pointers {
    /// No pointers.
    const NONE: Self = Self { first: ptr::null_mut(), second: ptr::null_mut() };
}

/// A whole tuple's shape: the repr it takes, and how its integer bits and pointers sit in it.
///
/// # Safety
/// `join` gives the repr of the pointers and bits it takes, which `split` gives back, each pointer
/// as it was, its provenance kept, the first one's tags cleared where the integer bits lie there.
/// The integer bits lie from `BASE`, or, where `INTEGERS_IN_POINTER`, from `base`, above the first
/// pointer's own tags, where `pointer_tags` sets them; `split` reads them of every repr from
/// there, below `BITS` where the repr is one `join` gave. `Validity<V>` is what the repr promises
/// where the fields side by side promise `V`, and `TopReach` is `Reach<true>` only where the
/// integer bits run from bit 0 to the repr's top bit.
#[doc(hidden)]
#[expect(unsafe_code, reason = "a tuple's `Atom` impl trusts the shape's encoding")]
pub impl(crate) const unsafe trait Shape {
    /// The repr.
    type Repr: const Primitive + CompareExchange;
    /// The validity of a tuple whose fields side by side promise `V`.
    type Validity<V: const Validity>: const Validity;
    /// `Reach<true>` where the integer bits fill the repr, so the last field's slot ends at its
    /// top bit.
    type TopReach;
    /// Where the integer bits start: bit 0, or the second word beside a pointer; in a pointer's
    /// low bits, above its own tags, where `INTEGERS_IN_POINTER`.
    const BASE: u32;
    /// How many bits the integer fields take.
    const BITS: u32;
    /// Whether the integer fields lie in the first pointer's low bits.
    const INTEGERS_IN_POINTER: bool;
    /// The tags the first pointer sets: where the integer fields lie in its low bits, their `bits`,
    /// above its own `base` tag bits, beside `outer`, a holding word's.
    #[inline]
    #[must_use]
    fn pointer_tags(bits: u128, base: u32, outer: Tags) -> Tags {
        let _ = (bits, base);
        outer
    }
    /// The repr of `pointers`, tags set, and the integer fields' `bits`.
    ///
    /// # Safety
    /// Each pointer is one a field's `pointer` gave, of the repr type the shape holds in its
    /// place.
    unsafe fn join(pointers: Pointers, bits: u128) -> Self::Repr;
    /// The pointers and the integer fields' bits of `repr`, whose first pointer keeps its own tags
    /// in `base` low bits.
    fn split(repr: Self::Repr, base: u32) -> (Pointers, u128);
}

/// Whether a tuple's integer fields fill their repr, as a type: [`Padding`] says what that does to
/// the validity.
#[doc(hidden)]
#[derive(Debug)]
pub struct Fill<const FULL: bool>;

/// What clear bits above a tuple's integer fields do to its validity.
#[doc(hidden)]
pub impl(crate) const trait Padding {
    /// The validity, the fields side by side promising `V`.
    type Validity<V: const Validity>: const Validity;
}

const impl Padding for Fill<true> {
    type Validity<V: const Validity> = V;
}

const impl Padding for Fill<false> {
    type Validity<V: const Validity> = V::PaddedElements;
}

/// Implements `Push` of an integer after integers of each count of bits.
macro_rules! integers_push {
    ($($width:literal: $($bits:literal)+;)+) => {$($(
        impl Push<Integer<$width>> for Integers<$bits> {
            type Next = Integers<{ $bits + $width }>;
        }
    )+)+};
}

/// Implements `Push` of a pointer after integers of each count of bits, which share its word.
macro_rules! integers_to_tagged {
    ($($bits:literal)+) => {$(
        impl<R> Push<Pointer<R>> for Integers<$bits> {
            type Next = Tagged<R, $bits>;
        }
    )+};
}

/// Implements `Push` of a pointer after integers of each count of bits, too many to share its word.
#[cfg(wide)]
macro_rules! integers_to_integer_word {
    ($($bits:literal)+) => {$(
        impl<R> Push<Pointer<R>> for Integers<$bits> {
            type Next = IntegerWord<R, $bits>;
        }
    )+};
}

/// Implements `Push` of a pointer after integers of each count of bits, 29 or fewer: in its low
/// bits beside data, in a word of their own beside code, as its `Pointee` says.
macro_rules! integers_to_pointer {
    ($($bits:literal)+) => {$(
        impl<R: PointerRepr<Pointee: IntegersBeside>> Push<Pointer<R>> for Integers<$bits> {
            type Next = <R::Pointee as IntegersBeside>::Shape<R, $bits>;
        }
    )+};
}

/// Implements `Push` of an integer of each width, 29 bits or fewer, after a pointer alone: in its
/// low bits beside data, in a word of its own beside code, as its `Pointee` says. Beside an integer
/// already, the pointer is data's, as `Tagged` holds no other.
macro_rules! pointer_push {
    ($($width:literal)+) => {$(
        impl<R: PointerRepr<Pointee: IntegersBeside>> Push<Integer<$width>> for Tagged<R, 0> {
            type Next = <R::Pointee as IntegersBeside>::Shape<R, $width>;
        }
    )+};
}

/// Implements `Push` of an integer after a pointer whose word it still shares.
macro_rules! tagged_push {
    ($($width:literal: $($bits:literal)+;)+) => {$($(
        impl<R> Push<Integer<$width>> for Tagged<R, $bits> {
            type Next = Tagged<R, { $bits + $width }>;
        }
    )+)+};
}

/// Implements `Push` of an integer after a pointer whose word it no longer fits: the integers move
/// to a word of their own.
#[cfg(wide)]
macro_rules! tagged_to_integer_word {
    ($($width:literal: $($bits:literal)+;)+) => {$($(
        impl<R> Push<Integer<$width>> for Tagged<R, $bits> {
            type Next = IntegerWord<R, { $bits + $width }>;
        }
    )+)+};
}

/// Implements `Push` of an integer after a pointer and integers that take each count of bits in a
/// word of their own.
#[cfg(wide)]
macro_rules! integer_word_push {
    ($($width:literal: $($bits:literal)+;)+) => {$($(
        impl<R> Push<Integer<$width>> for IntegerWord<R, $bits> {
            type Next = IntegerWord<R, { $bits + $width }>;
        }
    )+)+};
}

/// Implements `Push` of a second pointer after a first whose low bits hold integers of each count
/// of bits.
#[cfg(wide)]
macro_rules! tagged_to_two_pointers {
    ($($bits:literal)+) => {$(
        impl<R, Q> Push<Pointer<Q>> for Tagged<R, $bits> {
            type Next = TwoPointers<R, Q, $bits>;
        }
    )+};
}

/// Implements `Push` of an integer after two pointers, into the first one's low bits.
#[cfg(wide)]
macro_rules! two_pointers_push {
    ($($width:literal: $($bits:literal)+;)+) => {$($(
        impl<R, Q> Push<Integer<$width>> for TwoPointers<R, Q, $bits> {
            type Next = TwoPointers<R, Q, { $bits + $width }>;
        }
    )+)+};
}

/// Implements `Shape` for integer shapes of each count of bits.
macro_rules! integers_finish {
    ($($bits:literal)+) => {$(
        // SAFETY: the repr is the bits alone, which `split` reads back from bit 0; there is no
        // pointer. Where `$bits` is the repr's width, every repr is the fields side by side, which
        // promise `V`, and the last slot ends at the top bit, `Reach<true>`; else `from_repr`
        // refuses a bit above them, `V::PaddedElements`, and no slot reaches the top.
        #[expect(unsafe_code, reason = "the shape's encoding")]
        const unsafe impl Shape for Integers<$bits> {
            type Repr = <Width<{ narrowest_width($bits) }> as SelectRepr>::Repr;
            type Validity<V: const Validity> =
                <Fill<{ $bits == narrowest_width($bits) }> as Padding>::Validity<V>;
            type TopReach = Reach<{ $bits == narrowest_width($bits) }>;
            const BASE: u32 = 0;
            const BITS: u32 = $bits;
            const INTEGERS_IN_POINTER: bool = false;
            #[expect(unsafe_code, reason = "the trait's contract, which bits need none of")]
            #[inline]
            unsafe fn join(_: Pointers, bits: u128) -> Self::Repr {
                Primitive::from_bits(bits)
            }
            #[inline]
            fn split(repr: Self::Repr, _: u32) -> (Pointers, u128) {
                (Pointers::NONE, repr.to_bits())
            }
        }
    )+};
}

/// Implements `Shape` for a pointer beside integers of each count of bits in its low bits: one
/// word, as a pointer word.
macro_rules! tagged_finish {
    ($($bits:literal)+) => {$(
        // SAFETY: the repr is the pointer, the integer bits set in its low bits above `base` by its
        // own `to_tagged_repr`, as `pointer_tags` passes them; `split` clears them, which leaves the
        // pointer as it was, and reads them back. Every repr is the pointer and the bits beside
        // it, each field read alone, so it promises the fields' `V`; no slot reaches the top.
        #[expect(unsafe_code, reason = "the shape's encoding")]
        const unsafe impl<R: const PointerRepr> Shape for Tagged<R, $bits> {
            type Repr = R;
            type Validity<V: const Validity> = V;
            type TopReach = Reach<false>;
            const BASE: u32 = 0;
            const BITS: u32 = $bits;
            const INTEGERS_IN_POINTER: bool = true;
            #[inline]
            fn pointer_tags(bits: u128, base: u32, outer: Tags) -> Tags {
                let own = mask($bits).unbounded_shl(base);
                outer.with(own.wrapping_cast(), bits.unbounded_shl(base).wrapping_cast())
            }
            #[expect(unsafe_code, reason = "rebuilds the pointer the field gave")]
            #[inline]
            unsafe fn join(pointers: Pointers, _: u128) -> R {
                // SAFETY: the caller's first pointer is one an `R`'s `pointer`, its `into_raw`,
                // gave.
                unsafe { R::from_raw(pointers.first) }
            }
            #[inline]
            fn split(repr: R, base: u32) -> (Pointers, u128) {
                let own = mask($bits).unbounded_shl(base);
                let bits = (Primitive::packed_bits(repr) & own).unbounded_shr(base);
                let first = Primitive::clear_packed_bits(repr, own).into_raw();
                (Pointers { first, ..Pointers::NONE }, bits)
            }
        }
    )+};
}

/// Implements `Shape` for a pointer beside integers of each count of bits in a word of their own:
/// two words.
#[cfg(wide)]
macro_rules! integer_word_finish {
    ($($bits:literal)+) => {$(
        // SAFETY: the repr is the pointer, in the first word, beside the integer bits, in the
        // second, which `split` reads back each as it was. `from_repr` refuses a bit of the second
        // word above `$bits`, so the repr promises only what `V` does of zero; no slot reaches the
        // top.
        #[expect(unsafe_code, reason = "the shape's encoding")]
        const unsafe impl<R: const PointerRepr> Shape for IntegerWord<R, $bits> {
            type Repr = DoubleWord<R, usize>;
            type Validity<V: const Validity> = V::ZeroValidity;
            type TopReach = Reach<false>;
            const BASE: u32 = usize::BITS;
            const BITS: u32 = $bits;
            const INTEGERS_IN_POINTER: bool = false;
            #[expect(unsafe_code, reason = "rebuilds the pointer the field gave")]
            #[inline]
            unsafe fn join(pointers: Pointers, bits: u128) -> Self::Repr {
                // SAFETY: the caller's first pointer is one an `R`'s `pointer`, its `into_raw`,
                // gave.
                let first = unsafe { R::from_raw(pointers.first) };
                DoubleWord { first, second: bits.wrapping_cast() }
            }
            #[inline]
            fn split(repr: Self::Repr, _: u32) -> (Pointers, u128) {
                (Pointers { first: repr.first.into_raw(), ..Pointers::NONE }, repr.second.wrapping_cast())
            }
        }
    )+};
}

/// Implements `Shape` for two pointers, the first one's low bits holding integers of each count of
/// bits.
#[cfg(wide)]
macro_rules! two_pointers_finish {
    ($($bits:literal)+) => {$(
        // SAFETY: the repr is the two pointers, the integer bits set in the first one's low bits
        // above `base` by its own `to_tagged_repr`, as `pointer_tags` passes them; `split` clears
        // them, which leaves each pointer as it was, and reads them back. The repr promises only
        // what `V` does of zero, two nulls and no bits; no slot reaches the top.
        #[expect(unsafe_code, reason = "the shape's encoding")]
        const unsafe impl<R: const PointerRepr, Q: const PointerRepr> Shape
            for TwoPointers<R, Q, $bits>
        {
            type Repr = DoubleWord<R, Q>;
            type Validity<V: const Validity> = V::ZeroValidity;
            type TopReach = Reach<false>;
            const BASE: u32 = 0;
            const BITS: u32 = $bits;
            const INTEGERS_IN_POINTER: bool = true;
            #[inline]
            fn pointer_tags(bits: u128, base: u32, outer: Tags) -> Tags {
                let own = mask($bits).unbounded_shl(base);
                outer.with(own.wrapping_cast(), bits.unbounded_shl(base).wrapping_cast())
            }
            #[expect(unsafe_code, reason = "rebuilds the pointers the fields gave")]
            #[inline]
            unsafe fn join(pointers: Pointers, _: u128) -> Self::Repr {
                // SAFETY: the caller's first pointer is one an `R`'s `pointer`, its `into_raw`,
                // gave.
                let first = unsafe { R::from_raw(pointers.first) };
                // SAFETY: and its second one a `Q`'s.
                let second = unsafe { Q::from_raw(pointers.second) };
                DoubleWord { first, second }
            }
            #[inline]
            fn split(repr: Self::Repr, base: u32) -> (Pointers, u128) {
                let own = mask($bits).unbounded_shl(base);
                let bits = (Primitive::packed_bits(repr.first) & own).unbounded_shr(base);
                let first = Primitive::clear_packed_bits(repr.first, own).into_raw();
                (Pointers { first, second: repr.second.into_raw() }, bits)
            }
        }
    )+};
}

// Generated, and checked against its generator, line for line.
#[rustfmt::skip]
mod table;

/// What a tuple's layout reads of one field: never its pointee's alignment, which a type's layout
/// may not read, as [`Atom::POINTEE_ALIGNMENT`] says.
#[derive(Clone, Copy)]
struct Member {
    /// Whether it is stored as a pointer.
    is_pointer: bool,
    /// Its repr's width, its slot's.
    bits: u32,
    /// Its range's bounds.
    reprs: (u128, u128),
    /// A pointer's own tag bits.
    tag_width: u32,
    /// Its place in a packed value, at offset 0.
    field: PackedField,
}

impl Member {
    /// Field `T`'s.
    const fn of<T: Atom<Repr: TupleFieldRepr>>() -> Self {
        Self {
            is_pointer: <T::Repr as TupleFieldRepr>::IS_POINTER,
            bits: <T::Repr as Primitive>::BITS,
            reprs: T::REPRS.bounds(),
            tag_width: T::TAG_WIDTH,
            field: PackedField::new(T::REPRS, 0),
        }
    }
}

/// Where a field sits in a tuple's repr.
#[derive(Clone, Copy)]
struct Seat {
    /// Whether it is stored as a pointer.
    is_pointer: bool,
    /// Whether it is the first pointer, whose low bits take a holding word's tags.
    is_first_pointer: bool,
    /// An integer's offset among the integer bits.
    offset: u32,
}

/// The seats of `members`, in order; past them, unused.
const fn seats(members: &[Member]) -> [Seat; 12] {
    let mut seats = [Seat { is_pointer: false, is_first_pointer: false, offset: 0 }; 12];
    let (mut pointers, mut offset) = (0_u32, 0_u32);
    let mut rest = members;
    let mut out = seats.as_mut_slice();
    while let ([member, after @ ..], [seat, others @ ..]) = (rest, out) {
        *seat = if member.is_pointer {
            pointers = pointers.wrapping_add(1);
            Seat { is_pointer: true, is_first_pointer: pointers == 1, offset: 0 }
        } else {
            offset = offset.wrapping_add(member.bits);
            Seat {
                is_pointer: false,
                is_first_pointer: false,
                offset: offset.wrapping_sub(member.bits),
            }
        };
        rest = after;
        out = others;
    }
    seats
}

/// The first of `members` stored as a pointer.
const fn first_pointer(members: &[Member]) -> Option<Member> {
    let mut rest = members;
    while let [member, after @ ..] = rest {
        if member.is_pointer {
            return Some(*member);
        }
        rest = after;
    }
    None
}

/// The first pointer's own tag bits among `members`; none without a pointer.
const fn first_pointer_tag_width(members: &[Member]) -> u32 {
    match first_pointer(members) {
        Some(member) => member.tag_width,
        None => 0,
    }
}

/// The pointee alignment of the first of `fields` stored as a pointer, each field given as whether
/// it is one and its pointee's alignment: kept out of [`Member`], as its doc says.
const fn first_pointee_alignment(fields: &[(bool, PointeeAlignment)]) -> PointeeAlignment {
    let mut rest = fields;
    while let [(pointer, alignment), after @ ..] = rest {
        if *pointer {
            return *alignment;
        }
        rest = after;
    }
    PointeeAlignment::NONE
}

/// Refuses the build of `T`, a tuple whose tags, `tag_width` bits, need more low bits than its
/// first pointer leaves clear, as its pointee's alignment says, a function's none: code reads the
/// alignment, never the layout.
///
/// Every use of a tuple reaches it: its encode, both its decodes and its projection, so one an
/// atomic holds without an encode, through `Zeroable` or `from_ptr`, is refused too.
#[track_caller]
const fn assert_tags_fit<T>(tag_width: u32, alignment: PointeeAlignment) {
    if tag_width != 0 && alignment.width() < tag_width {
        let rest = concat!(
            "`: its fields stored as bits share its first pointer's word, and need more low bits ",
            "than the pointer leaves clear, as its pointee's alignment says, a function's none: ",
            "align the pointee more, put a pointer to data first, or derive a struct of the same ",
            "fields with `#[atom(repr = u128)]`"
        );
        refuse(&Message::new().text("`").name(type_name::<T>(), rest.len()).text(rest));
    }
}

/// Whether `T` is stored as a pointer.
const fn is_pointer<T: Atom<Repr: TupleFieldRepr>>() -> bool {
    <T::Repr as TupleFieldRepr>::IS_POINTER
}

/// The range of a tuple of `members` stored as `R`: its integer fields' bounds side by side, or,
/// beside a pointer, every repr, or every one but zero where a field's range leaves zero out.
const fn range<R: Primitive>(members: &[Member]) -> ReprRange<R> {
    if first_pointer(members).is_some() {
        let mut rest = members;
        while let [member, after @ ..] = rest {
            let (start, end) = member.reprs;
            if start != 0 && start <= end {
                return ReprRange::NON_ZERO;
            }
            rest = after;
        }
        return ReprRange::FULL;
    }
    let mut fields = [(0, 0, 0); 12];
    let mut rest = members;
    let mut out = fields.as_mut_slice();
    while let ([member, after @ ..], [field, others @ ..]) = (rest, out) {
        let (start, end) = member.reprs;
        *field = (start, end, member.bits);
        rest = after;
        out = others;
    }
    let (fields, _) = fields.split_at(members.len());
    ReprRange::of_fields(fields)
}

/// Where field `index` of `members` lies in a repr of shape `S`.
const fn placement<S: Shape>(members: &[Member], index: usize) -> PackedField {
    let seats = seats(members);
    let (Some(member), Some(seat)) = (members.get(index), seats.get(index)) else {
        return PackedField::of_pointer(0, 0);
    };
    if seat.is_pointer {
        let offset = if seat.is_first_pointer { 0 } else { usize::BITS };
        return PackedField::of_pointer(member.tag_width, offset);
    }
    let base = if S::INTEGERS_IN_POINTER { first_pointer_tag_width(members) } else { S::BASE };
    member.field.with_offset(base.wrapping_add(seat.offset))
}

/// The bit after the last field `index` of `members` governs in a repr of shape `S`: a signed
/// field's slot's end, whose bits above it copy its sign, else its place's.
const fn end<S: Shape>(members: &[Member], index: usize) -> u32 {
    let field = placement::<S>(members, index);
    match members.get(index) {
        Some(member) if field.layout().is_signed() => field.offset().wrapping_add(member.bits),
        Some(_) | None => field.next_offset(),
    }
}

/// Adds `value`'s repr to `bits`, at `seat`, where it is an integer field.
#[inline]
const fn put_integer<T: [const] Atom<Repr: [const] TupleFieldRepr>>(
    value: T, seat: Seat, bits: &mut u128,
) {
    if !seat.is_pointer {
        *bits |= value.to_repr().packed_bits().unbounded_shl(seat.offset);
    }
}

/// Places `value`'s repr in `pointers`, at `seat`, where it is a pointer field, the first one
/// taking `tags`, and adds the bits it finds set where they go to `misaligned`.
#[inline]
const fn put_pointer<T: [const] Atom<Repr: [const] TupleFieldRepr>>(
    value: T, seat: Seat, tags: Tags, pointers: &mut Pointers, misaligned: &mut usize,
) {
    if seat.is_pointer {
        let tags = if seat.is_first_pointer { tags } else { Tags::EMPTY };
        let (repr, found) = value.to_tagged_repr(tags);
        *misaligned |= found;
        if seat.is_first_pointer {
            pointers.first = repr.pointer();
        } else {
            pointers.second = repr.pointer();
        }
    }
}

/// The repr of the field at `seat` of a tuple whose pointers and integer bits are these.
///
/// # Safety
/// Where the field is a pointer, its pointer is one an `R`'s `pointer` gave.
#[expect(unsafe_code, reason = "rebuilds a pointer field's repr from what it gave")]
#[inline]
const unsafe fn repr_at<R: [const] TupleFieldRepr>(
    seat: Seat, pointers: Pointers, bits: u128,
) -> R {
    if seat.is_pointer {
        let pointer = if seat.is_first_pointer { pointers.first } else { pointers.second };
        // SAFETY: the caller's pointer is one an `R`'s `pointer` gave.
        unsafe { R::from_pointer(pointer) }
    } else {
        R::from_bits(bits.unbounded_shr(seat.offset))
    }
}

/// The repr list of the tuple `($($field),+)`: `(A::Repr, (B::Repr, ()))`.
macro_rules! reprs {
    () => { () };
    ($head:ident $($rest:ident)*) => { (<$head as Atom>::Repr, reprs!($($rest)*)) };
}

/// The validity of fields side by side: each one's, met with the next's.
macro_rules! beside {
    ($last:ident) => { <$last as Atom>::Validity };
    ($head:ident $($rest:ident)+) => {
        <<$head as Atom>::Validity as Validity>::Beside<beside!($($rest)+)>
    };
}

/// Implements `Atom`, `AtomBitwise`, a padded tuple's `FieldPatternsDecode`, each field's
/// `HasPackedField`, and `ProjectFields` for the tuple of these fields, each a type and its index,
/// the last's index named again apart.
macro_rules! tuple {
    ($(($field:ident $index:tt))+ last $last:tt) => {
        // SAFETY: each integer field's repr sits in its slot, side by side, and each pointer in a
        // word of its own, as the shape the table folds them into says; `to_repr` refuses a
        // pointer misaligned for the tags a holding word sets, and a pointee that leaves too few
        // bits for the integer fields; `from_repr` refuses integer bits above the fields and
        // decodes each field alone, by the repr alone, as `from_repr_unchecked` does unchecked,
        // each pointer field from the repr type the fold gave its place, its own; `REPRS` holds
        // each repr `to_repr` returns; the validity is the fields' met side by side, padded where
        // their bits leave the repr's top clear, or what zero alone promises beside a pointer in
        // two words; `TAG_WIDTH` and `POINTEE_ALIGNMENT` are the first pointer's, its tags beside
        // the integer fields in its low bits; and each field may cross threads, as its own impl
        // promises.
        #[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
        const unsafe impl<$($field: [const] Atom<Repr: [const] TupleFieldRepr>),+> Atom for ($($field,)+)
        where
            reprs!($($field)+): Fold<Integers<0>, Shape: const Shape>,
        {
            type Repr = <<reprs!($($field)+) as Fold<Integers<0>>>::Shape as Shape>::Repr;
            type Validity = <<reprs!($($field)+) as Fold<Integers<0>>>::Shape as Shape>::Validity<
                beside!($($field)+),
            >;
            const REPRS: ReprRange<Self::Repr> = range(&[$(Member::of::<$field>()),+]);
            const TAG_WIDTH: u32 = {
                let base = first_pointer_tag_width(&[$(Member::of::<$field>()),+]);
                if <<reprs!($($field)+) as Fold<Integers<0>>>::Shape as Shape>::INTEGERS_IN_POINTER {
                    base.saturating_add(
                        <<reprs!($($field)+) as Fold<Integers<0>>>::Shape as Shape>::BITS,
                    )
                } else {
                    base
                }
            };
            const POINTEE_ALIGNMENT: PointeeAlignment = first_pointee_alignment(
                &[$((is_pointer::<$field>(), $field::POINTEE_ALIGNMENT)),+],
            );
            #[inline]
            #[track_caller]
            fn to_repr(self) -> Self::Repr {
                let (repr, misaligned) = self.to_tagged_repr(Tags::EMPTY);
                assert_aligned::<Self>(misaligned);
                repr
            }
            #[expect(unsafe_code, reason = "joins the pointers the fields gave")]
            #[inline]
            fn to_tagged_repr(self, tags: Tags) -> (Self::Repr, usize) {
                type S<$($field),+> = <reprs!($($field)+) as Fold<Integers<0>>>::Shape;
                const { assert_tags_fit::<Self>(Self::TAG_WIDTH, Self::POINTEE_ALIGNMENT) };
                let seats = const { seats(&[$(Member::of::<$field>()),+]) };
                let base = const { first_pointer_tag_width(&[$(Member::of::<$field>()),+]) };
                let (mut pointers, mut bits, mut misaligned) = (Pointers::NONE, 0, 0);
                $(put_integer(self.$index, seats[$index], &mut bits);)+
                let outer = <S<$($field),+> as Shape>::pointer_tags(bits, base, tags);
                $(put_pointer(self.$index, seats[$index], outer, &mut pointers, &mut misaligned);)+
                // SAFETY: each pointer is one the field the fold seats there gave, of the repr
                // type the shape holds in that place.
                let repr = unsafe { <S<$($field),+> as Shape>::join(pointers, bits) };
                if <Self::Repr as Primitive>::IS_BITS_EXACT {
                    // A tuple of bits alone takes a word's tags as any value of bits does.
                    let (repr, found) = tags.set_in(repr);
                    (repr, misaligned | found)
                } else {
                    (repr, misaligned)
                }
            }
            #[expect(unsafe_code, reason = "rebuilds each pointer field's repr from what it gave")]
            #[inline]
            fn from_repr(repr: Self::Repr) -> Option<Self> {
                type S<$($field),+> = <reprs!($($field)+) as Fold<Integers<0>>>::Shape;
                const { assert_tags_fit::<Self>(Self::TAG_WIDTH, Self::POINTEE_ALIGNMENT) };
                let seats = const { seats(&[$(Member::of::<$field>()),+]) };
                let base = const { first_pointer_tag_width(&[$(Member::of::<$field>()),+]) };
                let (pointers, bits) = <S<$($field),+> as Shape>::split(repr, base);
                if bits.unbounded_shr(<S<$($field),+> as Shape>::BITS) != 0 {
                    return None;
                }
                Some(($(
                    // SAFETY: a pointer field's pointer is one `split` gave, of the repr type the
                    // fold seats there, its own.
                    match $field::from_repr(unsafe { repr_at(seats[$index], pointers, bits) }) {
                        Some(value) => value,
                        None => return None,
                    },
                )+))
            }
            #[expect(unsafe_code, reason = "decodes each field of a repr that decodes")]
            #[inline]
            unsafe fn from_repr_unchecked(repr: Self::Repr) -> Self {
                type S<$($field),+> = <reprs!($($field)+) as Fold<Integers<0>>>::Shape;
                const { assert_tags_fit::<Self>(Self::TAG_WIDTH, Self::POINTEE_ALIGNMENT) };
                let seats = const { seats(&[$(Member::of::<$field>()),+]) };
                let base = const { first_pointer_tag_width(&[$(Member::of::<$field>()),+]) };
                let (pointers, bits) = <S<$($field),+> as Shape>::split(repr, base);
                ($({
                    // SAFETY: a pointer field's pointer is one `split` gave, of the repr type the
                    // fold seats there, its own.
                    let field = unsafe { repr_at(seats[$index], pointers, bits) };
                    // SAFETY: the caller's repr decodes, so each field's does, as `from_repr`
                    // reads them.
                    unsafe { $field::from_repr_unchecked(field) }
                },)+)
            }
        }

        /// And, or, xor and not combine each field with its own, where every field has them and the
        /// fields fill the repr.
        // `do_not_recommend`, so a tuple without them reports `AtomBitwise`'s message.
        #[diagnostic::do_not_recommend]
        impl<$($field: AtomBitwise<Repr: TupleFieldRepr>),+> AtomBitwise for ($($field,)+)
        where
            Self: Atom<Validity = Total, Repr: Bitwise>,
        {
        }

        // A tuple of `AtomBitwise` fields that leave bits of their repr above them is no
        // `AtomBitwise`, but its field is the fields' bits alone.
        //
        // SAFETY: the tuple's range lies within its slots, as `of_fields` builds it, so a field of
        // it takes the slots' bits or fewer, and each pattern of them leaves every slot a repr of
        // its field, each of which decodes, as `AtomBitwise`'s `Total` says, and the bits above the
        // slots clear. The validity of a tuple of `AtomBitwise` fields is `ZeroValid` only where
        // they leave bits above.
        #[expect(unsafe_code, reason = "a padded tuple's field takes every pattern of its fields")]
        unsafe impl<$($field: AtomBitwise<Repr: TupleFieldRepr>),+> FieldPatternsDecode<($($field,)+)>
            for ZeroValid
        where
            ($($field,)+): Atom<Validity = ZeroValid, Repr: ExactBits>,
        {
        }

        tuple!(@fields [$($field)+] $(($field $index))+ last $last);

        // `do_not_recommend`, so a value without fields is not told to wrap itself in a tuple.
        //
        // SAFETY: the projection lends the place of each field, at its path, each as visible as a
        // tuple's fields, which are public, and no other.
        #[diagnostic::do_not_recommend]
        #[expect(unsafe_code, reason = "the projection lends each field's place")]
        const unsafe impl<$($field: [const] Atom<Repr: [const] TupleFieldRepr>),+> ProjectFields
            for ($($field,)+)
        where
            reprs!($($field)+): Fold<Integers<0>, Shape: const Shape>,
        {
            type Fields<'a, P: FieldPath<Value = Self> + 'a>
                = ($(&'a AtomicField<Join<P, Field<Self, $index, $field>>>,)+)
            where
                Self: 'a;
            #[expect(unsafe_code, reason = "lends each field's place")]
            #[inline]
            fn project<P: FieldPath<Value = Self>>(
                place: &AtomicField<P>,
            ) -> ($(&AtomicField<Join<P, Field<Self, $index, $field>>>,)+) {
                const { assert_tags_fit::<Self>(Self::TAG_WIDTH, Self::POINTEE_ALIGNMENT) };
                // SAFETY: a tuple's fields are as visible as the tuple.
                ($(unsafe { project_field::<P, $field, $index>(place) },)+)
            }
        }
    };
    (@fields $all:tt $(($field:ident $index:tt))+ last $last:tt) => {
        $(tuple!(@field $all $field $index $last);)+
    };
    (@field [$($all:ident)+] $field:ident $index:tt $last:tt) => {
        // SAFETY: `PLACEMENT` is the field's range in its slot, where `to_repr` packs it, every
        // other field in bits of its own; `END` its slot's end where it is signed, whose bits
        // copy its sign, else its range's; a pointer's, the low bits of its word that hold its own
        // tags; `Reach` reaches the top only for the last field, of integers that fill the repr,
        // where every repr of its slot decodes (`Total`), so a carry out of its range into its
        // slot's top bits leaves one that does, and out of its slot leaves the word; `field`
        // returns the field.
        #[expect(unsafe_code, reason = "`FieldPath` for `Field` trusts the placement")]
        const unsafe impl<$($all: [const] Atom<Repr: [const] TupleFieldRepr>),+>
            HasPackedField<$index, $field> for ($($all,)+)
        where
            Self: [const] Atom,
            reprs!($($all)+): Fold<Integers<0>, Shape: const Shape>,
        {
            const PLACEMENT: PackedField = placement::<
                <reprs!($($all)+) as Fold<Integers<0>>>::Shape,
            >(&[$(Member::of::<$all>()),+], $index);
            const LAYOUT: PackedLayout = PackedLayout::new(&[]);
            const END: u32 = end::<
                <reprs!($($all)+) as Fold<Integers<0>>>::Shape,
            >(&[$(Member::of::<$all>()),+], $index);
            type Reach = <LastField<{ $index == $last }> as ReachOf<
                <<reprs!($($all)+) as Fold<Integers<0>>>::Shape as Shape>::TopReach,
                <$field as Atom>::Validity,
            >>::Reach;
            #[inline]
            fn field(self) -> $field {
                self.$index
            }
        }
    };
}

/// Whether a field is the last, as a type.
#[doc(hidden)]
#[derive(Debug)]
pub struct LastField<const LAST: bool>;

/// A field's `Reach`: [`LastReach`] for the last field, `Reach<false>` for any other.
#[doc(hidden)]
pub impl(crate) trait ReachOf<R, V> {
    /// The reach.
    type Reach;
}

impl<R, V> ReachOf<R, V> for LastField<true> {
    type Reach = LastReach<R, V>;
}

impl<R, V> ReachOf<R, V> for LastField<false> {
    type Reach = Reach<false>;
}

/// The reach of a tuple's last field, of validity `V`, after fields whose shape's reach is `R`: the
/// top only where `R` is, the integer fields filling the repr, and `V` is `Total`, as an array's
/// last element asks `AtomAdd`. A field's place is as wide as its range, which may leave its
/// slot's top bits above it, so a carry out of the place may stay in the slot, every repr of
/// which must decode.
#[doc(hidden)]
#[derive(Debug)]
pub struct LastReach<R, V>(PhantomData<fn() -> (R, V)>);

impl ReachesTop for LastReach<Reach<true>, Total> {}

tuple!((A 0) last 0);
tuple!((A 0) (B 1) last 1);
tuple!((A 0) (B 1) (C 2) last 2);
tuple!((A 0) (B 1) (C 2) (D 3) last 3);
tuple!((A 0) (B 1) (C 2) (D 3) (E 4) last 4);
tuple!((A 0) (B 1) (C 2) (D 3) (E 4) (F 5) last 5);
tuple!((A 0) (B 1) (C 2) (D 3) (E 4) (F 5) (G 6) last 6);
tuple!((A 0) (B 1) (C 2) (D 3) (E 4) (F 5) (G 6) (H 7) last 7);
tuple!((A 0) (B 1) (C 2) (D 3) (E 4) (F 5) (G 6) (H 7) (I 8) last 8);
tuple!((A 0) (B 1) (C 2) (D 3) (E 4) (F 5) (G 6) (H 7) (I 8) (J 9) last 9);
tuple!((A 0) (B 1) (C 2) (D 3) (E 4) (F 5) (G 6) (H 7) (I 8) (J 9) (K 10) last 10);
tuple!((A 0) (B 1) (C 2) (D 3) (E 4) (F 5) (G 6) (H 7) (I 8) (J 9) (K 10) (L 11) last 11);
