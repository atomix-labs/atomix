//! The read-modify-writes that need a capability: add and subtract ([`AtomAdd`]), maximum and
//! minimum ([`AtomOrd`]), and the bitwise operations, a bit's among them ([`AtomBitwise`]).
//!
//! Each bound sits on its method, so a value without the capability gets the capability's own
//! message. `and`, `or` and `xor` and their `fetch_` forms take a value with [`AtomBitwise`], or a
//! raw pointer's `usize` mask, through `TakesBitwiseOperand`. `fetch_max` and `fetch_min`
//! also need the target's [`MinMax`], and the `fetch_` bitwise forms its [`FetchBitwise`]:
//! `aarch64` has both, `x86_64` neither. A bit's set, clear and toggle need the target's
//! [`BitTest`]: both have it from 16 bits, and `aarch64` from 8.

use core::sync::atomic::Ordering as CoreOrdering;

use super::Atomic;
use crate::atom::{Atom, AtomAdd, AtomBitwise, AtomOrd};
use crate::ordering::RmwOrdering;
use crate::primitive::{BitTest, Bitwise, FetchAdd, FetchBitwise, MaskBitwise, MinMax};

/// How an atomic of `T` takes a `V` as the operand of `and`, `or` and `xor`, and of their
/// `fetch_` forms: a value with [`AtomBitwise`] takes another of itself, and a raw pointer to a
/// sized type a `usize` mask on its address, as core's `AtomicPtr` does.
///
/// It has no message of its own, since no call reaches one: each atomic has one impl, from which
/// rustc infers `V`, so an operand of another type is a mismatched type, and a value without
/// `AtomBitwise` reports that trait's message. That needs the trait on the atomic, not the value:
/// rustc reports an outer bound's message in place of the unmet one's where the two have one self
/// type.
///
/// # Safety
/// Each method applies its operation to the atomic's cell, leaving a repr that decodes, as
/// `Atomic`'s field INVARIANT asks, and returns the repr the cell held before.
#[doc(hidden)]
#[expect(unsafe_code, reason = "the fetch_ forms decode the repr each method returns")]
pub impl(crate) const unsafe trait TakesBitwiseOperand<T: Atom, V> {
    /// Applies `& operand` to the repr, and returns the repr before.
    fn fetch_and_repr(&self, operand: V, order: CoreOrdering) -> T::Repr;
    /// Applies `| operand` to the repr, and returns the repr before.
    fn fetch_or_repr(&self, operand: V, order: CoreOrdering) -> T::Repr;
    /// Applies `^ operand` to the repr, and returns the repr before.
    fn fetch_xor_repr(&self, operand: V, order: CoreOrdering) -> T::Repr;
}

// A value with `AtomBitwise` takes another of itself, whose repr is the operand.
//
// SAFETY: each method is its repr's own operation on the cell, which returns the repr before, and
// every repr of an `AtomBitwise` value decodes (`Total`), so the one it leaves does.
#[expect(unsafe_code, reason = "`Total` keeps the promise")]
const unsafe impl<T: AtomBitwise + [const] Atom> TakesBitwiseOperand<T, T> for Atomic<T>
where
    T::Repr: [const] Bitwise,
{
    #[inline]
    fn fetch_and_repr(&self, operand: T, order: CoreOrdering) -> T::Repr {
        T::Repr::fetch_and(self.primitive_cell(), operand.to_repr(), order)
    }
    #[inline]
    fn fetch_or_repr(&self, operand: T, order: CoreOrdering) -> T::Repr {
        T::Repr::fetch_or(self.primitive_cell(), operand.to_repr(), order)
    }
    #[inline]
    fn fetch_xor_repr(&self, operand: T, order: CoreOrdering) -> T::Repr {
        T::Repr::fetch_xor(self.primitive_cell(), operand.to_repr(), order)
    }
}

/// Implements `TakesBitwiseOperand<$pointer, usize>` for `Atomic<$pointer>`, an atomic raw
/// pointer to a sized `P`, whose repr is `*mut P`: the mask goes on its address through
/// `MaskBitwise`, core's `AtomicPtr`'s `fetch_or` and its kin, which keep its provenance. Not
/// `const`, as core's are not.
macro_rules! pointer_masks {
    ($($pointer:ty),+) => {$(
        // SAFETY: each method is core's operation on the cell, which returns the pointer before,
        // and every pointer decodes (`Total`), so the one it leaves does.
        #[expect(unsafe_code, reason = "`Total` keeps the promise")]
        unsafe impl<P> TakesBitwiseOperand<$pointer, usize> for Atomic<$pointer> {
            #[inline]
            fn fetch_and_repr(&self, mask: usize, order: CoreOrdering) -> *mut P {
                <*mut P>::fetch_and_mask(self.primitive_cell(), mask, order)
            }
            #[inline]
            fn fetch_or_repr(&self, mask: usize, order: CoreOrdering) -> *mut P {
                <*mut P>::fetch_or_mask(self.primitive_cell(), mask, order)
            }
            #[inline]
            fn fetch_xor_repr(&self, mask: usize, order: CoreOrdering) -> *mut P {
                <*mut P>::fetch_xor_mask(self.primitive_cell(), mask, order)
            }
        }
    )+};
}

pointer_masks!(*mut P, *const P);

impl<T: Atom> Atomic<T> {
    /// Adds `delta` to the repr, wrapping, and returns the value before.
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[inline]
    pub const fn fetch_add<O: RmwOrdering>(&self, delta: T::Repr, order: O) -> T
    where
        T: AtomAdd + [const] Atom,
        T::Repr: [const] FetchAdd,
    {
        let _ = order;
        let before = T::Repr::fetch_add(self.primitive_cell(), delta, O::CORE);
        // SAFETY: by the field INVARIANT, the repr read from the cell decodes.
        unsafe { T::from_repr_unchecked(before) }
    }

    /// Subtracts `delta` from the repr, wrapping, and returns the value before.
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[inline]
    pub const fn fetch_sub<O: RmwOrdering>(&self, delta: T::Repr, order: O) -> T
    where
        T: AtomAdd + [const] Atom,
        T::Repr: [const] FetchAdd,
    {
        let _ = order;
        let before = T::Repr::fetch_sub(self.primitive_cell(), delta, O::CORE);
        // SAFETY: by the field INVARIANT, the repr read from the cell decodes.
        unsafe { T::from_repr_unchecked(before) }
    }

    /// Keeps the larger of `value` and the current value, and returns the value before.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[inline]
    pub const fn fetch_max<O: RmwOrdering>(&self, value: T, order: O) -> T
    where
        T: AtomOrd + [const] Atom,
        T::Repr: [const] MinMax,
    {
        let _ = order;
        let before = T::Repr::fetch_max(self.primitive_cell(), value.to_repr(), O::CORE);
        // SAFETY: by the field INVARIANT, the repr read from the cell decodes.
        unsafe { T::from_repr_unchecked(before) }
    }

    /// Keeps the smaller of `value` and the current value, and returns the value before.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[inline]
    pub const fn fetch_min<O: RmwOrdering>(&self, value: T, order: O) -> T
    where
        T: AtomOrd + [const] Atom,
        T::Repr: [const] MinMax,
    {
        let _ = order;
        let before = T::Repr::fetch_min(self.primitive_cell(), value.to_repr(), O::CORE);
        // SAFETY: by the field INVARIANT, the repr read from the cell decodes.
        unsafe { T::from_repr_unchecked(before) }
    }

    /// Applies `& operand`, where `T` has [`AtomBitwise`] or is a raw pointer to a sized type:
    /// [`fetch_and`](Self::fetch_and) with the value before discarded.
    ///
    /// The operand is another `T`, or for a raw pointer a `usize` mask on its address, as core's
    /// [`AtomicPtr::fetch_and`] takes, which keeps the pointer's provenance; a pointer's is not
    /// `const`, as core's is not.
    ///
    /// Discarding the value before is what gives `and`, `or`, `xor` and `not` to every target:
    /// `x86_64` runs each as one `lock` instruction, which cannot return the value before. That
    /// needs an optimized build; unoptimized, `x86_64` compiles each as a compare-exchange loop.
    ///
    /// These four alone have such a form, under [portable-atomic]'s names, since no other
    /// read-modify-write needs one: a [`fetch_add`](Self::fetch_add) whose value goes unused is
    /// already `lock add` on `x86_64`.
    ///
    /// # Examples
    /// ```
    /// # extern crate atomix_core as atomix;
    /// use core::ptr;
    ///
    /// use atomix::ordering::{AcqRel, Acquire};
    /// use atomix::{AtomicPtr, AtomicU64};
    ///
    /// // Order flags: bit 1 marks an order as hidden.
    /// let flags = AtomicU64::new(0b11);
    /// flags.and(!0b10, AcqRel);
    /// assert_eq!(flags.load(Acquire), 0b01, "the hidden flag cleared");
    ///
    /// // A node's pointer, its low bit a mark the node's alignment leaves free.
    /// let mut node = 7_u64;
    /// let next = AtomicPtr::new(ptr::from_mut(&mut node));
    /// next.or(1, AcqRel);
    /// next.and(!1, AcqRel);
    /// assert_eq!(next.load(Acquire), ptr::from_mut(&mut node), "marked, then unmarked");
    /// ```
    ///
    /// [`AtomicPtr::fetch_and`]: core::sync::atomic::AtomicPtr::fetch_and
    /// [portable-atomic]: https://docs.rs/portable-atomic
    #[inline]
    pub const fn and<V, O: RmwOrdering>(&self, operand: V, order: O)
    where
        Self: [const] TakesBitwiseOperand<T, V>,
    {
        let _ = order;
        <Self as TakesBitwiseOperand<T, V>>::fetch_and_repr(self, operand, O::CORE);
    }

    /// Applies `| operand`, where `T` has [`AtomBitwise`] or is a raw pointer to a sized type:
    /// [`fetch_or`](Self::fetch_or) with the value before discarded.
    ///
    /// Its operands and its cost are [`and`](Self::and)'s.
    #[inline]
    pub const fn or<V, O: RmwOrdering>(&self, operand: V, order: O)
    where
        Self: [const] TakesBitwiseOperand<T, V>,
    {
        let _ = order;
        <Self as TakesBitwiseOperand<T, V>>::fetch_or_repr(self, operand, O::CORE);
    }

    /// Applies `^ operand`, where `T` has [`AtomBitwise`] or is a raw pointer to a sized type:
    /// [`fetch_xor`](Self::fetch_xor) with the value before discarded.
    ///
    /// Its operands and its cost are [`and`](Self::and)'s.
    #[inline]
    pub const fn xor<V, O: RmwOrdering>(&self, operand: V, order: O)
    where
        Self: [const] TakesBitwiseOperand<T, V>,
    {
        let _ = order;
        <Self as TakesBitwiseOperand<T, V>>::fetch_xor_repr(self, operand, O::CORE);
    }

    /// Inverts every bit: [`fetch_not`](Self::fetch_not) with the value before discarded, at
    /// [`and`](Self::and)'s cost.
    #[inline]
    pub const fn not<O: RmwOrdering>(&self, order: O)
    where
        T: AtomBitwise + [const] Atom,
        T::Repr: [const] Bitwise,
    {
        let _ = order;
        T::Repr::fetch_not(self.primitive_cell(), O::CORE);
    }

    /// Applies `& operand`, where `T` has [`AtomBitwise`] or is a raw pointer to a sized type, and
    /// returns the value before.
    ///
    /// Its operands are [`and`](Self::and)'s.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[must_use = "to discard the value before, call `and`, which every target has"]
    #[inline]
    pub const fn fetch_and<V, O: RmwOrdering>(&self, operand: V, order: O) -> T
    where
        T: [const] Atom,
        Self: [const] TakesBitwiseOperand<T, V>,
        T::Repr: FetchBitwise,
    {
        let _ = order;
        let before = <Self as TakesBitwiseOperand<T, V>>::fetch_and_repr(self, operand, O::CORE);
        // SAFETY: `before` is the repr the cell held, by `TakesBitwiseOperand`'s promise, which
        // decodes by the field INVARIANT.
        unsafe { T::from_repr_unchecked(before) }
    }

    /// Applies `| operand`, where `T` has [`AtomBitwise`] or is a raw pointer to a sized type, and
    /// returns the value before.
    ///
    /// Its operands are [`and`](Self::and)'s.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[must_use = "to discard the value before, call `or`, which every target has"]
    #[inline]
    pub const fn fetch_or<V, O: RmwOrdering>(&self, operand: V, order: O) -> T
    where
        T: [const] Atom,
        Self: [const] TakesBitwiseOperand<T, V>,
        T::Repr: FetchBitwise,
    {
        let _ = order;
        let before = <Self as TakesBitwiseOperand<T, V>>::fetch_or_repr(self, operand, O::CORE);
        // SAFETY: `before` is the repr the cell held, by `TakesBitwiseOperand`'s promise, which
        // decodes by the field INVARIANT.
        unsafe { T::from_repr_unchecked(before) }
    }

    /// Applies `^ operand`, where `T` has [`AtomBitwise`] or is a raw pointer to a sized type, and
    /// returns the value before.
    ///
    /// Its operands are [`and`](Self::and)'s.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[must_use = "to discard the value before, call `xor`, which every target has"]
    #[inline]
    pub const fn fetch_xor<V, O: RmwOrdering>(&self, operand: V, order: O) -> T
    where
        T: [const] Atom,
        Self: [const] TakesBitwiseOperand<T, V>,
        T::Repr: FetchBitwise,
    {
        let _ = order;
        let before = <Self as TakesBitwiseOperand<T, V>>::fetch_xor_repr(self, operand, O::CORE);
        // SAFETY: `before` is the repr the cell held, by `TakesBitwiseOperand`'s promise, which
        // decodes by the field INVARIANT.
        unsafe { T::from_repr_unchecked(before) }
    }

    /// Inverts every bit, and returns the value before.
    #[doc(cfg(any(target_arch = "aarch64", target_arch = "arm64ec")))]
    #[expect(unsafe_code, reason = "decodes a repr read from the cell")]
    #[must_use = "to discard the value before, call `not`, which every target has"]
    #[inline]
    pub const fn fetch_not<O: RmwOrdering>(&self, order: O) -> T
    where
        T: AtomBitwise + [const] Atom,
        T::Repr: [const] Bitwise + FetchBitwise,
    {
        let _ = order;
        let before = T::Repr::fetch_not(self.primitive_cell(), O::CORE);
        // SAFETY: by the field INVARIANT, the repr read from the cell decodes.
        unsafe { T::from_repr_unchecked(before) }
    }

    /// Turns bit `bit` of the repr on, and returns it before: `lock bts` on `x86_64`, `ldset` on
    /// `aarch64`.
    ///
    /// `bit` counts from the repr's lowest bit, modulo its width, as a shift by
    /// [`wrapping_shl`](u64::wrapping_shl) does, so bit 67 of a `u64` is bit 3, and every bit of a
    /// `bool` is its one bit. `x86_64` has no 8-bit `lock bts`, so an 8-bit repr or a `bool` has it
    /// on `aarch64` alone.
    ///
    /// It takes its name from [portable-atomic]'s `bit_set`, and so returns the bit before as that
    /// does. The atomic's own [`set`](Self::set) writes a whole value through `&mut`. A `bool`
    /// field's [`set`](crate::AtomicField::set) returns nothing, since the field's form that
    /// returns the bit is [`test_and_set`](crate::AtomicField::test_and_set); an atomic's form that
    /// discards it is [`or`](Self::or).
    ///
    /// # Examples
    /// ```
    /// # extern crate atomix_core as atomix;
    /// use atomix::AtomicU64;
    /// use atomix::ordering::AcqRel;
    ///
    /// // A bit per worker of a pool, set while the worker is busy.
    /// static BUSY: AtomicU64 = AtomicU64::new(0);
    ///
    /// let worker = 5;
    /// assert!(!BUSY.bit_set(worker, AcqRel), "worker 5 was idle, and this thread took it");
    /// assert!(BUSY.bit_set(worker, AcqRel), "so a second claim finds it busy");
    /// ```
    ///
    /// [portable-atomic]: https://docs.rs/portable-atomic
    #[must_use = "to discard the bit before, call `or`, which every target has"]
    #[inline]
    pub const fn bit_set<O: RmwOrdering>(&self, bit: u32, order: O) -> bool
    where
        T: AtomBitwise,
        T::Repr: [const] BitTest,
    {
        let _ = order;
        T::Repr::test_and_set_bit(self.primitive_cell(), bit, O::CORE)
    }

    /// Turns bit `bit` of the repr off, and returns it before: `lock btr` on `x86_64`, `ldclr` on
    /// `aarch64`.
    ///
    /// As [`bit_set`](Self::bit_set), on the reprs it takes; [`and`](Self::and) is the form that
    /// discards the bit.
    ///
    /// # Examples
    /// ```
    /// # extern crate atomix_core as atomix;
    /// use atomix::AtomicU64;
    /// use atomix::ordering::{AcqRel, Acquire};
    ///
    /// // A bit per worker of a pool, set while the worker is busy: worker 5 is.
    /// static BUSY: AtomicU64 = AtomicU64::new(1 << 5);
    ///
    /// assert!(BUSY.bit_clear(5, AcqRel), "worker 5 was busy, and this thread freed it");
    /// assert_eq!(BUSY.load(Acquire), 0, "so every worker is idle");
    /// ```
    #[must_use = "to discard the bit before, call `and`, which every target has"]
    #[inline]
    pub const fn bit_clear<O: RmwOrdering>(&self, bit: u32, order: O) -> bool
    where
        T: AtomBitwise,
        T::Repr: [const] BitTest,
    {
        let _ = order;
        T::Repr::test_and_clear_bit(self.primitive_cell(), bit, O::CORE)
    }

    /// Inverts bit `bit` of the repr, and returns it before: `lock btc` on `x86_64`, `ldeor` on
    /// `aarch64`.
    ///
    /// As [`bit_set`](Self::bit_set), on the reprs it takes; [`xor`](Self::xor) is the form that
    /// discards the bit.
    ///
    /// # Examples
    /// ```
    /// # extern crate atomix_core as atomix;
    /// use atomix::AtomicU32;
    /// use atomix::ordering::AcqRel;
    ///
    /// // A bit per shard of a cache, the parity of the shard's epoch.
    /// static PARITY: AtomicU32 = AtomicU32::new(0);
    ///
    /// let shard = 3;
    /// assert!(!PARITY.bit_toggle(shard, AcqRel), "shard 3 left an even epoch");
    /// assert!(PARITY.bit_toggle(shard, AcqRel), "and then the odd one it entered");
    /// ```
    #[must_use = "to discard the bit before, call `xor`, which every target has"]
    #[inline]
    pub const fn bit_toggle<O: RmwOrdering>(&self, bit: u32, order: O) -> bool
    where
        T: AtomBitwise,
        T::Repr: [const] BitTest,
    {
        let _ = order;
        T::Repr::test_and_toggle_bit(self.primitive_cell(), bit, O::CORE)
    }
}
