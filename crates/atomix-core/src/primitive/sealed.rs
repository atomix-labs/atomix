//! [`ReprOf`]: the repr of a reference, a function pointer or a borrowed descriptor, which no bits
//! forge, and whose low bits a word's tags may share.

use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::FnPtr;
use core::sync::atomic::Ordering as CoreOrdering;

#[cfg(aarch64_code)]
use super::FetchBitwise;
#[cfg(wide)]
use super::Word;
use super::{
    BitTest, CellAccess, Code, CompareExchange, Data, ExactBits, Load, MaskBitwise, PointerRepr,
    Primitive, RawAccess, ReadByExchange, Store, Swap,
};
use crate::atom::{Metadata, PointerMetadata};
use crate::atomic::FieldPath;
use crate::ordering::{AcqRel, OrderingFloor, Relaxed};
use crate::range::{PointeeAlignment, mask};

/// The repr of a value no bits may forge, a reference or a function pointer: the primitive a raw
/// pointer to the same place is stored as, holding a pointer a `V` gave, or null.
///
/// On Unix and with the `std` feature, a borrowed file descriptor's too: a `u32` of the
/// descriptor's complement, or zero.
///
/// It has no constructor: a `V` gives one, through its [`Atom::to_repr`](crate::Atom::to_repr),
/// and bits make none. A repr built of bits is null, which decodes as no value, and so is one whose
/// bits change anywhere but in the low bits its pointee's alignment leaves clear, where a word that
/// holds it keeps its tags, so no safe code forges a reference that a decode would hand out, nor a
/// function pointer it would call. A repr with a tag set decodes as no value either: a word clears
/// its tags before it decodes its pointer. `V` names the value whole, its lifetimes and its
/// signature too, and the repr is invariant in it, so no repr one type gave decodes as another.
///
/// Its operations are the raw repr's, so its every instruction is one an `AtomicPtr`, a double word
/// of a wide reference, or a descriptor's `AtomicU32` runs. A reference's take an `Acquire` read
/// and a `Release` write, or stronger, as [`LoadOrdering`](crate::ordering::LoadOrdering) and its
/// siblings say.
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
    // INVARIANT: null, as `V::Raw::from_bits(0)` gives it, or the raw repr a `V`'s `to_repr` gave,
    // a pointer with its provenance kept or a descriptor's complement; either with any of the bits
    // below `V`'s pointee alignment, `alignment_mask::<V>()`, set, a word's tags. Its writers are
    // `new`, whose callers prove it; `from_bits`, `with_packed_bits` and `clear_packed_bits`,
    // which change only those bits or make it null; `null`, which makes it null;
    // `Word::from_exposed_bits`, whose callers pass the bits `exposed_bits` gave of such a repr,
    // as a double word's cell does; the cell's reads and read-modify-writes, from a
    // `SealedCell<V>`, which holds only what `into_cell`, `set` and the cell's writes put
    // there, each a `ReprOf<V>`, or such a repr with its tag bits changed by `MaskBitwise`,
    // which confines its masks to them; and the raw place `RawAccess` lends, whose `get_mut`
    // takes only a `ReprOf<V>`, and whose pointer `Atomic::from_ptr` and `Atomic::as_ptr` bind
    // to this invariant in their contracts.
    /// The pointer, as the raw pointer's repr, or the descriptor's complement.
    repr: V::Raw,
    /// The value that gave it, held invariantly, and `Send` and `Sync` whatever it is.
    marker: PhantomData<fn(V) -> V>,
}

impl<V: SealedValue> ReprOf<V> {
    /// The repr `repr`, sealed.
    ///
    /// # Safety
    /// `repr` is null, or the raw repr a `V`'s `to_repr` gave, a pointer with its provenance kept
    /// or a descriptor's complement, either with tags in the low bits `V`'s pointee alignment
    /// leaves clear, as the field INVARIANT says.
    #[expect(unsafe_code, reason = "a repr whose decode trusts it")]
    #[inline]
    pub(crate) const unsafe fn new(repr: V::Raw) -> Self {
        Self { repr, marker: PhantomData }
    }

    /// The raw repr: null, or one a `V` gave, either with tags.
    #[inline]
    pub(crate) const fn repr(self) -> V::Raw {
        self.repr
    }

    /// Null: the repr of no value.
    #[inline]
    pub(crate) const fn null() -> Self
    where
        V::Raw: [const] Primitive,
    {
        Self { repr: V::Raw::from_bits(0), marker: PhantomData }
    }
}

/// The low bits of a repr of a `V` that a word's tags may take: those its pointee's alignment
/// leaves clear in the address, none for a function or a trait object.
#[inline]
pub(crate) const fn alignment_mask<V: SealedValue>() -> u128 {
    mask(V::POINTEE_ALIGNMENT.width())
}

/// A value stored as a [`ReprOf`], a reference, a function pointer or a borrowed file descriptor:
/// the primitive a raw pointer to the same place is stored as, or a descriptor's, and the
/// orderings an operation on it takes.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a reference, a function pointer or a borrowed descriptor",
    label = "expected `&T`, a `fn` pointer or a `BorrowedFd`"
)]
pub impl(crate) trait SealedValue {
    /// The primitive it is stored as: a raw pointer's to the same place, `*mut T`, a `DoubleWord`
    /// for a wide reference, `*mut ()` for a function; or a descriptor's `u32`.
    type Raw: Copy;
    /// The weakest ordering an operation on it takes: a reference's `AcqRel`, an `Acquire` read and
    /// a `Release` write, so a thread that reads it sees the writes that made what it points to; a
    /// function pointer's `Relaxed`, as it points to code no write changes, and a descriptor's, as
    /// it names the kernel's resource, which its lifetime keeps open.
    type OrderingFloor: const OrderingFloor;
    /// The alignment of what the pointer points to, which leaves its low bits clear for a word's
    /// tags: none for a function, whose code's alignment no type says, nor for a descriptor.
    ///
    /// Only code reads it, never a type's layout, as [`Atom::POINTEE_ALIGNMENT`] says.
    ///
    /// [`Atom::POINTEE_ALIGNMENT`]: crate::Atom::POINTEE_ALIGNMENT
    const POINTEE_ALIGNMENT: PointeeAlignment;
    /// What the pointer points to, by its type alone, which a tuple's layout reads: [`Data`], but
    /// a function pointer's [`Code`].
    type Pointee = Data;
}

impl<T: ?Sized> SealedValue for &T
where
    Metadata<T>: PointerMetadata<T>,
{
    type Raw = <Metadata<T> as PointerMetadata<T>>::Repr;
    type OrderingFloor = AcqRel;
    const POINTEE_ALIGNMENT: PointeeAlignment =
        <Metadata<T> as PointerMetadata<T>>::POINTEE_ALIGNMENT;
}

// Beside the impl for references, through negative coherence: no reference is a function pointer.
impl<F: FnPtr> SealedValue for F {
    type Raw = *mut ();
    type OrderingFloor = Relaxed;
    const POINTEE_ALIGNMENT: PointeeAlignment = PointeeAlignment::NONE;
    type Pointee = Code;
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

// SAFETY: by the field INVARIANT, the repr is null or the raw repr a `V` gave, so it reaches what a
// `V` reaches: moving it to another thread moves no more than moving the `V` would.
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

// Bits make null alone, as a `VtablePointer`'s do; a change of any bit but a tag's makes null too,
// so the address stays one a `V` gave. The floor is the value's.
#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] Primitive>> Primitive for ReprOf<V> {
    const BITS: u32 = V::Raw::BITS;
    const IS_BITS_EXACT: bool = false;
    type OrderingFloor = V::OrderingFloor;
    #[inline]
    fn from_bits(_: u128) -> Self {
        Self::null()
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
        let changed = (self.repr.packed_bits() ^ bits) & mask(V::Raw::BITS);
        if changed & !alignment_mask::<V>() == 0 {
            Self { repr: self.repr.with_packed_bits(bits), marker: PhantomData }
        } else {
            Self::null()
        }
    }
    // A mask of tag bits alone, as every word's decode clears, clears them as the raw pointer does,
    // which tells LLVM they are clear.
    #[inline]
    fn clear_packed_bits(self, mask: u128) -> Self {
        if mask & !alignment_mask::<V>() == 0 {
            Self { repr: self.repr.clear_packed_bits(mask), marker: PhantomData }
        } else {
            self.with_packed_bits(self.packed_bits() & !mask)
        }
    }
}

// A thin reference's or a function's repr is one pointer, its provenance kept.
//
// SAFETY: `into_raw` is the raw repr's, and `from_raw` that raw repr's `from_raw`, which gives it
// back as it was, by its own impl's promise, wrapped again.
#[expect(unsafe_code, reason = "a sealed repr's rebuild trusts the round trip")]
const unsafe impl<V: SealedValue<Raw: const PointerRepr>> PointerRepr for ReprOf<V> {
    type Pointee = V::Pointee;
    #[inline]
    fn into_raw(self) -> *mut () {
        self.repr.into_raw()
    }
    #[expect(unsafe_code, reason = "rebuilds a sealed repr from the pointer it gave")]
    #[inline]
    unsafe fn from_raw(pointer: *mut ()) -> Self {
        // SAFETY: the caller's pointer is one `into_raw` gave a `ReprOf<V>`, so the raw pointer's
        // `into_raw` gave it.
        let repr = unsafe { V::Raw::from_raw(pointer) };
        // SAFETY: that repr is the one this type held, since the raw repr's `from_raw` undoes its
        // `into_raw`, by `PointerRepr`'s promise, which keeps its INVARIANT.
        unsafe { Self::new(repr) }
    }
}

// A double word holds a thin reference's or a function's repr beside another word: the cell
// exposes the pointer's provenance, and takes it back from the bits `exposed_bits` gave alone.
#[cfg(wide)]
const impl<V: SealedValue<Raw: [const] Word + [const] Primitive>> Word for ReprOf<V> {
    const IS_BITS_EXACT: bool = false;
    type OrderingFloor = V::OrderingFloor;
    #[inline]
    fn from_bits(bits: u128) -> Self {
        <Self as Primitive>::from_bits(bits)
    }
    #[inline]
    fn is_bits(self, bits: u128) -> bool {
        <Self as Primitive>::is_bits(self, bits)
    }
    #[inline]
    fn packed_bits(self) -> u128 {
        <Self as Primitive>::packed_bits(self)
    }
    #[inline]
    fn with_packed_bits(self, bits: u128) -> Self {
        <Self as Primitive>::with_packed_bits(self, bits)
    }
    #[inline]
    fn clear_packed_bits(self, mask: u128) -> Self {
        <Self as Primitive>::clear_packed_bits(self, mask)
    }
    #[inline]
    fn exposed_bits(self) -> u64 {
        self.repr.exposed_bits()
    }
    #[expect(unsafe_code, reason = "takes back a provenance the caller says the bits exposed")]
    #[inline]
    unsafe fn from_exposed_bits(bits: u64) -> Self {
        // SAFETY: the caller's bits are what `exposed_bits` gave of a `ReprOf<V>`, so of its raw
        // repr, a pointer a `V` gave, either tagged, or null, by the field INVARIANT.
        Self { repr: unsafe { V::Raw::from_exposed_bits(bits) }, marker: PhantomData }
    }
}

/// The cell of a [`ReprOf`]: the raw repr's own, in a type only `into_cell` builds, so it holds
/// only a repr a `V` gave, or null, either with tags in the bits its pointee's alignment leaves
/// clear.
#[repr(transparent)]
pub struct SealedCell<V: SealedValue<Raw: CellAccess>> {
    // INVARIANT: holds a `ReprOf<V>`'s raw repr: null, or the raw repr a `V`'s `to_repr` gave, a
    // pointer with its provenance kept or a descriptor's complement; either with tags in the bits
    // below `V`'s pointee alignment, `alignment_mask::<V>()`. Its writers are
    // `into_cell` and `set`, each of a `ReprOf<V>`; the exchanges, `store`, `swap` and
    // `store_by_exchange`, each of a `ReprOf<V>` their caller passes, and
    // `compare_exchange_write_back_weak`, of one `with_packed_bits` gives, as its caller promises;
    // the mask operations, which `tags_alone` and `address_kept` confine to the tag bits; and the
    // raw place `RawAccess` lends, whose `get_mut` takes only a `ReprOf<V>`, and whose pointer
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

/// `success`, an exchange's that writes `new`, raised to `V`'s floor: as a load's where `new` is
/// null, which publishes nothing, as `load_rmw`'s exchange of null for null writes, else as an
/// exchange's.
#[inline]
const fn raised_success<V: SealedValue<Raw: [const] Primitive>>(
    new: ReprOf<V>, success: CoreOrdering,
) -> CoreOrdering {
    if new.repr.is_bits(0) {
        V::OrderingFloor::raise_for_load(success)
    } else {
        V::OrderingFloor::raise_for_exchange(success)
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

// Each operation is the raw pointer's, on its cell, which holds only reprs a `V` gave, or null,
// either tagged, so each read gives one. Each that hands out a repr, or writes one of its own, runs
// with the ordering it is passed raised to `V`'s floor, so no read of a reference misses the writes
// that made what it points to, and no write of one fails to publish them, even where a caller of
// these hidden operations passes `Relaxed`; `Atomic`'s own orderings meet the floor already, so for
// them the raise folds away. Each that hands out bits alone, or a `bool`, and writes back the
// reference it read, takes its ordering as it is: no reference leaves it, and its write continues
// the release sequence of the store it read from. Each is `do_not_recommend`, so a capability's
// refusal lists no `ReprOf` beside the primitives.
#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] CompareExchange + [const] Primitive>> CompareExchange
    for ReprOf<V>
{
    #[inline]
    fn read_for_rmw(cell: &SealedCell<V>, order: CoreOrdering) -> Self {
        let order = V::OrderingFloor::raise_for_load(order);
        Self { repr: V::Raw::read_for_rmw(&cell.cell, order), marker: PhantomData }
    }
    #[inline]
    fn compare_exchange(
        cell: &SealedCell<V>, current: Self, new: Self, success: CoreOrdering,
        failure: CoreOrdering,
    ) -> Result<Self, Self> {
        let (success, failure) =
            (raised_success(new, success), V::OrderingFloor::raise_for_load(failure));
        sealed(V::Raw::compare_exchange(&cell.cell, current.repr, new.repr, success, failure))
    }
    #[inline]
    fn compare_exchange_weak(
        cell: &SealedCell<V>, current: Self, new: Self, success: CoreOrdering,
        failure: CoreOrdering,
    ) -> Result<Self, Self> {
        let (success, failure) =
            (raised_success(new, success), V::OrderingFloor::raise_for_load(failure));
        sealed(V::Raw::compare_exchange_weak(&cell.cell, current.repr, new.repr, success, failure))
    }
    // The reads hand out nothing; the write is `new`'s, raised as a store's.
    #[inline]
    fn store_by_exchange(cell: &SealedCell<V>, new: Self, order: CoreOrdering) {
        V::Raw::store_by_exchange(&cell.cell, new.repr, V::OrderingFloor::raise_for_store(order));
    }
    // The write is `current`'s pointer, its tags alone changed, or null, as the caller promises,
    // so it publishes nothing, and the read hands out a repr, so it acquires: a write-back's raise.
    #[expect(unsafe_code, reason = "the raise trusts `new` to keep `current`'s pointer")]
    #[inline]
    unsafe fn compare_exchange_write_back_weak(
        cell: &SealedCell<V>, current: Self, new: Self, success: CoreOrdering,
        failure: CoreOrdering,
    ) -> Result<Self, Self> {
        let success = V::OrderingFloor::raise_for_write_back(success);
        let failure = V::OrderingFloor::raise_for_load(failure);
        sealed(V::Raw::compare_exchange_weak(&cell.cell, current.repr, new.repr, success, failure))
    }
}

#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] Load>> Load for ReprOf<V> {
    #[inline]
    fn load(cell: &SealedCell<V>, order: CoreOrdering) -> Self {
        let order = V::OrderingFloor::raise_for_load(order);
        Self { repr: V::Raw::load(&cell.cell, order), marker: PhantomData }
    }
    #[inline]
    fn load_bits(cell: &SealedCell<V>, order: CoreOrdering) -> u128 {
        V::Raw::load_bits(&cell.cell, order)
    }
}

#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] Store>> Store for ReprOf<V> {
    #[inline]
    fn store(cell: &SealedCell<V>, value: Self, order: CoreOrdering) {
        V::Raw::store(&cell.cell, value.repr, V::OrderingFloor::raise_for_store(order));
    }
}

#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] Swap>> Swap for ReprOf<V> {
    #[inline]
    fn swap(cell: &SealedCell<V>, value: Self, order: CoreOrdering) -> Self {
        let order = V::OrderingFloor::raise_for_exchange(order);
        Self { repr: V::Raw::swap(&cell.cell, value.repr, order), marker: PhantomData }
    }
}

// A double word's exchange compares every bit of its cell, which holds addresses alone, so one of
// zero for zero writes nothing new; a pointer's compares its address alone, as `ReadByExchange`
// says, so neither a pointer nor its `ReprOf` reads by exchange; a descriptor's `u32` does, as its
// every bit is its own and an exchange writes them back as they were.
#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] ReadByExchange + [const] Primitive>> ReadByExchange
    for ReprOf<V>
{
    #[inline]
    fn load_bits_by_exchange(cell: &SealedCell<V>, order: CoreOrdering) -> u128 {
        V::Raw::load_bits_by_exchange(&cell.cell, order)
    }
}

/// `mask`, its bits outside the tags a `V`'s repr may hold cleared, so no operation it masks moves
/// the address.
#[inline]
const fn tags_alone<V: SealedValue<Raw: [const] MaskBitwise>>(
    mask: <V::Raw as MaskBitwise>::Mask,
) -> <V::Raw as MaskBitwise>::Mask {
    Primitive::from_bits(mask.to_bits() & alignment_mask::<V>())
}

/// `mask`, every bit outside the tags a `V`'s repr may hold set, so no `and` it masks moves the
/// address.
#[inline]
const fn address_kept<V: SealedValue<Raw: [const] MaskBitwise>>(
    mask: <V::Raw as MaskBitwise>::Mask,
) -> <V::Raw as MaskBitwise>::Mask {
    Primitive::from_bits(mask.to_bits() | !alignment_mask::<V>())
}

// Each mask is confined to the tag bits, so a word's tag fields change in place and the address,
// one a `V` gave, stays as it is, whatever mask a caller passes. Each `fetch_` form reads back the
// repr, so its ordering is raised to `V`'s floor as a read-back's, an acquiring read; each form
// that discards it hands out nothing, so takes its ordering as it is.
#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] MaskBitwise>> MaskBitwise for ReprOf<V> {
    type Mask = <V::Raw as MaskBitwise>::Mask;
    #[inline]
    fn bit(position: u32) -> Self::Mask {
        V::Raw::bit(position)
    }
    #[inline]
    fn fetch_or_mask(cell: &SealedCell<V>, mask: Self::Mask, order: CoreOrdering) -> Self {
        let order = V::OrderingFloor::raise_for_write_back(order);
        let before = V::Raw::fetch_or_mask(&cell.cell, tags_alone::<V>(mask), order);
        Self { repr: before, marker: PhantomData }
    }
    #[inline]
    fn fetch_and_mask(cell: &SealedCell<V>, mask: Self::Mask, order: CoreOrdering) -> Self {
        let order = V::OrderingFloor::raise_for_write_back(order);
        let before = V::Raw::fetch_and_mask(&cell.cell, address_kept::<V>(mask), order);
        Self { repr: before, marker: PhantomData }
    }
    #[inline]
    fn fetch_xor_mask(cell: &SealedCell<V>, mask: Self::Mask, order: CoreOrdering) -> Self {
        let order = V::OrderingFloor::raise_for_write_back(order);
        let before = V::Raw::fetch_xor_mask(&cell.cell, tags_alone::<V>(mask), order);
        Self { repr: before, marker: PhantomData }
    }
    #[inline]
    fn or_mask(cell: &SealedCell<V>, mask: Self::Mask, order: CoreOrdering) {
        V::Raw::or_mask(&cell.cell, tags_alone::<V>(mask), order);
    }
    #[inline]
    fn and_mask(cell: &SealedCell<V>, mask: Self::Mask, order: CoreOrdering) {
        V::Raw::and_mask(&cell.cell, address_kept::<V>(mask), order);
    }
    #[inline]
    fn xor_mask(cell: &SealedCell<V>, mask: Self::Mask, order: CoreOrdering) {
        V::Raw::xor_mask(&cell.cell, tags_alone::<V>(mask), order);
    }
}

/// Whether the bit at `position`, modulo the width, is one a `V`'s repr may hold a tag in.
#[inline]
const fn is_tag_bit<V: SealedValue<Raw: [const] Primitive>>(position: u32) -> bool {
    match position.checked_rem(V::Raw::BITS) {
        Some(bit) => bit < V::POINTEE_ALIGNMENT.width(),
        None => false,
    }
}

/// The bit at `position` of the repr `cell` holds, read by an or of nothing: what a bit test past
/// the tags gives, changing nothing.
#[inline]
const fn test_bit<V: SealedValue<Raw: [const] MaskBitwise>>(
    cell: &SealedCell<V>, position: u32, order: CoreOrdering,
) -> bool {
    let nothing = Primitive::from_bits(0);
    let bits = V::Raw::fetch_or_mask(&cell.cell, nothing, order).packed_bits();
    bits & V::Raw::bit(position).to_bits() != 0
}

/// `BitTest`'s methods for a `ReprOf`, each `$method` at a position and `$field` at a field's: the
/// raw pointer's at a tag bit, else the bit read and left as it is.
macro_rules! tag_bit_tests {
    ($($method:ident, $field:ident;)+) => {$(
        #[inline]
        fn $method(cell: &SealedCell<V>, position: u32, order: CoreOrdering) -> bool {
            if is_tag_bit::<V>(position) {
                V::Raw::$method(&cell.cell, position, order)
            } else {
                test_bit(cell, position, order)
            }
        }
        #[inline]
        fn $field<P: FieldPath>(cell: &SealedCell<V>, order: CoreOrdering) -> bool {
            if is_tag_bit::<V>(P::OFFSET) {
                V::Raw::$field::<P>(&cell.cell, order)
            } else {
                test_bit(cell, P::OFFSET, order)
            }
        }
    )+};
}

// A tag bit's test is the raw pointer's, `lock bts` or `ldset` alike; a bit past the tags is read
// and left as it is, so the address stays one a `V` gave.
#[diagnostic::do_not_recommend]
const impl<V: SealedValue<Raw: [const] BitTest>> BitTest for ReprOf<V> {
    tag_bit_tests! {
        test_and_set_bit, test_and_set_field;
        test_and_clear_bit, test_and_clear_field;
        test_and_toggle_bit, test_and_toggle_field;
    }
}

// As the raw pointer's, whose cfg `atomic/field/mod.rs` repeats.
#[cfg(aarch64_code)]
#[diagnostic::do_not_recommend]
impl<V: SealedValue<Raw: FetchBitwise>> FetchBitwise for ReprOf<V> {}
