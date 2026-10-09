//! `Atom` for a shared reference and for a function pointer, each stored as a [`ReprOf`] of
//! itself, which no bits forge.
//!
//! The two blanket impls sit side by side through negative coherence: rustc proves `&T: !FnPtr`,
//! as no reference is a function pointer, where without it a downstream crate might implement
//! `FnPtr` for a reference to its own type.

#![expect(unsafe_code, reason = "each `Atom` impl here promises what loads rely on")]

use core::ops::FnPtr;
use core::ptr::{self, NonNull};

use super::Atom;
use super::ptr::{Metadata, PointerMetadata};
use crate::primitive::ReprOf;
use crate::range::{PointeeAlignment, ReprRange};
use crate::validity::TotalZeroNiche;

/// Stored as a raw pointer to the same `T` is, the pointer alone where `T` is sized, else in a
/// `DoubleWord`, inside a [`ReprOf`], which only a reference gives, so a decode hands out no
/// reference that bits made. Null is no reference's, so an `Option` of one is free.
///
/// `T` is `Sync`, so the reference may cross threads. A load acquires and a store releases, as
/// [`LoadOrdering`](crate::ordering::LoadOrdering) and its siblings ask of a value that holds a
/// reference, so a thread that loads it sees the writes that made what it points to.
// SAFETY: the repr is the reference's pointer, never null, so within every repr but zero; it is
// sealed, so every repr the cell holds is null or one a `&'a T` gave, by `ReprOf`'s INVARIANT, and
// `from_repr` decodes each but null as that reference, as `TotalZeroNiche` says, by the repr alone,
// and the default `from_repr_unchecked` unwraps it; `T: Sync`, so a `&'a T` may cross threads; the
// floor of `ReprOf<&'a T>`, an `Acquire` read and a `Release` write, orders the writes that made
// `*self` before every read through a reference a load hands out; and the default `to_tagged_repr`
// keeps its promise.
#[diagnostic::do_not_recommend]
const unsafe impl<'a, T: ?Sized + Sync> Atom for &'a T
where
    Metadata<T>: [const] PointerMetadata<T>,
{
    type Repr = ReprOf<&'a T>;
    type Validity = TotalZeroNiche;
    const REPRS: ReprRange<Self::Repr> = ReprRange::NON_ZERO;
    const POINTEE_ALIGNMENT: PointeeAlignment =
        <Metadata<T> as PointerMetadata<T>>::POINTEE_ALIGNMENT;
    #[inline]
    fn to_repr(self) -> Self::Repr {
        let pointer = <Metadata<T> as PointerMetadata<T>>::to_repr(ptr::from_ref(self).cast_mut());
        // SAFETY: the repr of `self`'s own pointer, its provenance kept.
        unsafe { ReprOf::new(pointer) }
    }
    #[inline]
    fn from_repr(repr: Self::Repr) -> Option<Self> {
        match <NonNull<T> as Atom>::from_repr(repr.repr()) {
            // SAFETY: by `ReprOf`'s INVARIANT, a repr that is not null is one a `&'a T` gave, its
            // provenance kept: it points to a `T` that lives, changed only through its own
            // interior mutability, for all of `'a`.
            Some(pointer) => Some(unsafe { pointer.as_ref() }),
            None => None,
        }
    }
}

// By hand, not `FnPtr::as_ptr` and `FnPtr::from_ptr`, which name the conversion but are not
// `const`, as this impl's `to_repr` and `from_repr` are.
/// The bytes of a function pointer, read as a data pointer's, provenance and all.
union Pun<F: Copy> {
    /// The function pointer.
    function: F,
    /// The same pointer, as data.
    pointer: *mut (),
}

/// Stored as its code pointer, inside a [`ReprOf`], which only a function pointer of the same type
/// gives, so a decode hands out no function pointer that bits made, nor one of another signature.
/// Null is no function's, so an `Option` of one is free.
///
/// Every signature, a higher-ranked one such as `fn(&str) -> usize` among them, every ABI and
/// both safeties. Its operations take every ordering: it points to code, which no write changes.
///
/// A compare-exchange compares addresses, and one function may have several, or two functions
/// one: take `current` from a load or a failed exchange, never from a function named afresh.
// SAFETY: the repr is the function's pointer, never null, so within every repr but zero; it is
// sealed, so every repr the cell holds is null or one an `F` gave, by `ReprOf`'s INVARIANT, and
// `from_repr` decodes each but null as that function pointer, as `TotalZeroNiche` says, by the
// repr alone, and the default `from_repr_unchecked` unwraps it; a function pointer may cross
// threads, as it is `Send` and `Sync`; and the default `to_tagged_repr` keeps its promise.
//
// A bare `F` makes this impl a candidate for every type, so where a value's own impl fails on a
// value it holds, rustc names the outer value, `Option<String>`, not the inner one, `String`:
// `do_not_recommend` changes none of those refusals, so the impl goes without it.
const unsafe impl<F: FnPtr> Atom for F {
    type Repr = ReprOf<F>;
    type Validity = TotalZeroNiche;
    const REPRS: ReprRange<Self::Repr> = ReprRange::NON_ZERO;
    #[inline]
    fn to_repr(self) -> Self::Repr {
        const {
            assert!(size_of::<F>() == size_of::<*mut ()>(), "a function pointer is one word");
        };
        // SAFETY: `F` is a function pointer, as wide as a data pointer, as the assertion checks,
        // so its bytes are a pointer's, provenance and all, which a union read keeps.
        let pointer = unsafe { Pun { function: self }.pointer };
        // SAFETY: the pointer `self` is, its provenance kept.
        unsafe { ReprOf::new(pointer) }
    }
    #[inline]
    fn from_repr(repr: Self::Repr) -> Option<Self> {
        let pointer = repr.repr();
        if pointer.is_null() {
            return None;
        }
        // SAFETY: by `ReprOf`'s INVARIANT, a pointer that is not null is one an `F` gave, its
        // provenance kept, so its bytes are that `F`'s.
        Some(unsafe { Pun::<F> { pointer }.function })
    }
}
