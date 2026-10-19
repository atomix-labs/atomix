//! [`Packed`]: an array packed at its elements' width in an integer the user states, as a packed
//! struct packs its fields.

use core::any::type_name;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

#[cfg(feature = "zerocopy-08")]
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

use super::Atom;
use crate::atomic::{Array, AtomicField, FieldPath, Indexed};
use crate::message::{Message, refuse};
use crate::primitive::{CompareExchange, ExactBits, Primitive};
use crate::range::{PackedField, PackedLayout, ReprRange};
use crate::validity::Validity;

/// An array packed at its elements' width in the integer `R`, as a packed struct packs its
/// fields: element `i` at `i` times the bits its type's range needs, element 0 lowest.
///
/// Where `[T; N]` gives each element its repr's width, a byte for a two-variant enum, `Packed`
/// gives it the bits its values need, one for that enum, so more fit a word: sixteen `RangedU8<0,
/// 3>` in a `u32`, where `[T; N]` needs a `u128`. The integer is stated, as a generic struct's
/// `#[atom(repr = …)]` is, since a type chosen by the elements' width would need
/// `generic_const_exprs`; elements too wide for it fail the build.
///
/// It is the array otherwise: it dereferences to it, so `packed[i]` and `packed.iter()` read it,
/// and it compares, orders, hashes and prints as the array does. [`Packed::into_inner`] gives the
/// array back. An atomic of it lends each element as a place, as an atomic array does, but adds to
/// none in place: whether its elements fill `R` is a constant, not a type.
///
/// With atomix's features, it has the traits its array has of serde (`serde`), zerocopy
/// (`zerocopy-08`) and arbitrary (`arbitrary`), and bytemuck's `Zeroable`, `NoUninit` and
/// `AnyBitPattern` (`bytemuck`), each as the array's.
///
/// # Examples
/// ```
/// # extern crate atomix_core as atomix;
///
/// use atomix::ordering::{AcqRel, Acquire};
/// use atomix::{Atomic, Packed, RangedU8};
///
/// /// How full each of sixteen queues is, in quarters: two bits each, one `u32` for all.
/// static FILL: Atomic<Packed<[RangedU8<0, 3>; 16], u32>> =
///     Atomic::new(Packed::new([RangedU8::MIN; 16]));
///
/// let three_quarters = RangedU8::new(3).expect("3 lies in 0 to 3");
/// FILL.element(9).update(AcqRel, Acquire, |_| three_quarters);
/// assert_eq!(FILL.load(Acquire)[9], three_quarters, "queue 9, in its two bits");
/// assert_eq!(size_of_val(&FILL), 4, "sixteen queues in a word");
/// ```
#[repr(transparent)]
#[cfg_attr(
    feature = "zerocopy-08",
    derive(FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)
)]
pub struct Packed<A, R> {
    /// The array.
    array: A,
    /// The integer it is packed in.
    marker: PhantomData<fn() -> R>,
}

impl<A, R> Packed<A, R> {
    /// `array`, to be packed in `R`.
    #[inline]
    #[must_use]
    pub const fn new(array: A) -> Self {
        Self { array, marker: PhantomData }
    }

    /// The array `packed` holds: an associated function, as `ManuallyDrop::into_inner` is, since
    /// `Packed` dereferences to the array, whose own methods a method would shadow.
    #[inline]
    #[must_use]
    pub const fn into_inner(packed: Self) -> A {
        packed.array
    }
}

const impl<A, R> Deref for Packed<A, R> {
    type Target = A;
    #[inline]
    fn deref(&self) -> &A {
        &self.array
    }
}

const impl<A, R> DerefMut for Packed<A, R> {
    #[inline]
    fn deref_mut(&mut self) -> &mut A {
        &mut self.array
    }
}

const impl<A, R> From<A> for Packed<A, R> {
    #[inline]
    fn from(array: A) -> Self {
        Self::new(array)
    }
}

// Each of the traits below is the array's, written out so that `R`, a marker, needs none.

const impl<A: [const] Clone, R> Clone for Packed<A, R> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new(self.array.clone())
    }
}

impl<A: Copy, R> Copy for Packed<A, R> {}

const impl<A: [const] Default, R> Default for Packed<A, R> {
    #[inline]
    fn default() -> Self {
        Self::new(A::default())
    }
}

const impl<A: [const] PartialEq, R> PartialEq for Packed<A, R> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.array == other.array
    }
}

const impl<A: [const] Eq, R> Eq for Packed<A, R> {}

const impl<A: [const] PartialOrd, R> PartialOrd for Packed<A, R> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.array.partial_cmp(&other.array)
    }
}

const impl<A: [const] Ord, R> Ord for Packed<A, R> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.array.cmp(&other.array)
    }
}

impl<A: Hash, R> Hash for Packed<A, R> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.array.hash(state);
    }
}

/// Prints the array, as `Wrapping` prints its value.
impl<A: fmt::Debug, R> fmt::Debug for Packed<A, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.array.fmt(f)
    }
}

/// How `Packed` lays out `N` elements of `T` in `R`: each a field of `T`'s range, side by side
/// from bit 0, and the bits above the last extending it; and how many bits each takes.
///
/// # Panics
/// Where there are no elements, or they take more bits than `R` has: in a constant, which each
/// caller is, the build fails instead.
#[track_caller]
const fn packed_layout<T: Atom, const N: usize, R: Primitive>() -> (PackedLayout, u32) {
    if N == 0 {
        let rest = "`: it packs no elements, and a `Packed` packs one at least";
        refuse(
            &Message::new().text("`").name(type_name::<Packed<[T; N], R>>(), rest.len()).text(rest),
        );
    }
    let width = PackedField::new(T::REPRS, 0).layout().width();
    // Past `u32::MAX` elements the offset saturates, so the test below refuses elements of any
    // bits.
    let last = N.saturating_sub(1).saturating_cast::<u32>();
    let top = PackedField::new(T::REPRS, width.saturating_mul(last));
    // A packed value's layout is its top field's, the fields below it side by side.
    let layout = PackedLayout::new(&[top]);
    if layout.width() > R::BITS {
        let advice = "`: state a wider integer, or pack fewer";
        // What the refusal writes between the type's name and the advice: its words, three
        // counts' digits and `R`'s name, a primitive's, room enough for each.
        let between = 96;
        let message = Message::new()
            .text("`")
            .name(type_name::<Packed<[T; N], R>>(), advice.len().saturating_add(between))
            .text("`: its ")
            .number(N.wrapping_cast())
            .text(" elements take ")
            .number(layout.width().into())
            .text(" bits, past the ")
            .number(R::BITS.into())
            .text(" of `")
            .name(type_name::<R>(), advice.len())
            .text(advice);
        refuse(&message);
    }
    (layout, width)
}

/// The field of element `index` of a `Packed` whose elements, of `T`, take `width` bits each.
#[inline]
const fn element_field<T: Atom>(index: usize, width: u32) -> PackedField {
    PackedField::new(T::REPRS, width.wrapping_mul(index.wrapping_cast()))
}

// SAFETY: as a packed struct of `N` fields of `T`, side by side from bit 0, its bits above them
// extending the last one: `to_repr` packs each element in its field, so the repr lies in the
// layout's range, and decodes as the array; `from_repr` refuses bits above that do not extend the
// layout, and decodes each field alone, by the repr alone, as `from_repr_unchecked` does unchecked;
// the validity is a padded array's of `T`'s, as the bits above the fields extend the top one, and
// zero, where `T`'s zero decodes, extends it; `packed_layout` refuses an array of no elements, and
// elements wider than `R`, each at compile time, so every element's field lies within `R`; and each
// element may cross threads, as its own impl promises.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl<T, const N: usize, R> Atom for Packed<[T; N], R>
where
    T: [const] Atom<Repr: const ExactBits>,
    R: const ExactBits + CompareExchange,
{
    type Repr = R;
    type Validity = <T::Validity as Validity>::PaddedElements;
    const REPRS: ReprRange<R> = packed_layout::<T, N, R>().0.range();
    #[inline]
    fn to_repr(self) -> R {
        let (layout, width) = const { packed_layout::<T, N, R>() };
        let mut bits = 0;
        let mut index = 0;
        let mut rest = self.array.as_slice();
        while let [element, after @ ..] = rest {
            bits |= element_field::<T>(index, width).pack(element.to_repr().to_bits());
            index = index.wrapping_add(1);
            rest = after;
        }
        layout.repr(bits)
    }
    #[inline]
    fn from_repr(repr: R) -> Option<Self> {
        let (layout, width) = const { packed_layout::<T, N, R>() };
        let Some(bits) = layout.canonical_bits(repr) else {
            return None;
        };
        let Some(first) =
            T::from_repr(Primitive::from_bits(element_field::<T>(0, width).unpack(bits)))
        else {
            return None;
        };
        let mut array = [first; N];
        let mut index = 0;
        let mut rest = array.as_mut_slice();
        while let [element, after @ ..] = rest {
            let field = element_field::<T>(index, width).unpack(bits);
            let Some(decoded) = T::from_repr(Primitive::from_bits(field)) else {
                return None;
            };
            *element = decoded;
            index = index.wrapping_add(1);
            rest = after;
        }
        Some(Self::new(array))
    }
    #[expect(unsafe_code, reason = "decodes each field of a repr that decodes")]
    #[inline]
    unsafe fn from_repr_unchecked(repr: R) -> Self {
        let width = const { packed_layout::<T, N, R>().1 };
        let bits = repr.to_bits();
        let first = Primitive::from_bits(element_field::<T>(0, width).unpack(bits));
        // SAFETY: the caller's repr decodes, so each field does, as its element.
        let mut array = [unsafe { T::from_repr_unchecked(first) }; N];
        let mut index = 0;
        let mut rest = array.as_mut_slice();
        while let [element, after @ ..] = rest {
            let field = Primitive::from_bits(element_field::<T>(index, width).unpack(bits));
            // SAFETY: as above.
            *element = unsafe { T::from_repr_unchecked(field) };
            index = index.wrapping_add(1);
            rest = after;
        }
        Self::new(array)
    }
}

const impl<T, const N: usize, R> Array for Packed<[T; N], R>
where
    T: [const] Atom<Repr: const ExactBits>,
    R: const ExactBits + CompareExchange,
{
    type Element = T;
    type Elements<'a, C: Atom + 'a>
        = [&'a AtomicField<Indexed<C, T>>; N]
    where
        Self: 'a;
    const LENGTH: usize = N;
    const STRIDE: u32 = packed_layout::<T, N, R>().1;
    const EXTENDS_LAST: bool = {
        let (layout, width) = packed_layout::<T, N, R>();
        layout.is_extended_by(element_field::<T>(N.wrapping_sub(1), width), R::BITS)
    };
    #[expect(clippy::indexing_slicing, reason = "the caller's index lies below `N`")]
    #[inline]
    fn element(self, index: usize) -> T {
        self.array[index]
    }
    #[inline]
    fn project<P: FieldPath<Value = Self>>(
        place: &AtomicField<P>,
    ) -> [&AtomicField<Indexed<P::Container, T>>; N] {
        place.each_element()
    }
}
