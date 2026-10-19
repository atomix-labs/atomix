//! One field of an atomic packed value, changed alone: [`AtomicField`], the place a
//! [`FieldPath`] names, and the projection that lends one per field.

use core::hint::assert_unchecked;
use core::marker::{Destruct, PhantomData};
use core::{fmt, ptr};

#[doc(hidden)]
pub use self::element::Array;
pub use self::element::{Element, Last};
pub(crate) use self::path::ReachesTop;
pub use self::path::{Field, FieldPath, Indexed, Join, Then, TopField, Whole};
#[doc(hidden)]
pub use self::path::{HasPackedField, PlaceTail, Reach};
use super::Atomic;
use crate::atom::{Atom, FieldAdd, FieldBitwise};
use crate::ordering::{FieldOf, LoadOrdering, RmwOrdering, StoreOrdering};
use crate::primitive::{
    BitTest, CellAccess, CompareExchange, ExactBits, FetchAdd, FetchBitwise, Load, MaskBitwise,
    Primitive, ReadByExchange,
};
use crate::range::{PackedField, mask, sign_extend};

mod element;
mod path;

/// A packed value whose atomic projects onto its fields, each an [`AtomicField`].
///
/// `#[derive(Atom)]` implements it for a packed struct, `Quote`, and writes its projection,
/// `QuoteFields<'a, P>`, the struct of one `&'a AtomicField` per field, each of the field's own
/// visibility, that [`Atomic::fields`] and [`AtomicField::fields`] lend.
///
/// # Safety
/// The projection lends the place of each field of `Self`, at its path from the place projected,
/// each no more visible than its field, and no other place: never the place projected, whose path
/// at an atomic's root, [`Whole`], spans every bit of the repr, more than the field promises,
/// [`FieldBitwise`] and [`FieldAdd`], say decode.
///
/// # Examples
/// [`#[derive(Atom)]`'s example][derive] projects an atomic ring buffer slot onto its fields.
///
/// [derive]: https://docs.rs/atomix-rs/latest/atomix/derive.Atom.html#changing-one-field
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no fields an atomic projects onto",
    label = "`fields` needs a packed struct that derives `Atom`",
    note = "`#[derive(Atom)]` projects a struct of several fields: an enum, a newtype or a zero-width struct has none"
)]
#[expect(unsafe_code, reason = "the field operations trust the places the projection lends")]
pub const unsafe trait ProjectFields: Atom {
    /// The fields of the place at the path `P`, whose value is `Self`.
    type Fields<'a, P: FieldPath<Value = Self> + 'a>
    where
        Self: 'a;
    /// The fields of `place`.
    #[doc(hidden)]
    fn project<P: FieldPath<Value = Self>>(place: &AtomicField<P>) -> Self::Fields<'_, P>;
}

/// The repr of the container of the field `P`.
type ContainerRepr<P> = <<P as FieldPath>::Container as Atom>::Repr;

/// The mask of the repr of the container of the field `P`.
type ContainerMask<P> = <ContainerRepr<P> as MaskBitwise>::Mask;

/// Where a field's bits lie in its container's repr, and how they read back: a path's constants,
/// or a run-time place's, an array's element's at an index known at run time.
#[derive(Clone, Copy)]
struct FieldPlacement {
    /// The field's lowest bit.
    offset: u32,
    /// How many bits it takes.
    width: u32,
    /// Whether it is two's complement, sign-extended when read.
    signed: bool,
    /// The bit after the last it governs, as [`FieldPath::END`] says.
    end: u32,
    /// How many bits the container's repr has.
    container_width: u32,
}

impl FieldPlacement {
    /// The bits of the field at the path `P`.
    #[inline]
    const fn of<P: FieldPath>() -> Self {
        Self {
            offset: P::OFFSET,
            width: P::WIDTH,
            signed: P::IS_SIGNED,
            end: P::END,
            container_width: ContainerRepr::<P>::BITS,
        }
    }

    /// The bits of a `V` in a `C`'s repr whose offset and end `metadata` holds, as
    /// [`metadata`](Self::metadata) packs them: a run-time place's, its width and sign `V`'s, as
    /// every field's are its value's.
    #[inline]
    const fn from_metadata<C: Atom, V: Atom>(metadata: usize) -> Self {
        let layout = const { PackedField::new(V::REPRS, 0).layout() };
        let offset = metadata.wrapping_shr(u8::BITS).wrapping_cast::<u32>();
        let span = (metadata & usize::from(u8::MAX)).wrapping_cast::<u32>();
        Self {
            offset,
            width: layout.width(),
            signed: layout.is_signed(),
            end: offset.wrapping_add(span),
            container_width: <C::Repr as Primitive>::BITS,
        }
    }

    /// The offset, and how many bits from it to the end, each at most 128, in one word, what a
    /// run-time place's tail holds: what [`from_metadata`](Self::from_metadata) reads back. The
    /// offset lies in the high bits, so a constant span reads back as one, whatever the offset.
    #[inline]
    const fn metadata(self) -> usize {
        let span = self.end.wrapping_sub(self.offset).wrapping_cast::<usize>();
        self.offset.wrapping_cast::<usize>().wrapping_shl(u8::BITS) | span
    }

    /// The bits of `inner`, a field of the value these bits hold, in this field's container: as
    /// [`Then`] joins two paths.
    ///
    /// `inner`'s bits lie at this field's offset plus its own; it governs no more of the value than
    /// this field stores, but, where the container's bits above this field copy its top bit, which
    /// is `inner`'s where `inner` is signed and ends at this field's width, those bits too.
    #[inline]
    const fn then(self, inner: Self) -> Self {
        let end = if self.end > self.offset.saturating_add(self.width)
            && inner.signed
            && inner.offset.saturating_add(inner.width) == self.width
        {
            self.end
        } else if inner.end < self.width {
            self.offset.saturating_add(inner.end)
        } else {
            self.offset.saturating_add(self.width)
        };
        Self {
            offset: self.offset.saturating_add(inner.offset),
            width: inner.width,
            signed: inner.signed,
            end,
            container_width: self.container_width,
        }
    }

    /// `bits`, of a repr of the field's value, packed in their place in the container's repr:
    /// moved to the field's offset, and, where signed, extended up to the bits it governs.
    #[inline]
    const fn pack(self, bits: u128) -> u128 {
        self.shift_up(self.extend(bits) & mask(self.end.wrapping_sub(self.offset)))
    }

    /// The operand of an and that writes `bits`, of a repr of the field's value, to the field and
    /// keeps every other bit: [`pack`](Self::pack)'s, every bit the field does not govern set,
    /// built as the complement of the governed bits `bits` clear, which takes one shift, where
    /// the packed bits or'd with the complement of the governed ones take two.
    #[inline]
    const fn and_operand(self, bits: u128) -> u128 {
        !self.shift_up(!self.extend(bits) & mask(self.end.wrapping_sub(self.offset)))
    }

    /// `bits`, of a repr of the field's value, unsigned below its width or, where signed, extended
    /// above it: the bits it stores, before they move to its offset.
    #[inline]
    const fn extend(self, bits: u128) -> u128 {
        if self.signed {
            sign_extend(bits, self.width).cast_unsigned()
        } else {
            bits & mask(self.width)
        }
    }

    /// The bits the field governs in the container's repr.
    #[inline]
    const fn governed(self) -> u128 {
        self.shift_up(mask(self.end.wrapping_sub(self.offset)))
    }

    /// The field of `bits`, a repr of the container's, read back as the unsigned bits of its
    /// value's repr, `value_width` bits wide.
    #[inline]
    const fn unpack(self, bits: u128, value_width: u32) -> u128 {
        let field = self.shift_down(bits) & mask(self.width);
        if self.signed {
            sign_extend(field, self.width).cast_unsigned() & mask(value_width)
        } else {
            field
        }
    }

    /// The field of `bits`, a repr of the container's, replaced by `value`, of a repr of its
    /// value's.
    #[inline]
    const fn replace(self, bits: u128, value: u128) -> u128 {
        (bits & !self.governed()) | self.pack(value)
    }

    /// `bits` moved up to the field's offset, in the narrowest integer that holds the container's
    /// repr, so a shift by an offset known only at run time is one instruction.
    #[inline]
    const fn shift_up(self, bits: u128) -> u128 {
        if self.container_width <= u64::BITS {
            u128::from(bits.wrapping_cast::<u64>().wrapping_shl(self.offset))
        } else {
            bits.wrapping_shl(self.offset)
        }
    }

    /// `bits` moved down from the field's offset, as [`shift_up`](Self::shift_up) moves them up.
    #[inline]
    const fn shift_down(self, bits: u128) -> u128 {
        if self.container_width <= u64::BITS {
            u128::from(bits.wrapping_cast::<u64>().wrapping_shr(self.offset))
        } else {
            bits.wrapping_shr(self.offset)
        }
    }
}

/// The bits `bits` of a repr of `P`'s value, packed in their place in its container's repr: moved
/// to its offset, and, where signed, extended up to the bits it governs.
#[inline]
const fn pack<P: FieldPath>(bits: u128) -> u128 {
    let bits = if P::IS_SIGNED {
        sign_extend(bits, P::WIDTH).cast_unsigned()
    } else {
        bits & mask(P::WIDTH)
    };
    bits.unbounded_shl(P::OFFSET) & mask(P::END)
}

/// The bits the field `P` governs in its container's repr.
#[inline]
const fn governed<P: FieldPath>() -> u128 {
    mask(P::END) & !mask(P::OFFSET)
}

/// The field `P` of `bits`, a repr of its container's, unpacked as the unsigned bits of its value's
/// repr.
#[inline]
const fn unpack<P: FieldPath>(bits: u128) -> u128 {
    let field = bits.unbounded_shr(P::OFFSET) & mask(P::WIDTH);
    if P::IS_SIGNED {
        sign_extend(field, P::WIDTH).cast_unsigned()
            & mask(<<P::Value as Atom>::Repr as Primitive>::BITS)
    } else {
        field
    }
}

/// The field `P` of the container's repr `bits`.
///
/// # Safety
/// `bits` are a repr of `P`'s container that decodes.
#[expect(unsafe_code, reason = "decodes a field of a container repr that decodes")]
#[inline]
const unsafe fn decode<P: [const] FieldPath>(bits: u128) -> P::Value {
    // SAFETY: by `FieldPath`'s promise, the field of a repr that decodes is a repr that decodes,
    // and the caller's repr decodes.
    unsafe { P::Value::from_repr_unchecked(Primitive::from_bits(unpack::<P>(bits))) }
}

/// The field `P` of `repr`, a repr of its container's: its bits read back, or, for a field stored
/// as a pointer, whose bits do not hold its provenance, the field of the value `repr` decodes as.
///
/// # Safety
/// `repr` decodes as `P`'s container.
#[expect(unsafe_code, reason = "decodes a field of a container repr that decodes")]
#[inline]
const unsafe fn decode_repr<P: [const] FieldPath>(repr: ContainerRepr<P>) -> P::Value {
    if <<P::Value as Atom>::Repr as Primitive>::IS_BITS_EXACT {
        // SAFETY: the caller's repr decodes.
        unsafe { decode::<P>(repr.packed_bits()) }
    } else {
        // SAFETY: the caller's repr decodes.
        P::field(unsafe { P::Container::from_repr_unchecked(repr) })
    }
}

impl<C: Atom> Atomic<C> {
    /// The fields of the value, each a place of its own: `quote.fields().live`.
    ///
    /// `F` is the projection `#[derive(Atom)]` writes beside the value: `QuoteFields`.
    ///
    /// # Examples
    /// [`#[derive(Atom)]`'s example][derive] claims a ring buffer slot through its fields.
    ///
    /// [derive]: https://docs.rs/atomix-rs/latest/atomix/derive.Atom.html#changing-one-field
    // `F`, rather than the projection in the return type, which rustc would resolve before the
    // bound, so a value without fields reports `ProjectFields`' message.
    #[expect(unsafe_code, reason = "lends the place of the whole value to the projection")]
    #[inline]
    #[must_use]
    pub const fn fields<'a, F>(&'a self) -> F
    where
        C: [const] ProjectFields<Fields<'a, Whole<C>> = F>,
    {
        // SAFETY: the place of the whole value goes to the projection alone, which, by
        // `ProjectFields`' promise, lends only its fields' places, each no more visible than its
        // field.
        C::project(unsafe { AtomicField::<Whole<C>>::from_atomic(self) })
    }
}

/// The place of the field `INDEX`, a `V`, of the value at `place`: each place of the projection
/// `#[derive(Atom)]` writes.
///
/// # Safety
/// The place reaches only code the field is visible to: a private field may carry an invariant its
/// owner's `unsafe` code relies on, which no other module may then break.
#[doc(hidden)]
#[expect(unsafe_code, reason = "lends a field's place, which only code it is visible to may hold")]
#[inline]
#[must_use]
pub const unsafe fn project_field<P: FieldPath, V: Atom, const INDEX: u32>(
    place: &AtomicField<P>,
) -> &AtomicField<Join<P, Field<P::Value, INDEX, V>>>
where
    P::Value: HasPackedField<INDEX, V>,
{
    // SAFETY: the place reaches only code the field is visible to, by the caller's contract.
    unsafe { <P::Tail as PlaceTail>::join::<P, Field<P::Value, INDEX, V>>(place) }
}

/// A field of an atomic packed value, as a place of its own: what [`Atomic::fields`] lends.
///
/// Each operation is one atomic instruction on the atomic's whole word, as [`Atomic`]'s are, whose
/// operand changes no other field, but [`update`](Self::update) and
/// [`try_update`](Self::try_update), compare-exchange loops, and [`load_rmw`](Self::load_rmw), one
/// compare-exchange, which writes. Which a field has depends on its value, and on what its
/// container's repr runs without a loop:
///
/// - **Every field**: [`load`](Self::load) where the repr has [`Load`], else
///   [`load_rmw`](Self::load_rmw), and `update` and `try_update`: a 128-bit container's field has
///   these alone.
/// - **A `bool`**: [`set`](Self::set), [`clear`](Self::clear), [`toggle`](Self::toggle) and
///   [`store`](Self::store), and, where the repr has [`BitTest`],
///   [`test_and_set`](Self::test_and_set), [`test_and_clear`](Self::test_and_clear) and
///   [`test_and_toggle`](Self::test_and_toggle), which return the bit before.
/// - **A value with [`FieldBitwise`]**: [`and`](Self::and), [`or`](Self::or), [`xor`](Self::xor)
///   and [`not`](Self::not), where the repr has [`MaskBitwise`], and their `fetch_` forms, where
///   [`FetchBitwise`] too.
/// - **A value with [`FieldAdd`], in the field that ends at the repr's top bit ([`TopField`])**:
///   [`fetch_add`](Self::fetch_add) and [`fetch_sub`](Self::fetch_sub), where it has [`FetchAdd`].
///
/// Each `fetch_` form returns the container before: the one snapshot of every field that the
/// instruction reads. Each operation is `const` where its container's is, as [`Atomic`] says, and
/// `Debug` reads the field as `Atomic`'s reads the word. A pointer word's tag field has each
/// operation its value gives it but the add, since the pointer lies above it, and each keeps the
/// pointer's provenance. Its pointer field has only [`load`](Self::load), which reads it through
/// the word, and, where the pointer is a word too, [`fields`](Self::fields).
///
/// Beside a reference, a field stored as bits takes its own value's orderings wherever it hands
/// out no more than itself: its loads, `set` and the other forms that discard what they read, and
/// the tests of a bit. An `update`, a `try_update` or a `fetch_` form hands out the container, so
/// takes [`FieldOf`]'s: an `Acquire` read, its write needing no `Release`, as it puts the reference
/// back as it read it.
///
/// A place is the atomic itself, its path in its type: `AtomicField` is `repr(C)`, the atomic its
/// first field, so a reference to the atomic is a reference to each place in it. A place at an
/// index known at run time, an [`Indexed`] path's, holds where its bits lie in its tail, a slice of
/// units, which takes no bytes: its reference is two words, the second the slice's length, their
/// offset and span, which its operations shift by where a constant place's shift by its path's
/// constants, and which an index the compiler knows folds away.
///
/// # Examples
/// [`#[derive(Atom)]`'s example][derive] claims a ring buffer slot with one field's
/// `test_and_set`, and counts its readers with another's `fetch_add`.
///
/// [derive]: https://docs.rs/atomix-rs/latest/atomix/derive.Atom.html#changing-one-field
#[repr(C)]
pub struct AtomicField<P: FieldPath> {
    /// The atomic the field lies in.
    atomic: Atomic<P::Container>,
    /// The path from the atomic's repr to the field.
    path: PhantomData<P>,
    // INVARIANT: at a place of a path whose tail is `[()]`, `Indexed`'s, the tail's length is the
    // metadata of the bits of a `P::Value`, a value whose repr is bits (`IS_BITS_EXACT`), in
    // `P::Container`'s repr: an element's below its array's length, as `FieldPlacement::element`
    // places it in its array's bits, or a field's of one, as `FieldPlacement::then` places it, so
    // bits of any width lie below the repr's width, and none at most at it. Written by
    // `from_placement` alone, whose callers prove it, and which checks `IS_BITS_EXACT` at build.
    /// Where the field's bits lie, at a place whose index comes at run time: `[()]`, whose length
    /// holds their offset and end. `()` at any other.
    tail: P::Tail,
}

// A place is its atomic, which `from_atomic` and `from_placement` cast to it: a place's bytes are
// the atomic's, and a run-time place's reference adds one word, its tail's length.
const _: () = {
    assert!(size_of::<AtomicField<Whole<u64>>>() == size_of::<Atomic<u64>>(), "a place's bytes");
    assert!(align_of::<AtomicField<Whole<u64>>>() == align_of::<Atomic<u64>>(), "its alignment");
    assert!(
        size_of::<&AtomicField<Indexed<[u8; 4], u8>>>() == 2 * size_of::<usize>(),
        "a run-time place's reference, the atomic's address and the tail's length"
    );
};

impl<P: FieldPath<Tail = ()>> AtomicField<P> {
    /// The place of the field `P` of `atomic`.
    ///
    /// # Safety
    /// The place reaches only code the field is visible to; and, where `P` is [`Whole`], whose
    /// bits are more than [`FieldBitwise`] and [`FieldAdd`] promise decode, only the projection.
    #[expect(unsafe_code, reason = "a reference to a wrapper of the referent")]
    #[inline]
    const unsafe fn from_atomic(atomic: &Atomic<P::Container>) -> &Self {
        // SAFETY: `AtomicField<P>` is `repr(C)`, its first field `Atomic<P::Container>`, beside
        // two of no bytes, a marker and the tail `()`, so the cast keeps the layout, the
        // provenance and the referent's `UnsafeCell`, and the reference lives as long as the
        // atomic's.
        unsafe { &*ptr::from_ref(atomic).cast::<Self>() }
    }
}

impl<P: FieldPath<Tail = [()]>> AtomicField<P> {
    /// The place of the field `P` of `atomic` whose bits lie at `placement`, at an index known at
    /// run time.
    ///
    /// # Safety
    /// `placement` places the bits of a `P::Value` in `P::Container`'s repr, as the tail INVARIANT
    /// says, and the place reaches only code the field is visible to.
    #[expect(unsafe_code, reason = "a reference to a wrapper of the referent, with its tail")]
    #[inline]
    const unsafe fn from_placement(
        atomic: &Atomic<P::Container>, placement: FieldPlacement,
    ) -> &Self {
        const {
            assert!(
                <<P::Value as Atom>::Repr as Primitive>::IS_BITS_EXACT,
                "a place at a run-time index reads its value's bits, which a pointer's are not"
            );
        };
        let place: *const Self = ptr::from_raw_parts(ptr::from_ref(atomic), placement.metadata());
        // SAFETY: `AtomicField<P>` is `repr(C)`, its first field `Atomic<P::Container>`, beside a
        // marker of no bytes and the tail `[()]`, whose elements take none, so the pointer keeps
        // the atomic's layout, its provenance and the referent's `UnsafeCell`, and the reference
        // lives as long as the atomic's; the tail's length keeps its INVARIANT, by the caller's
        // placement, the value's repr bits, as the build checked.
        unsafe { &*place }
    }
}

impl<P: FieldPath> AtomicField<P> {
    /// Whether the place's tail places its bits, at an index known at run time, not its path's
    /// constants.
    const IS_RUN_TIME: bool = <P::Tail as PlaceTail>::IS_RUN_TIME;

    /// The atomic the field lies in, for the place of one of its parts.
    #[inline]
    const fn atomic(&self) -> &Atomic<P::Container> {
        &self.atomic
    }

    /// The primitive's cell.
    #[inline]
    const fn cell(&self) -> &<ContainerRepr<P> as CellAccess>::Cell {
        self.atomic.primitive_cell()
    }

    /// Where the field's bits lie at a run-time place, as its tail holds them, an offset below the
    /// repr's width where they are any, as LLVM is told, so a shift by it needs no test of a wider
    /// one: of a place whose path's constants place it, the tail holds none.
    #[expect(unsafe_code, reason = "tells LLVM what the tail's INVARIANT holds")]
    #[inline]
    const fn run_time_placement(&self) -> FieldPlacement {
        let placement =
            FieldPlacement::from_metadata::<P::Container, P::Value>(self.tail.metadata());
        // SAFETY: by the tail INVARIANT, bits of any width lie below the repr's width; a tail of
        // nothing holds 0.
        unsafe {
            assert_unchecked(placement.width == 0 || placement.offset < ContainerRepr::<P>::BITS);
        }
        placement
    }

    /// Where the field's bits lie: the tail's, at a run-time place, else the path's constants.
    #[inline]
    const fn placement(&self) -> FieldPlacement {
        if Self::IS_RUN_TIME { self.run_time_placement() } else { FieldPlacement::of::<P>() }
    }

    /// The field of `bits`, a repr of its container's, read back: at a run-time place, at
    /// `placement`, its [`run_time_placement`](Self::run_time_placement), read before the word,
    /// so the shift's offset is ready with it; at any other, at its path's constants.
    ///
    /// # Safety
    /// `bits` are a repr of `P`'s container that decodes, and `placement` the place's own.
    #[expect(unsafe_code, reason = "decodes a field of a container repr that decodes")]
    #[inline]
    const unsafe fn decode_bits(placement: FieldPlacement, bits: u128) -> P::Value
    where
        P: [const] FieldPath,
    {
        if Self::IS_RUN_TIME {
            let width = <<P::Value as Atom>::Repr as Primitive>::BITS;
            let field = placement.unpack(bits, width);
            // SAFETY: the caller's repr decodes, so its bits the tail places hold a repr of the
            // field's value that decodes, by the tail INVARIANT, as a path's keep `FieldPath`'s
            // promise.
            unsafe { P::Value::from_repr_unchecked(Primitive::from_bits(field)) }
        } else {
            // SAFETY: the caller's repr decodes.
            unsafe { decode::<P>(bits) }
        }
    }

    /// `value` packed in the field's place, every other bit clear: the operand of an or and of a
    /// xor.
    #[inline]
    const fn operand(&self, value: P::Value) -> ContainerMask<P>
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: MaskBitwise,
    {
        let bits = value.to_repr().to_bits();
        let packed =
            if Self::IS_RUN_TIME { self.run_time_placement().pack(bits) } else { pack::<P>(bits) };
        ContainerMask::<P>::from_bits(packed)
    }

    /// `value` packed in the field's place, every bit it does not govern set: the operand of an
    /// and, which so leaves every other field as it is, one whose zero does not decode too.
    #[inline]
    const fn and_operand(&self, value: P::Value) -> ContainerMask<P>
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: MaskBitwise,
    {
        let bits = value.to_repr().to_bits();
        let packed = if Self::IS_RUN_TIME {
            self.run_time_placement().and_operand(bits)
        } else {
            pack::<P>(bits) | !governed::<P>()
        };
        ContainerMask::<P>::from_bits(packed)
    }

    /// Every bit the field governs: the operand of a not, as a xor.
    #[inline]
    const fn not_operand(&self) -> ContainerMask<P>
    where
        ContainerRepr<P>: MaskBitwise,
    {
        let governed =
            if Self::IS_RUN_TIME { self.run_time_placement().governed() } else { governed::<P>() };
        ContainerMask::<P>::from_bits(governed)
    }

    /// The field of `bits`, a repr of its container's, replaced by `value`, a repr of its value's:
    /// at `placement` at a run-time place, as [`decode_bits`](Self::decode_bits) reads it.
    #[inline]
    const fn replace(placement: FieldPlacement, bits: u128, value: u128) -> u128 {
        if Self::IS_RUN_TIME {
            placement.replace(bits, value)
        } else {
            (bits & !governed::<P>()) | pack::<P>(value)
        }
    }

    /// The fields of this field's value, each a place of its own:
    /// `outer.fields().inner.fields().ready`.
    ///
    /// `F` is the projection `#[derive(Atom)]` writes beside the value, as for
    /// [`Atomic::fields`].
    ///
    /// # Examples
    /// [`#[derive(Atom)]`'s example][derive] halts an order through a field of its field.
    ///
    /// [derive]: https://docs.rs/atomix-rs/latest/atomix/derive.Atom.html#changing-a-field-through-generic-code
    // `F`, as in `Atomic::fields`.
    #[inline]
    #[must_use]
    pub const fn fields<'a, F>(&'a self) -> F
    where
        P::Value: [const] ProjectFields<Fields<'a, P> = F>,
    {
        P::Value::project(self)
    }

    /// Reads the field: a load of the whole word, and the field's bits read back, or, for a
    /// pointer word's pointer, the word decoded.
    ///
    /// A field stored as bits takes its own value's orderings, beside a reference too: its load
    /// reads the word's bits, and hands out no reference.
    #[expect(unsafe_code, reason = "decodes a field of a repr read from the cell")]
    #[inline]
    pub const fn load<O: LoadOrdering<P::Value>>(&self, order: O) -> P::Value
    where
        P: [const] FieldPath,
        ContainerRepr<P>: [const] Load,
    {
        let _ = order;
        if Self::IS_RUN_TIME || <<P::Value as Atom>::Repr as Primitive>::IS_BITS_EXACT {
            let placement = self.run_time_placement();
            let bits = ContainerRepr::<P>::load_bits(self.cell(), O::CORE);
            // SAFETY: by `Atomic`'s field INVARIANT, the repr whose bits the cell holds decodes.
            unsafe { Self::decode_bits(placement, bits) }
        } else {
            let repr = ContainerRepr::<P>::load(self.cell(), O::CORE);
            // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
            unsafe { decode_repr::<P>(repr) }
        }
    }

    /// Reads the field with a compare-exchange of the whole word, for a container whose repr has
    /// no [`Load`].
    ///
    /// As [`Atomic::load_rmw`]: the exchange takes the cache line exclusive and writes it, so it
    /// faults on read-only memory.
    #[expect(unsafe_code, reason = "decodes a field of a repr read from the cell")]
    #[inline]
    pub const fn load_rmw<O: LoadOrdering<P::Value>>(&self, order: O) -> P::Value
    where
        P: [const] FieldPath,
        ContainerRepr<P>: [const] ReadByExchange,
    {
        if Self::IS_RUN_TIME || <<P::Value as Atom>::Repr as Primitive>::IS_BITS_EXACT {
            let placement = self.run_time_placement();
            let bits = ContainerRepr::<P>::load_bits_by_exchange(self.cell(), O::CORE);
            // SAFETY: by `Atomic`'s field INVARIANT, the repr whose bits the cell holds decodes.
            unsafe { Self::decode_bits(placement, bits) }
        } else {
            let repr = self.atomic.load_rmw_repr(order);
            // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
            unsafe { decode_repr::<P>(repr) }
        }
    }

    /// Replaces the field with `f` of it, and returns the container before.
    ///
    /// A compare-exchange loop on the whole word, as [`Atomic::update`] is, which leaves every
    /// other field as it finds it.
    #[expect(unsafe_code, reason = "decodes reprs read from the cell")]
    #[inline]
    pub const fn update<
        S: RmwOrdering<FieldOf<P::Container>>,
        F: LoadOrdering<P::Container>,
        U: [const] FnMut(P::Value) -> P::Value + [const] Destruct,
    >(
        &self, set_order: S, fetch_order: F, mut f: U,
    ) -> P::Container
    where
        P: [const] FieldPath,
        <P::Value as Atom>::Repr: [const] ExactBits,
        ContainerRepr<P>: [const] CompareExchange,
    {
        let placement = self.run_time_placement();
        let replace_field = const |bits: u128| {
            // SAFETY: by `Atomic`'s field INVARIANT, the repr whose bits these are decodes.
            let value = f(unsafe { Self::decode_bits(placement, bits) }).to_repr().to_bits();
            Some(Self::replace(placement, bits, value))
        };
        // SAFETY: each repr written is one read from the cell, which decodes, by `Atomic`'s field
        // INVARIANT, with the field's bits replaced by a repr of a value of it, so it decodes, by
        // `FieldPath`'s promise, or at a run-time place by the tail's INVARIANT.
        let replaced =
            unsafe { self.atomic.try_update_bits(set_order, fetch_order, replace_field) };
        match replaced {
            // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
            Ok(before) | Err(before) => unsafe { P::Container::from_repr_unchecked(before) },
        }
    }

    /// As [`update`](Self::update), stopping without a write when `f` returns `None`.
    ///
    /// # Errors
    /// The container seen, when `f` declined its field.
    #[doc(alias = "fetch_update")]
    #[expect(unsafe_code, reason = "decodes reprs read from the cell")]
    #[inline]
    pub const fn try_update<
        S: RmwOrdering<FieldOf<P::Container>>,
        F: LoadOrdering<P::Container>,
        U: [const] FnMut(P::Value) -> Option<P::Value> + [const] Destruct,
    >(
        &self, set_order: S, fetch_order: F, mut f: U,
    ) -> Result<P::Container, P::Container>
    where
        P: [const] FieldPath,
        <P::Value as Atom>::Repr: [const] ExactBits,
        ContainerRepr<P>: [const] CompareExchange,
    {
        let placement = self.run_time_placement();
        let replace_field = const |bits: u128| {
            // SAFETY: by `Atomic`'s field INVARIANT, the repr whose bits these are decodes.
            let Some(value) = f(unsafe { Self::decode_bits(placement, bits) }) else {
                return None;
            };
            Some(Self::replace(placement, bits, value.to_repr().to_bits()))
        };
        // SAFETY: as in `update`.
        let replaced =
            unsafe { self.atomic.try_update_bits(set_order, fetch_order, replace_field) };
        match replaced {
            // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
            Ok(before) => Ok(unsafe { P::Container::from_repr_unchecked(before) }),
            // SAFETY: as above.
            Err(seen) => Err(unsafe { P::Container::from_repr_unchecked(seen) }),
        }
    }

    /// Turns the bit on: `lock or` on `x86_64`, `ldset` on `aarch64`.
    #[inline]
    pub const fn set<O: RmwOrdering<P::Value>>(&self, order: O)
    where
        P: [const] FieldPath<Value = bool>,
        ContainerRepr<P>: [const] MaskBitwise,
    {
        self.or(true, order);
    }

    /// Turns the bit off: `lock and` on `x86_64`, `ldclr` on `aarch64`.
    #[inline]
    pub const fn clear<O: RmwOrdering<P::Value>>(&self, order: O)
    where
        P: [const] FieldPath<Value = bool>,
        ContainerRepr<P>: [const] MaskBitwise,
    {
        self.and(false, order);
    }

    /// Inverts the bit: `lock xor` on `x86_64`, `ldeor` on `aarch64`.
    #[inline]
    pub const fn toggle<O: RmwOrdering<P::Value>>(&self, order: O)
    where
        P: [const] FieldPath<Value = bool>,
        ContainerRepr<P>: [const] MaskBitwise,
    {
        self.xor(true, order);
    }

    /// Writes `value` to the bit: [`set`](Self::set) or [`clear`](Self::clear).
    ///
    /// A plain store of the field alone would race the other fields' read-modify-writes, so this
    /// is a read-modify-write of the whole word too, taking a store's orderings.
    #[inline]
    pub const fn store<O: StoreOrdering<P::Value>>(&self, value: bool, order: O)
    where
        P: [const] FieldPath<Value = bool>,
        ContainerRepr<P>: [const] MaskBitwise,
    {
        let _ = order;
        let cell = self.cell();
        if value {
            ContainerRepr::<P>::or_mask(cell, self.operand(true), O::CORE);
        } else {
            ContainerRepr::<P>::and_mask(cell, self.and_operand(false), O::CORE);
        }
    }

    /// Turns the bit on, and returns it before: `lock bts` on `x86_64`, `ldset` on `aarch64`.
    ///
    /// `x86_64` has no 8-bit `lock bts`, so a container of 8 bits has it on `aarch64` alone.
    #[must_use = "to discard the bit before, call `set`, which every target has"]
    #[inline]
    pub const fn test_and_set<O: RmwOrdering<P::Value>>(&self, order: O) -> bool
    where
        P: FieldPath<Value = bool>,
        ContainerRepr<P>: [const] BitTest,
    {
        let _ = order;
        if Self::IS_RUN_TIME {
            let offset = self.run_time_placement().offset;
            ContainerRepr::<P>::test_and_set_bit(self.cell(), offset, O::CORE)
        } else {
            ContainerRepr::<P>::test_and_set_field::<P>(self.cell(), O::CORE)
        }
    }

    /// Turns the bit off, and returns it before: `lock btr` on `x86_64`, `ldclr` on `aarch64`.
    ///
    /// As [`test_and_set`](Self::test_and_set), on the containers it takes.
    #[must_use = "to discard the bit before, call `clear`, which every target has"]
    #[inline]
    pub const fn test_and_clear<O: RmwOrdering<P::Value>>(&self, order: O) -> bool
    where
        P: FieldPath<Value = bool>,
        ContainerRepr<P>: [const] BitTest,
    {
        let _ = order;
        if Self::IS_RUN_TIME {
            let offset = self.run_time_placement().offset;
            ContainerRepr::<P>::test_and_clear_bit(self.cell(), offset, O::CORE)
        } else {
            ContainerRepr::<P>::test_and_clear_field::<P>(self.cell(), O::CORE)
        }
    }

    /// Inverts the bit, and returns it before: `lock btc` on `x86_64`, `ldeor` on `aarch64`.
    ///
    /// As [`test_and_set`](Self::test_and_set), on the containers it takes.
    #[must_use = "to discard the bit before, call `toggle`, which every target has"]
    #[inline]
    pub const fn test_and_toggle<O: RmwOrdering<P::Value>>(&self, order: O) -> bool
    where
        P: FieldPath<Value = bool>,
        ContainerRepr<P>: [const] BitTest,
    {
        let _ = order;
        if Self::IS_RUN_TIME {
            let offset = self.run_time_placement().offset;
            ContainerRepr::<P>::test_and_toggle_bit(self.cell(), offset, O::CORE)
        } else {
            ContainerRepr::<P>::test_and_toggle_field::<P>(self.cell(), O::CORE)
        }
    }

    /// Applies `& value` to the field: [`fetch_and`](Self::fetch_and) with the container before
    /// discarded, which every target runs as one instruction, as [`Atomic::and`] says.
    ///
    /// Its operand keeps every other bit, so a field whose zero does not decode stays as it is.
    #[inline]
    pub const fn and<O: RmwOrdering<P::Value>>(&self, value: P::Value, order: O)
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: [const] MaskBitwise,
    {
        let _ = order;
        ContainerRepr::<P>::and_mask(self.cell(), self.and_operand(value), O::CORE);
    }

    /// Applies `| value` to the field: [`fetch_or`](Self::fetch_or) with the container before
    /// discarded, at [`and`](Self::and)'s cost.
    #[inline]
    pub const fn or<O: RmwOrdering<P::Value>>(&self, value: P::Value, order: O)
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: [const] MaskBitwise,
    {
        let _ = order;
        ContainerRepr::<P>::or_mask(self.cell(), self.operand(value), O::CORE);
    }

    /// Applies `^ value` to the field: [`fetch_xor`](Self::fetch_xor) with the container before
    /// discarded, at [`and`](Self::and)'s cost.
    #[inline]
    pub const fn xor<O: RmwOrdering<P::Value>>(&self, value: P::Value, order: O)
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: [const] MaskBitwise,
    {
        let _ = order;
        ContainerRepr::<P>::xor_mask(self.cell(), self.operand(value), O::CORE);
    }

    /// Inverts every bit of the field: [`fetch_not`](Self::fetch_not) with the container before
    /// discarded, at [`and`](Self::and)'s cost.
    #[inline]
    pub const fn not<O: RmwOrdering<P::Value>>(&self, order: O)
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: [const] MaskBitwise,
    {
        let _ = order;
        ContainerRepr::<P>::xor_mask(self.cell(), self.not_operand(), O::CORE);
    }

    /// Applies `& value` to the field, and returns the container before.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[must_use = "to discard the container before, call `and`, which every target has"]
    #[inline]
    pub const fn fetch_and<O: RmwOrdering<FieldOf<P::Container>>>(
        &self, value: P::Value, order: O,
    ) -> P::Container
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: [const] MaskBitwise + FetchBitwise,
    {
        let _ = order;
        let before =
            ContainerRepr::<P>::fetch_and_mask(self.cell(), self.and_operand(value), O::CORE);
        // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
        unsafe { P::Container::from_repr_unchecked(before) }
    }

    /// Applies `| value` to the field, and returns the container before.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[must_use = "to discard the container before, call `or`, which every target has"]
    #[inline]
    pub const fn fetch_or<O: RmwOrdering<FieldOf<P::Container>>>(
        &self, value: P::Value, order: O,
    ) -> P::Container
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: [const] MaskBitwise + FetchBitwise,
    {
        let _ = order;
        let before = ContainerRepr::<P>::fetch_or_mask(self.cell(), self.operand(value), O::CORE);
        // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
        unsafe { P::Container::from_repr_unchecked(before) }
    }

    /// Applies `^ value` to the field, and returns the container before.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[must_use = "to discard the container before, call `xor`, which every target has"]
    #[inline]
    pub const fn fetch_xor<O: RmwOrdering<FieldOf<P::Container>>>(
        &self, value: P::Value, order: O,
    ) -> P::Container
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: [const] MaskBitwise + FetchBitwise,
    {
        let _ = order;
        let before = ContainerRepr::<P>::fetch_xor_mask(self.cell(), self.operand(value), O::CORE);
        // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
        unsafe { P::Container::from_repr_unchecked(before) }
    }

    /// Inverts every bit of the field, and returns the container before.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[must_use = "to discard the container before, call `not`, which every target has"]
    #[inline]
    pub const fn fetch_not<O: RmwOrdering<FieldOf<P::Container>>>(&self, order: O) -> P::Container
    where
        P: [const] FieldPath,
        P::Value: FieldBitwise,
        ContainerRepr<P>: [const] MaskBitwise + FetchBitwise,
    {
        let _ = order;
        let before = ContainerRepr::<P>::fetch_xor_mask(self.cell(), self.not_operand(), O::CORE);
        // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
        unsafe { P::Container::from_repr_unchecked(before) }
    }

    /// Adds `delta` to the field, wrapping, and returns the container before: `lock xadd` on
    /// `x86_64`, `ldadd` on `aarch64`.
    ///
    /// The field ends at the repr's top bit, or in a slot that does ([`TopField`]), so no carry
    /// out of it reaches another field.
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[inline]
    pub const fn fetch_add<O: RmwOrdering<FieldOf<P::Container>>>(
        &self, delta: <P::Value as Atom>::Repr, order: O,
    ) -> P::Container
    where
        P: TopField + [const] FieldPath,
        P::Value: FieldAdd,
        ContainerRepr<P>: [const] FetchAdd,
    {
        let _ = order;
        let delta = ContainerRepr::<P>::from_bits(pack::<P>(delta.to_bits()));
        let before = ContainerRepr::<P>::fetch_add(self.cell(), delta, O::CORE);
        // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
        unsafe { P::Container::from_repr_unchecked(before) }
    }

    /// Subtracts `delta` from the field, wrapping, and returns the container before: `lock xadd`
    /// on `x86_64`, `ldadd` on `aarch64`, after a negation.
    ///
    /// As [`fetch_add`](Self::fetch_add), the borrow leaving the word.
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[inline]
    pub const fn fetch_sub<O: RmwOrdering<FieldOf<P::Container>>>(
        &self, delta: <P::Value as Atom>::Repr, order: O,
    ) -> P::Container
    where
        P: TopField + [const] FieldPath,
        P::Value: FieldAdd,
        ContainerRepr<P>: [const] FetchAdd,
    {
        let _ = order;
        let delta = ContainerRepr::<P>::from_bits(pack::<P>(delta.to_bits()));
        let before = ContainerRepr::<P>::fetch_sub(self.cell(), delta, O::CORE);
        // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
        unsafe { P::Container::from_repr_unchecked(before) }
    }
}

impl<P: FieldPath> fmt::Debug for AtomicField<P>
where
    P::Value: fmt::Debug,
{
    #[expect(unsafe_code, reason = "decodes a field of a repr read from the cell")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let placement = self.run_time_placement();
        let repr = self.atomic.load_or_exchange_repr();
        if Self::IS_RUN_TIME {
            // SAFETY: by `Atomic`'s field INVARIANT, the repr read from the cell decodes.
            unsafe { Self::decode_bits(placement, repr.packed_bits()) }.fmt(f)
        } else {
            // SAFETY: as above.
            unsafe { decode_repr::<P>(repr) }.fmt(f)
        }
    }
}
