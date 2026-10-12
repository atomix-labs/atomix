//! An atomic array's elements as places: [`Element`] and [`Last`], the paths to one whose index is
//! a constant, and [`Indexed`]'s, the place of one whose index comes at run time.

use core::any::type_name;
use core::fmt;
use core::hint::assert_unchecked;
use core::marker::PhantomData;

use super::path::ReachesTop;
use super::{
    AtomicField, FieldPath, FieldPlacement, Indexed, Join, PlaceTail, Then, TopField, Whole,
};
use crate::atom::{Atom, AtomAdd, Slots};
use crate::atomic::Atomic;
use crate::message::{Message, refuse};
use crate::primitive::{ExactBits, Primitive};
use crate::range::PackedField;

/// An array an atomic holds, whose elements are stored as bits: its element, its length, where
/// each element lies, and its projection onto its elements' places.
#[doc(hidden)]
pub impl(crate) const trait Array: [const] Atom {
    /// The type of each element.
    type Element: [const] Atom<Repr: const ExactBits>;
    /// The place of each element of the array in an atomic of `C`: an array of places of
    /// [`Indexed`] paths.
    type Elements<'a, C: Atom + 'a>
    where
        Self: 'a;
    /// How many elements it has, at least one.
    const LENGTH: usize;
    /// How many bits lie from one element's lowest bit to the next's.
    const STRIDE: u32;
    /// Whether the repr's bits above the elements copy the last one's sign, as a packed value's
    /// above its top field do: an array's slots leave the bits above them clear.
    const EXTENDS_LAST: bool;
    /// The element at `index`, below `LENGTH`.
    fn element(self, index: usize) -> Self::Element;
    /// The place of each element of the array at `place`, and no other place: never `place`'s
    /// own, which may be [`Whole`]'s.
    fn project<P: FieldPath<Value = Self>>(
        place: &AtomicField<P>,
    ) -> Self::Elements<'_, P::Container>;
}

const impl<T: [const] Atom<Repr: const ExactBits>, const N: usize> Array for [T; N]
where
    T::Repr: const Slots<N>,
{
    type Element = T;
    type Elements<'a, C: Atom + 'a>
        = [&'a AtomicField<Indexed<C, T>>; N]
    where
        Self: 'a;
    const LENGTH: usize = N;
    const STRIDE: u32 = T::Repr::BITS;
    const EXTENDS_LAST: bool = false;
    #[expect(clippy::indexing_slicing, reason = "the caller's index lies below `N`")]
    #[inline]
    fn element(self, index: usize) -> T {
        self[index]
    }
    #[inline]
    fn project<P: FieldPath<Value = Self>>(
        place: &AtomicField<P>,
    ) -> [&AtomicField<Indexed<P::Container, T>>; N] {
        place.each_element()
    }
}

impl FieldPlacement {
    /// The bits of element `index`, below `A::LENGTH`, of the array `A` whose bits these are: as
    /// [`then`](Self::then) joins a field of the array's own at the element's slot.
    #[inline]
    const fn element<A: Array>(self, index: usize) -> Self {
        let layout = const { PackedField::new(<A::Element as Atom>::REPRS, 0).layout() };
        let width = layout.width();
        // An element of no bits, as `()` is, lies at its array's own offset, as a packed value's
        // field of no bits lies at its running offset: an offset past it may lie past the repr.
        let offset =
            if width == 0 { 0 } else { index.wrapping_cast::<u32>().wrapping_mul(A::STRIDE) };
        // A signed element governs its whole slot, whose bits above it copy its sign, and the last
        // one the repr's bits above the elements, where they copy it too.
        let end = if !layout.is_signed() {
            offset.wrapping_add(width)
        } else if A::EXTENDS_LAST && index.wrapping_add(1) == A::LENGTH {
            <A::Repr as Primitive>::BITS
        } else {
            offset.wrapping_add(A::STRIDE)
        };
        let slot = Self {
            offset,
            width,
            signed: layout.is_signed(),
            end,
            container_width: <A::Repr as Primitive>::BITS,
        };
        self.then(slot)
    }
}

/// The element `INDEX` of the array `A`, as a path: what [`Atomic::place`] lends a place of.
///
/// # Examples
/// ```
/// # extern crate atomix_core as atomix;
///
/// use atomix::ordering::{Acquire, Relaxed};
/// use atomix::{Atomic, AtomicField, Element};
///
/// /// The workers' states, one byte each.
/// static STATES: Atomic<[u8; 4]> = Atomic::new([0; 4]);
///
/// let third: &AtomicField<Element<[u8; 4], 2>> = STATES.place::<2>();
/// third.or(0b10, Relaxed);
/// assert_eq!(STATES.load(Acquire), [0, 0, 0b10, 0], "the third byte alone");
/// ```
// As `Field`'s.
#[derive(Clone, Copy)]
pub struct Element<A, const INDEX: usize>(PhantomData<fn(A) -> A>);

impl<A, const INDEX: usize> fmt::Debug for Element<A, INDEX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Element<{}, {INDEX}>", type_name::<A>())
    }
}

/// Refuses `index` where it lies past an array of `length` elements, as indexing an array does:
/// at compile time, for a constant index.
#[inline]
#[track_caller]
const fn assert_in_bounds(index: usize, length: usize) {
    if index >= length {
        refuse_index(index, length);
    }
}

/// Refuses `index`, past the end of an array of `length` elements, as indexing an array does.
#[cold]
#[inline(never)]
#[track_caller]
const fn refuse_index(index: usize, length: usize) -> ! {
    let message =
        Message::new().text("index out of bounds: the len is ").number(length.wrapping_cast());
    refuse(&message.text(" but the index is ").number(index.wrapping_cast()))
}

impl<A: Array, const INDEX: usize> Element<A, INDEX> {
    /// Where the element's bits lie in the array's repr.
    const PLACEMENT: FieldPlacement = {
        assert_in_bounds(INDEX, A::LENGTH);
        FieldPlacement::of::<Whole<A>>().element::<A>(INDEX)
    };
}

// The element's slot holds its repr, as `to_repr` packs it, and `from_repr` decodes each slot
// alone, checking only the bits above the slots: clear, which no element governs, or, where
// `EXTENDS_LAST` says, copies of the last one's sign, which it governs and each write extends. Any
// repr of the element that decodes, written in its bits so, leaves an array that decodes, with
// that element alone changed.
const impl<A: [const] Array, const INDEX: usize> FieldPath for Element<A, INDEX> {
    type Container = A;
    type Value = A::Element;
    const OFFSET: u32 = Self::PLACEMENT.offset;
    const WIDTH: u32 = Self::PLACEMENT.width;
    const IS_SIGNED: bool = Self::PLACEMENT.signed;
    const END: u32 = Self::PLACEMENT.end;
    type Tail = ();
    type Join<B: FieldPath<Container = Self::Value, Tail = ()>> = Then<Self, B>;
    #[inline]
    fn field(container: A) -> A::Element {
        container.element(INDEX)
    }
}

/// The last element of the array `A`, as a path: what [`Atomic::last`] lends a place of.
///
/// Where the slots fill the repr and the element adds, it ends at the repr's top bit, a
/// [`TopField`], whose `fetch_add` carries out of the word.
// As `Field`'s.
#[derive(Clone, Copy)]
pub struct Last<A>(PhantomData<fn(A) -> A>);

impl<A> fmt::Debug for Last<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Last<{}>", type_name::<A>())
    }
}

impl<A: Array> Last<A> {
    /// Where the last element's bits lie in the array's repr.
    const PLACEMENT: FieldPlacement =
        FieldPlacement::of::<Whole<A>>().element::<A>(A::LENGTH.wrapping_sub(1));
}

// As `Element`'s, at the last index.
const impl<A: [const] Array> FieldPath for Last<A> {
    type Container = A;
    type Value = A::Element;
    const OFFSET: u32 = Self::PLACEMENT.offset;
    const WIDTH: u32 = Self::PLACEMENT.width;
    const IS_SIGNED: bool = Self::PLACEMENT.signed;
    const END: u32 = Self::PLACEMENT.end;
    type Tail = ();
    type Join<B: FieldPath<Container = Self::Value, Tail = ()>> = Then<Self, B>;
    #[inline]
    fn field(container: A) -> A::Element {
        container.element(A::LENGTH.wrapping_sub(1))
    }
}

// The last slot ends at the repr's top bit where the slots fill it, and an element whose every repr
// decodes, as `AtomAdd` says, fills its slot.
#[diagnostic::do_not_recommend]
impl<T: AtomAdd<Repr: const ExactBits>, const N: usize> TopField for Last<[T; N]> where
    T::Repr: const Slots<N, TopReach: ReachesTop>
{
}

impl<A: Array> Atomic<A> {
    /// Element `index` of the array, a place of its own, as `array[index]` names one:
    /// `busy.element(worker).test_and_set(AcqRel)`.
    ///
    /// It is a field's place, at an [`Indexed`] path, whose bits come at run time: it has each
    /// operation a field of its value has but the add, whose carry would reach the next element,
    /// each one instruction on the whole word: an element of an array of `bool`s has `set` and
    /// `test_and_set`, one of bytes `or` and `and`. A packed value's has its fields' places, and an
    /// array's its elements'. With a constant index, [`place`](Self::place) lends the same element
    /// as a place its path's constants place; with every index, [`elements`](Self::elements) lends
    /// them all.
    ///
    /// # Panics
    /// Where `index` lies past the array's end, as indexing an array does.
    ///
    /// # Examples
    /// ```
    /// # extern crate atomix_core as atomix;
    ///
    /// use atomix::Atomic;
    /// use atomix::ordering::{AcqRel, Acquire};
    ///
    /// /// Which of 64 workers are busy, a bit each.
    /// static BUSY: Atomic<[bool; 64]> = Atomic::new([false; 64]);
    ///
    /// let worker = 17;
    /// assert!(!BUSY.element(worker).test_and_set(AcqRel), "claimed once");
    /// assert!(BUSY.element(worker).test_and_set(AcqRel), "and not again");
    /// assert!(BUSY.load(Acquire)[worker], "the one bit set");
    /// ```
    #[inline]
    #[must_use]
    #[track_caller]
    #[expect(unsafe_code, reason = "places an element of the array the repr holds")]
    pub const fn element(&self, index: usize) -> &AtomicField<Indexed<A, A::Element>> {
        // SAFETY: the whole repr's bits are an `A`'s.
        unsafe { indexed_element::<A, A>(self, FieldPlacement::of::<Whole<A>>(), index) }
    }

    /// Element `index` of the array, or `None` past its end, as [`slice::get`] returns.
    ///
    /// To test a `bool` element's bit, as [`element`](Self::element)'s example does, write
    /// `is_some_and` or `if let`.
    #[inline]
    #[must_use]
    #[expect(unsafe_code, reason = "places an element of the array the repr holds")]
    pub const fn element_checked(
        &self, index: usize,
    ) -> Option<&AtomicField<Indexed<A, A::Element>>> {
        // SAFETY: as in `element`.
        unsafe { indexed_element_checked::<A, A>(self, FieldPlacement::of::<Whole<A>>(), index) }
    }

    /// Element `index` of the array, without the check of `index`, as
    /// [`slice::get_unchecked`] returns.
    ///
    /// # Safety
    /// `index` lies below the array's length. Where the standard library's checks of undefined
    /// behaviour run, as in a debug build, an index past it aborts the process.
    #[expect(unsafe_code, reason = "forwards the caller's promise")]
    #[inline]
    #[must_use]
    #[track_caller]
    pub const unsafe fn element_unchecked(
        &self, index: usize,
    ) -> &AtomicField<Indexed<A, A::Element>> {
        // SAFETY: the caller's index lies within the array, and the whole repr's bits are an `A`'s.
        unsafe { indexed_element_unchecked::<A, A>(self, FieldPlacement::of::<Whole<A>>(), index) }
    }

    /// Element `INDEX` of the array, as the place of a field: every operation a field of its value
    /// has, and [`fields`](AtomicField::fields) where it is a packed value.
    ///
    /// An index past the array's end fails the build.
    #[expect(unsafe_code, reason = "lends an element's place, which every element's code may hold")]
    #[inline]
    #[must_use]
    pub const fn place<const INDEX: usize>(&self) -> &AtomicField<Element<A, INDEX>> {
        const { assert_in_bounds(INDEX, A::LENGTH) };
        // SAFETY: an array's elements are visible wherever the array is, and the path ends at an
        // `Element`, not at `Whole`.
        unsafe { AtomicField::from_atomic(self) }
    }

    /// The last element of the array, as the place of a field: where an array's slots fill the repr
    /// and the element has [`AtomAdd`], its [`fetch_add`](AtomicField::fetch_add) is one
    /// instruction, its carry leaving the word; a [`Packed`](crate::Packed)'s never adds in place.
    ///
    /// # Examples
    /// ```
    /// # extern crate atomix_core as atomix;
    ///
    /// use atomix::Atomic;
    /// use atomix::ordering::{AcqRel, Acquire};
    ///
    /// /// A phase, in the low half, and how many readers it has, in the high one.
    /// static PHASE: Atomic<[u16; 2]> = Atomic::new([3, 0]);
    ///
    /// let before = PHASE.last().fetch_add(1, AcqRel);
    /// assert_eq!(before, [3, 0], "the array before");
    /// assert_eq!(PHASE.load(Acquire), [3, 1], "a reader counted");
    /// ```
    #[expect(unsafe_code, reason = "lends an element's place, which every element's code may hold")]
    #[inline]
    #[must_use]
    pub const fn last(&self) -> &AtomicField<Last<A>> {
        // SAFETY: an array's elements are visible wherever the array is, and the path ends at
        // `Last`, not at `Whole`.
        unsafe { AtomicField::from_atomic(self) }
    }

    /// Each element of the array, a place of its own, to destructure or iterate:
    /// `let [first, second] = pair.elements();`.
    ///
    /// It builds every place, so [`element`](Self::element), not `elements()[index]`, lends one at
    /// an index known at run time alone.
    #[expect(unsafe_code, reason = "lends the place of the whole array to its projection")]
    #[inline]
    #[must_use]
    pub const fn elements(&self) -> A::Elements<'_, A>
    where
        A: [const] Array,
    {
        // SAFETY: the place of the whole value goes to the array's projection alone, which lends
        // only its elements' places, as `Array::project` says.
        A::project(unsafe { AtomicField::<Whole<A>>::from_atomic(self) })
    }
}

/// The path to an element of the array at the path `P`, at an index known at run time.
type IndexedElement<P> =
    Indexed<<P as FieldPath>::Container, <<P as FieldPath>::Value as Array>::Element>;

impl<P: FieldPath<Value: Array>> AtomicField<P> {
    /// Element `index` of the array in this field, a place of its own, as [`Atomic::element`]:
    /// `grid.element(row).element(column)`.
    ///
    /// # Panics
    /// Where `index` lies past the array's end.
    #[expect(unsafe_code, reason = "places an element of the array the field holds")]
    #[inline]
    #[must_use]
    #[track_caller]
    pub const fn element(&self, index: usize) -> &AtomicField<IndexedElement<P>> {
        // SAFETY: the place's bits are its value's, an array's, where its path's constants or its
        // tail place them.
        unsafe { indexed_element::<P::Container, P::Value>(&self.atomic, self.placement(), index) }
    }

    /// Element `index` of the array in this field, or `None` past its end, as
    /// [`Atomic::element_checked`].
    #[expect(unsafe_code, reason = "places an element of the array the field holds")]
    #[inline]
    #[must_use]
    pub const fn element_checked(&self, index: usize) -> Option<&AtomicField<IndexedElement<P>>> {
        let array = self.placement();
        // SAFETY: as in `element`.
        unsafe { indexed_element_checked::<P::Container, P::Value>(&self.atomic, array, index) }
    }

    /// Element `index` of the array in this field, without the check of `index`, as
    /// [`Atomic::element_unchecked`].
    ///
    /// # Safety
    /// `index` lies below the array's length.
    #[expect(unsafe_code, reason = "forwards the caller's promise")]
    #[inline]
    #[must_use]
    #[track_caller]
    pub const unsafe fn element_unchecked(&self, index: usize) -> &AtomicField<IndexedElement<P>> {
        let array = self.placement();
        // SAFETY: the caller's index lies within the array, and the place's bits are its value's,
        // as in `element`.
        unsafe { indexed_element_unchecked::<P::Container, P::Value>(&self.atomic, array, index) }
    }

    /// Element `INDEX` of the array in this field, as the place of a field, as
    /// [`Atomic::place`].
    #[expect(unsafe_code, reason = "lends an element's place, which every element's code may hold")]
    #[inline]
    #[must_use]
    pub const fn place<const INDEX: usize>(
        &self,
    ) -> &AtomicField<Join<P, Element<P::Value, INDEX>>> {
        const { assert_in_bounds(INDEX, <P::Value as Array>::LENGTH) };
        // SAFETY: an array's elements are visible wherever the array is.
        unsafe { <P::Tail as PlaceTail>::join::<P, Element<P::Value, INDEX>>(self) }
    }

    /// The last element of the array in this field, as the place of a field, as [`Atomic::last`]:
    /// every operation a field of its value has, but the add, which only a whole array's last
    /// element has.
    #[expect(unsafe_code, reason = "lends an element's place, which every element's code may hold")]
    #[inline]
    #[must_use]
    pub const fn last(&self) -> &AtomicField<Join<P, Last<P::Value>>> {
        // SAFETY: an array's elements are visible wherever the array is.
        unsafe { <P::Tail as PlaceTail>::join::<P, Last<P::Value>>(self) }
    }

    /// Each element of the array in this field, a place of its own, as [`Atomic::elements`].
    #[inline]
    #[must_use]
    pub const fn elements(&self) -> <P::Value as Array>::Elements<'_, P::Container>
    where
        P::Value: [const] Array,
    {
        P::Value::project(self)
    }

    /// Each element of the array in this field, `N` of them, its length: what the array's
    /// [`project`](Array::project) lends.
    #[inline]
    pub(crate) const fn each_element<const N: usize>(
        &self,
    ) -> [&AtomicField<IndexedElement<P>>; N] {
        let mut elements = [self.element(0); N];
        let mut index = 0;
        let mut rest = elements.as_mut_slice();
        while let [element, after @ ..] = rest {
            *element = self.element(index);
            index = index.wrapping_add(1);
            rest = after;
        }
        elements
    }
}

/// Element `index` of the array `A` whose bits, in `atomic`'s repr, `array` places.
///
/// # Safety
/// `array` places the bits of an `A` in `C`'s repr.
///
/// # Panics
/// Where `index` lies past the array's end.
#[expect(unsafe_code, reason = "places an element whose index it checked")]
#[inline]
#[track_caller]
const unsafe fn indexed_element<C: Atom, A: Array>(
    atomic: &Atomic<C>, array: FieldPlacement, index: usize,
) -> &AtomicField<Indexed<C, A::Element>> {
    if index < A::LENGTH {
        // SAFETY: the index lies within the array, as the test checked, and `array` places an
        // `A`'s bits in `C`'s repr, as the caller promises.
        unsafe { indexed_element_unchecked::<C, A>(atomic, array, index) }
    } else {
        refuse_index(index, A::LENGTH)
    }
}

/// Element `index` of the array `A`, as [`indexed_element`], or `None` past its end.
///
/// # Safety
/// As `indexed_element`'s.
#[expect(unsafe_code, reason = "places an element whose index it checked")]
#[inline]
const unsafe fn indexed_element_checked<C: Atom, A: Array>(
    atomic: &Atomic<C>, array: FieldPlacement, index: usize,
) -> Option<&AtomicField<Indexed<C, A::Element>>> {
    if index < A::LENGTH {
        // SAFETY: as in `indexed_element`.
        Some(unsafe { indexed_element_unchecked::<C, A>(atomic, array, index) })
    } else {
        None
    }
}

/// Element `index` of the array `A`, as [`indexed_element`], unchecked.
///
/// # Safety
/// `index` lies below `A::LENGTH`, and `array` places the bits of an `A` in `C`'s repr.
#[expect(unsafe_code, reason = "every operation of the place trusts its bits")]
#[inline]
#[track_caller]
const unsafe fn indexed_element_unchecked<C: Atom, A: Array>(
    atomic: &Atomic<C>, array: FieldPlacement, index: usize,
) -> &AtomicField<Indexed<C, A::Element>> {
    // SAFETY: the caller's index lies within the array; checked where the standard library's
    // checks of undefined behaviour run, as `get_unchecked` checks its index.
    unsafe {
        assert_unchecked(index < A::LENGTH);
    }
    // SAFETY: `array` places an `A`'s bits in `C`'s repr, and `index` lies below its length, by
    // the caller's contract, so `element` places that element as the tail INVARIANT asks, and an
    // element is visible wherever its array is.
    unsafe { AtomicField::from_placement(atomic, array.element::<A>(index)) }
}
