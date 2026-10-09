//! [`ReprOf`]: the repr of a reference or a function pointer, which no bits forge.

use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::FnPtr;
use core::sync::atomic::Ordering as CoreOrdering;

use super::{CellAccess, CompareExchange, Load, Primitive, RawAccess, ReadByExchange, Store, Swap};
use crate::atom::{Metadata, PointerMetadata};
use crate::ordering::{AcqRel, OrderingFloor, Relaxed};
use crate::range::mask;

/// The repr of a value no bits may forge, a reference or a function pointer: the primitive a raw
/// pointer to the same place is stored as, holding a pointer a `V` gave, or null.
///
/// It has no constructor: a `V` gives one, through its [`Atom::to_repr`](crate::Atom::to_repr),
/// and bits make none. A repr built of bits, or changed to other bits, is null, which decodes as
/// no value, so no safe code forges a reference that a decode would hand out, nor a function
/// pointer it would call. `V` names the value whole, its lifetimes and its signature too, and the
/// repr is invariant in it, so no repr one type gave decodes as another.
///
/// Its operations are the raw pointer's, so its every instruction is one an `AtomicPtr`, or a
/// double word of a wide reference, runs. A reference's take an `Acquire` read and a `Release`
/// write, or stronger, as [`LoadOrdering`](crate::ordering::LoadOrdering) and its siblings say.
///
/// # Examples
/// ```
/// # extern crate atomix_core as atomix;
/// use atomix::{Atom, Primitive, ReprOf};
///
/// /// What a trading service reads on every order.
/// struct Config {
///     depth: u32,
/// }
///
/// static DEFAULT: Config = Config { depth: 10 };
///
/// let repr: ReprOf<&Config> = (&DEFAULT).to_repr();
/// assert_eq!(<&Config>::from_repr(repr).map(|config| config.depth), Some(10), "decodes back");
/// let forged: ReprOf<&Config> = Primitive::from_bits(0xDEAD_BEE8);
/// assert!(<&Config>::from_repr(forged).is_none(), "bits make no reference");
/// ```
#[repr(transparent)]
pub struct ReprOf<V: SealedValue> {
    // INVARIANT: null, as `V::Raw::from_bits(0)` gives it, or the repr of a pointer a `V`'s
    // `to_repr` gave, its provenance kept. Its writers are `new`, whose callers prove it;
    // `from_bits`, `with_packed_bits` and `clear_packed_bits`, which keep it or make it null; the
    // cell's reads and exchanges, from a `SealedCell<V>`, which holds only what `into_cell`, `set`
    // and the cell's writes put there, each a `ReprOf<V>`; and the raw place `RawAccess` lends,
    // whose `get_mut` takes only a `ReprOf<V>`, and whose pointer `Atomic::from_ptr` and
    // `Atomic::as_ptr` bind to this invariant in their contracts.
    /// The pointer, as the raw pointer's repr.
    repr: V::Raw,
    /// The value that gave it, held invariantly, and `Send` and `Sync` whatever it is.
    marker: PhantomData<fn(V) -> V>,
}

impl<V: SealedValue> ReprOf<V> {
    /// The repr `repr`, sealed.
    ///
    /// # Safety
    /// `repr` is null, or the repr of a pointer a `V` gave, its provenance kept, as the field
    /// INVARIANT says.
    #[expect(unsafe_code, reason = "a repr whose decode trusts it")]
    #[inline]
    pub(crate) const unsafe fn new(repr: V::Raw) -> Self {
        Self { repr, marker: PhantomData }
    }

    /// The pointer, as the raw pointer's repr: null, or one a `V` gave.
    #[inline]
    pub(crate) const fn repr(self) -> V::Raw {
        self.repr
    }
}

/// A value stored as a [`ReprOf`], a reference or a function pointer: the primitive a raw pointer
/// to the same place is stored as, and the orderings an operation on it takes.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a reference or a function pointer",
    label = "expected `&T` or a `fn` pointer"
)]
pub impl(crate) trait SealedValue {
    /// The primitive a raw pointer to the same place is stored as: `*mut T`, a `DoubleWord` for a
    /// wide reference, `*mut ()` for a function.
    type Raw: Copy;
    /// The weakest ordering an operation on it takes: a reference's `AcqRel`, an `Acquire` read and
    /// a `Release` write, so a thread that reads it sees the writes that made what it points to; a
    /// function pointer's `Relaxed`, as it points to code no write changes.
    type OrderingFloor: OrderingFloor;
}

impl<T: ?Sized> SealedValue for &T
where
    Metadata<T>: PointerMetadata<T>,
{
    type Raw = <Metadata<T> as PointerMetadata<T>>::Repr;
    type OrderingFloor = AcqRel;
}

// Beside the impl for references, through negative coherence: no reference is a function pointer.
impl<F: FnPtr> SealedValue for F {
    type Raw = *mut ();
    type OrderingFloor = Relaxed;
}

// Written out: a derive would bound `V` by each trait, a reference's pointee among them.
impl<V: SealedValue> Clone for ReprOf<V> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: SealedValue> Copy for ReprOf<V> {}

impl<V: SealedValue<Raw: PartialEq>> PartialEq for ReprOf<V> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.repr == other.repr
    }
}

impl<V: SealedValue<Raw: Eq>> Eq for ReprOf<V> {}

impl<V: SealedValue<Raw: Hash>> Hash for ReprOf<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.repr.hash(state);
    }
}

impl<V: SealedValue<Raw: fmt::Debug>> fmt::Debug for ReprOf<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ReprOf").field(&self.repr).finish()
    }
}

// SAFETY: by the field INVARIANT, the repr is null or a `V`'s pointer, so it reaches what a `V`
// reaches: moving it to another thread moves no more than moving the `V` would.
#[expect(unsafe_code, reason = "a raw pointer takes away the auto `Send`")]
unsafe impl<V: SealedValue + Send> Send for ReprOf<V> {}

// SAFETY: a shared repr lends only copies of itself, as a shared `V`, which is `Copy`, does, which
// `V: Sync` allows; by the field INVARIANT it reaches nothing a `V` does not.
#[expect(unsafe_code, reason = "a raw pointer takes away the auto `Sync`")]
unsafe impl<V: SealedValue + Sync> Sync for ReprOf<V> {}

// A repr crosses threads as its value does, and a thin one is a word: the build fails here
// otherwise.
const _: () = {
    /// Compiles only where `T` may cross threads and be shared between them.
    const fn crosses_threads<T: Send + Sync>() {}
    crosses_threads::<ReprOf<&'static u8>>();
    crosses_threads::<ReprOf<fn()>>();
    assert!(size_of::<ReprOf<&u8>>() == size_of::<usize>(), "a thin reference's repr is a word");
    assert!(size_of::<ReprOf<fn()>>() == size_of::<usize>(), "a function pointer's is a word");
};

// Bits make null alone, as a `VtablePointer`'s do, and the floor is the value's.
#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] Primitive>> Primitive for ReprOf<V> {
    const BITS: u32 = V::Raw::BITS;
    const IS_BITS_EXACT: bool = false;
    type OrderingFloor = V::OrderingFloor;
    #[inline]
    fn from_bits(_: u128) -> Self {
        Self { repr: V::Raw::from_bits(0), marker: PhantomData }
    }
    #[inline]
    fn is_bits(self, bits: u128) -> bool {
        bits == 0 && self.repr.is_bits(0)
    }
    #[inline]
    fn packed_bits(self) -> u128 {
        self.repr.packed_bits()
    }
    #[inline]
    fn with_packed_bits(self, bits: u128) -> Self {
        if self.repr.packed_bits() == bits & mask(V::Raw::BITS) { self } else { Self::from_bits(0) }
    }
    #[inline]
    fn clear_packed_bits(self, mask: u128) -> Self {
        if self.repr.packed_bits() & mask == 0 { self } else { Self::from_bits(0) }
    }
}

/// The cell of a [`ReprOf`]: the raw pointer's own, in a type only `into_cell` builds, so no cell
/// holds a repr no `V` gave.
#[repr(transparent)]
pub struct SealedCell<V: SealedValue<Raw: CellAccess>> {
    // INVARIANT: holds a `ReprOf<V>`'s raw repr: null, or a pointer a `V`'s `to_repr` gave, its
    // provenance kept. Its writers are `into_cell` and `set`, each of a `ReprOf<V>`; the
    // exchanges, `store` and `swap`, each of a `ReprOf<V>` their caller passes; and the raw place
    // `RawAccess` lends, whose `get_mut` takes only a `ReprOf<V>`, and whose pointer
    // `Atomic::from_ptr` and `Atomic::as_ptr` bind to this invariant in their contracts.
    /// The raw pointer's cell.
    cell: <V::Raw as CellAccess>::Cell,
    /// The value whose reprs it holds, invariantly, as a shared cell holds it, and `Send` and
    /// `Sync` whatever it is, as the raw cell is.
    marker: PhantomData<fn(V) -> V>,
}

impl<V: SealedValue<Raw: CellAccess>> fmt::Debug for SealedCell<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedCell").finish_non_exhaustive()
    }
}

// Each takes or gives a `ReprOf<V>`, so the cell holds no other repr.
const impl<V: SealedValue<Raw: [const] CellAccess>> CellAccess for ReprOf<V> {
    type Cell = SealedCell<V>;
    #[inline]
    fn into_cell(self) -> SealedCell<V> {
        SealedCell { cell: self.repr.into_cell(), marker: PhantomData }
    }
    #[inline]
    fn from_cell(cell: SealedCell<V>) -> Self {
        Self { repr: V::Raw::from_cell(cell.cell), marker: PhantomData }
    }
    #[inline]
    fn get(cell: &mut SealedCell<V>) -> Self {
        Self { repr: V::Raw::get(&mut cell.cell), marker: PhantomData }
    }
    #[inline]
    fn set(cell: &mut SealedCell<V>, value: Self) {
        V::Raw::set(&mut cell.cell, value.repr);
    }
}

// The place a thin reference's or a function pointer's cell lends is the raw pointer's, which a
// `ReprOf<V>`, `repr(transparent)` over it, has the layout of: `get_mut` writes only a `ReprOf<V>`,
// and a write through `as_ptr`'s pointer keeps the field INVARIANT by `Atomic::as_ptr`'s contract.
//
// SAFETY: `SealedCell<V>` is `repr(transparent)` over the raw pointer's cell, which holds a
// `V::Raw` as `V::Raw` lays it out, by its own `RawAccess`, and `ReprOf<V>` is `repr(transparent)`
// over `V::Raw`, so the cell has a `ReprOf<V>`'s size and bit validity and holds it at its start.
// Under loom it promises nothing.
#[expect(unsafe_code, reason = "the raw pointer's cell holds a `ReprOf` as it lays one out")]
#[diagnostic::do_not_recommend]
const unsafe impl<V: SealedValue<Raw: [const] RawAccess>> RawAccess for ReprOf<V> {
    #[cfg(not(loom))]
    #[inline]
    fn get_mut(cell: &mut SealedCell<V>) -> &mut Self {
        let raw: *mut V::Raw = V::Raw::get_mut(&mut cell.cell);
        // SAFETY: `ReprOf<V>` is `repr(transparent)` over `V::Raw`, so the place of the one is the
        // place of the other, and the reference lives as long as the cell's borrow; it holds a
        // `ReprOf<V>`, as every repr the cell holds is one, by the field INVARIANT.
        #[expect(unsafe_code, reason = "a reference to a transparent wrapper of the referent")]
        unsafe {
            &mut *raw.cast::<Self>()
        }
    }
    #[cfg(not(loom))]
    #[inline]
    fn as_ptr(cell: &SealedCell<V>) -> *mut Self {
        V::Raw::as_ptr(&cell.cell).cast()
    }
}

/// The repr a result holds, sealed as the cell's was.
#[inline]
const fn sealed<V: SealedValue>(result: Result<V::Raw, V::Raw>) -> Result<ReprOf<V>, ReprOf<V>> {
    match result {
        Ok(repr) => Ok(ReprOf { repr, marker: PhantomData }),
        Err(repr) => Err(ReprOf { repr, marker: PhantomData }),
    }
}

// Each operation is the raw pointer's, on its cell, which holds only reprs a `V` gave, or null, so
// each read gives one. Each is `do_not_recommend`, so a capability's refusal lists no `ReprOf`
// beside the primitives.
#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] CompareExchange>> CompareExchange for ReprOf<V> {
    #[inline]
    fn read_for_rmw(cell: &SealedCell<V>, order: CoreOrdering) -> Self {
        Self { repr: V::Raw::read_for_rmw(&cell.cell, order), marker: PhantomData }
    }
    #[inline]
    fn compare_exchange(
        cell: &SealedCell<V>, current: Self, new: Self, success: CoreOrdering,
        failure: CoreOrdering,
    ) -> Result<Self, Self> {
        sealed(V::Raw::compare_exchange(&cell.cell, current.repr, new.repr, success, failure))
    }
    #[inline]
    fn compare_exchange_weak(
        cell: &SealedCell<V>, current: Self, new: Self, success: CoreOrdering,
        failure: CoreOrdering,
    ) -> Result<Self, Self> {
        sealed(V::Raw::compare_exchange_weak(&cell.cell, current.repr, new.repr, success, failure))
    }
}

#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] Load>> Load for ReprOf<V> {
    #[inline]
    fn load(cell: &SealedCell<V>, order: CoreOrdering) -> Self {
        Self { repr: V::Raw::load(&cell.cell, order), marker: PhantomData }
    }
}

#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] Store>> Store for ReprOf<V> {
    #[inline]
    fn store(cell: &SealedCell<V>, value: Self, order: CoreOrdering) {
        V::Raw::store(&cell.cell, value.repr, order);
    }
}

#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] Swap>> Swap for ReprOf<V> {
    #[inline]
    fn swap(cell: &SealedCell<V>, value: Self, order: CoreOrdering) -> Self {
        Self { repr: V::Raw::swap(&cell.cell, value.repr, order), marker: PhantomData }
    }
}

// A double word's exchange compares every bit of its cell, which holds addresses alone, so one of
// zero for zero writes nothing new; a pointer's compares its address alone, as `ReadByExchange`
// says, so neither a pointer nor its `ReprOf` reads by exchange.
#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] ReadByExchange>> ReadByExchange for ReprOf<V> {}
