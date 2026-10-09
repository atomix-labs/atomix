//! Memory orderings as types: each operation names the orderings it accepts, so a wrong one is a
//! compile error rather than a panic.
//!
//! An operation on a value that holds a reference accepts no ordering weaker than an `Acquire` read
//! and a `Release` write, so a thread that loads the reference sees the writes that made what it
//! points to: [`LoadOrdering<T>`](LoadOrdering), [`StoreOrdering<T>`](StoreOrdering) and
//! [`RmwOrdering<T>`](RmwOrdering) take the value, and a value takes the floor of its repr, so an
//! `Option`, a newtype or a struct that holds a reference takes the reference's. Generic code that
//! orders a `T` with `Relaxed` bounds it by [`HoldsNoReference`].
//!
//! Only the types here implement the traits. Each but [`StoreStore`], which core has no name for,
//! converts to core's [`Ordering`](CoreOrdering) through `From`, for code that hands its ordering
//! on to core, std or a C interface; nothing converts back, so an ordering stays a type.

use core::sync::atomic::Ordering as CoreOrdering;

use crate::atom::Atom;
use crate::primitive::Primitive;

/// Orders nothing beyond the operation's own atomicity.
#[derive(Debug, Copy, Hash)]
#[derive_const(Clone, Default, PartialEq, Eq)]
pub struct Relaxed;
/// Sees every write published by the [`Release`] it reads from.
#[derive(Debug, Copy, Hash)]
#[derive_const(Clone, Default, PartialEq, Eq)]
pub struct Acquire;
/// Publishes every earlier write to the [`Acquire`] that reads it.
#[derive(Debug, Copy, Hash)]
#[derive_const(Clone, Default, PartialEq, Eq)]
pub struct Release;
/// [`Acquire`] and [`Release`] at once, for a read-modify-write or a fence.
#[derive(Debug, Copy, Hash)]
#[derive_const(Clone, Default, PartialEq, Eq)]
pub struct AcqRel;
/// [`Acquire`], [`Release`] or both, as the operation allows, in one order of every `SeqCst`
/// operation and fence that all threads agree on.
#[derive(Debug, Copy, Hash)]
#[derive_const(Clone, Default, PartialEq, Eq)]
pub struct SeqCst;
/// A [`fence`](fn@crate::fence) that orders earlier stores before later ones, and nothing else.
#[derive(Debug, Copy, Hash)]
#[derive_const(Clone, Default, PartialEq, Eq)]
pub struct StoreStore;

/// An ordering core's read-modify-write takes, whatever the value, and its spelling there: each
/// ordering but [`StoreStore`], which core has no name for.
#[doc(hidden)]
pub impl(crate) trait CoreRmwOrdering: Copy + Into<CoreOrdering> {
    /// The ordering as `core` (and loom) spell it.
    const CORE: CoreOrdering;
}

/// An ordering core's load takes, and so a compare-exchange's failure, whatever the value:
/// [`Relaxed`], [`Acquire`] or [`SeqCst`].
#[doc(hidden)]
pub impl(crate) trait CoreLoadOrdering: CoreRmwOrdering {}

/// An ordering a read-modify-write of a `T` may take: every ordering but [`StoreStore`], which
/// only a fence takes, and, where `T` holds a reference, [`AcqRel`] or [`SeqCst`].
///
/// A reference read must see the writes that made what it points to, and a reference written must
/// publish them, so an exchange of one both acquires and releases. With no `T`, the orderings a
/// value that holds no reference takes.
///
/// Each converts to core's [`Ordering`](CoreOrdering), so generic code hands one on with `into`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot order a read-modify-write of `{T}`",
    label = "a read-modify-write takes `Relaxed`, `Acquire`, `Release`, `AcqRel` or `SeqCst` from `atomix::ordering`",
    note = "`StoreStore` orders a fence, and nothing else"
)]
pub impl(crate) trait RmwOrdering<T = ()>: CoreRmwOrdering {}

/// An ordering a load of a `T` may take, and so a compare-exchange's failure ordering:
/// [`Relaxed`], [`Acquire`] or [`SeqCst`], and, where `T` holds a reference, `Acquire` or `SeqCst`.
///
/// A reference read must see the writes that made what it points to. With no `T`, the orderings a
/// value that holds no reference takes.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot order a load of `{T}`",
    label = "a load takes `Relaxed`, `Acquire` or `SeqCst` from `atomix::ordering`",
    note = "`Release` and `AcqRel` order a store, and a load has none"
)]
pub impl(crate) trait LoadOrdering<T = ()>: CoreLoadOrdering {}

/// An ordering a store of a `T` may take: [`Relaxed`], [`Release`] or [`SeqCst`], and, where `T`
/// holds a reference, `Release` or `SeqCst`.
///
/// A reference written must publish the writes that made what it points to. With no `T`, the
/// orderings a value that holds no reference takes.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot order a store of `{T}`",
    label = "a store takes `Relaxed`, `Release` or `SeqCst` from `atomix::ordering`",
    note = "`Acquire` and `AcqRel` order a load, and a store has none"
)]
pub impl(crate) trait StoreOrdering<T = ()>: CoreRmwOrdering {}

/// The weakest ordering an operation on a primitive takes: [`Relaxed`], so every one of the
/// operation's kind, or, on a reference's repr, [`AcqRel`], so an `Acquire` read and a `Release`
/// write.
///
/// A primitive's hidden `OrderingFloor` names it, so a value takes its repr's: an `Option`, a
/// newtype or a struct holding a reference takes the reference's.
#[doc(hidden)]
pub impl(crate) trait OrderingFloor {
    /// The `core` ordering of the weakest load the floor takes: what `Debug` and serde's
    /// `Serialize` read with.
    const CORE_LOAD: CoreOrdering;
}

impl OrderingFloor for Relaxed {
    const CORE_LOAD: CoreOrdering = CoreOrdering::Relaxed;
}

impl OrderingFloor for AcqRel {
    const CORE_LOAD: CoreOrdering = CoreOrdering::Acquire;
}

/// An ordering a fence may take.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a fence ordering",
    label = "expected `Acquire`, `Release`, `AcqRel`, `SeqCst` or `StoreStore` from `atomix::ordering`",
    note = "a `Relaxed` fence orders nothing"
)]
pub impl(crate) trait FenceOrdering: Copy {
    /// The `core` fence's ordering that stands for this one: [`StoreStore`]'s is `Release`, the
    /// narrowest that covers it.
    #[doc(hidden)]
    const CORE_FENCE: CoreOrdering;
    /// Whether `fence` emits `aarch64`'s store-store barrier rather than `CORE_FENCE`'s.
    #[doc(hidden)]
    const IS_STORE_STORE: bool = false;
}

/// Implements `$trait` for each ordering, with `$core` the `core` ordering of the same name.
macro_rules! core_orderings {
    ($trait:ident::$core:ident: $($ordering:ident),+) => {$(
        impl $trait for $ordering {
            const $core: CoreOrdering = CoreOrdering::$ordering;
        }
    )+};
}

core_orderings!(CoreRmwOrdering::CORE: Relaxed, Acquire, Release, AcqRel, SeqCst);
core_orderings!(FenceOrdering::CORE_FENCE: Acquire, Release, AcqRel, SeqCst);

/// Implements `From` of each ordering for core's of the same name.
macro_rules! into_core_orderings {
    ($($ordering:ident),+) => {$(
        const impl From<$ordering> for CoreOrdering {
            #[inline]
            fn from(_: $ordering) -> Self {
                Self::$ordering
            }
        }
    )+};
}

into_core_orderings!(Relaxed, Acquire, Release, AcqRel, SeqCst);

impl FenceOrdering for StoreStore {
    const CORE_FENCE: CoreOrdering = CoreOrdering::Release;
    const IS_STORE_STORE: bool = true;
}

impl CoreLoadOrdering for Relaxed {}
impl CoreLoadOrdering for Acquire {}
impl CoreLoadOrdering for SeqCst {}

/// A value that holds no reference, so takes every ordering: one whose repr's floor is
/// [`Relaxed`].
///
/// [`Relaxed`], and [`Acquire`] and [`Release`] in a read-modify-write, order a `T` only where it
/// is one: generic code that orders a `T` with them bounds `T` by it, as rustc suggests, and a
/// reference's refusal names the value and why. Sealed: a value's repr says whether it is one.
///
/// # Examples
/// ```
/// # extern crate atomix_core as atomix;
/// use atomix::ordering::{HoldsNoReference, Relaxed};
/// use atomix::{Atom, Atomic, Load, Swap};
///
/// /// Reads a count of any type that holds no reference, `Relaxed`.
/// fn peek<T: Atom<Repr: Load> + HoldsNoReference>(count: &Atomic<T>) -> T {
///     count.load(Relaxed)
/// }
///
/// /// Resets it, `Relaxed` too: a read-modify-write's `Relaxed` asks the same.
/// fn reset<T: Atom<Repr: Swap> + HoldsNoReference>(count: &Atomic<T>, zero: T) -> T {
///     count.swap(zero, Relaxed)
/// }
///
/// let count = Atomic::new(7_u64);
/// assert_eq!(peek(&count), 7, "read back");
/// assert_eq!(reset(&count, 0), 7, "the count before");
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` can hold a reference, which is read with `Acquire` and written with `Release`",
    label = "too weak for a reference",
    note = "a thread that reads a reference with `Acquire` sees the writes that made what it points to, which the `Release` that wrote it publishes"
)]
pub impl(crate) trait HoldsNoReference {}

// `do_not_recommend`, so a reference's refusal reports this trait's message, which names the value,
// not the floor's mismatch.
#[diagnostic::do_not_recommend]
impl<T: Atom<Repr: Primitive<OrderingFloor = Relaxed>>> HoldsNoReference for T {}

// Written out, not generated: a wrong ordering's error lists these impls, and would show a macro.
// Each impl that takes only a value holding no reference is bounded by `HoldsNoReference`, whose
// message a reference's refusal reports.
impl<T: HoldsNoReference> LoadOrdering<T> for Relaxed {}
impl<T> LoadOrdering<T> for Acquire {}
impl<T> LoadOrdering<T> for SeqCst {}

impl<T: HoldsNoReference> StoreOrdering<T> for Relaxed {}
impl<T> StoreOrdering<T> for Release {}
impl<T> StoreOrdering<T> for SeqCst {}

impl<T: HoldsNoReference> RmwOrdering<T> for Relaxed {}
impl<T: HoldsNoReference> RmwOrdering<T> for Acquire {}
impl<T: HoldsNoReference> RmwOrdering<T> for Release {}
impl<T> RmwOrdering<T> for AcqRel {}
impl<T> RmwOrdering<T> for SeqCst {}

#[cfg(test)]
mod tests {
    use core::marker::Destruct;
    use core::sync::atomic::{AtomicU8, compiler_fence, fence};

    use super::{
        AcqRel, Acquire, CoreOrdering, FenceOrdering, LoadOrdering, Relaxed, Release, RmwOrdering,
        SeqCst, StoreOrdering, StoreStore,
    };
    use crate::atom::Atom;
    use crate::primitive::Primitive;

    /// Compiles only where `T`'s repr's floor is `F`.
    const fn floor_is<T: Atom<Repr: Primitive<OrderingFloor = F>>, F>() {}

    // A value takes its repr's floor.
    const _: () = {
        floor_is::<u64, Relaxed>();
        floor_is::<&u64, AcqRel>();
        floor_is::<Option<&u64>, AcqRel>();
        floor_is::<fn(&str) -> usize, Relaxed>();
        floor_is::<Option<unsafe extern "C" fn()>, Relaxed>();
    };

    /// Loads with `O`, and fails a compare-exchange with it, on `core`'s atomic, which panics on an
    /// ordering a load refuses.
    fn load_on_core<O: LoadOrdering>() {
        let atomic = AtomicU8::new(1);
        assert_eq!(atomic.load(O::CORE), 1, "the load reads the value");
        assert_eq!(
            atomic.compare_exchange(0, 2, CoreOrdering::SeqCst, O::CORE),
            Err(1),
            "the compare-exchange fails and reads the value"
        );
    }

    /// Stores with `O` on `core`'s atomic, which panics on an ordering a store refuses.
    fn store_on_core<O: StoreOrdering>() {
        let atomic = AtomicU8::new(0);
        atomic.store(1, O::CORE);
        assert_eq!(atomic.into_inner(), 1, "the store writes the value");
    }

    /// Fences with `O`'s `core` ordering, which `core` panics on if a fence refuses it.
    fn fence_on_core<O: FenceOrdering>() {
        fence(O::CORE_FENCE);
        compiler_fence(O::CORE_FENCE);
    }

    /// Whether `ordering` equals its clone and its type's default, each run as `const`: through a
    /// bound, so no lint rewrites a clone or a default of a unit into the unit.
    const fn is_its_clone_and_default<O>(ordering: &O) -> bool
    where
        O: [const] Clone + [const] Default + [const] PartialEq + [const] Destruct,
    {
        ordering.clone() == *ordering && O::default() == *ordering
    }

    // Each ordering compares, clones and defaults in a constant.
    const _: () = {
        assert!(
            is_its_clone_and_default(&Relaxed),
            "`Relaxed` equals its clone and its default, in const"
        );
        assert!(is_its_clone_and_default(&Acquire), "`Acquire`");
        assert!(is_its_clone_and_default(&Release), "`Release`");
        assert!(is_its_clone_and_default(&AcqRel), "`AcqRel`");
        assert!(is_its_clone_and_default(&SeqCst), "`SeqCst`");
        assert!(is_its_clone_and_default(&StoreStore), "and `StoreStore`");
    };

    // Each ordering converts to core's of its name in a constant.
    const _: () = {
        assert!(
            matches!(CoreOrdering::from(Relaxed), CoreOrdering::Relaxed),
            "`Relaxed` converts to core's of its name, in const"
        );
        assert!(matches!(CoreOrdering::from(Acquire), CoreOrdering::Acquire), "`Acquire`");
        assert!(matches!(CoreOrdering::from(Release), CoreOrdering::Release), "`Release`");
        assert!(matches!(CoreOrdering::from(AcqRel), CoreOrdering::AcqRel), "`AcqRel`");
        assert!(matches!(CoreOrdering::from(SeqCst), CoreOrdering::SeqCst), "and `SeqCst`");
    };

    /// Core's spelling of `order`, through the bound alone.
    fn core_spelling<O: RmwOrdering>(order: O) -> CoreOrdering {
        order.into()
    }

    #[test]
    fn generic_code_hands_an_ordering_on_to_core() {
        assert_eq!(core_spelling(AcqRel), CoreOrdering::AcqRel, "a read-modify-write's");
        assert_eq!(core_spelling(Acquire), CoreOrdering::Acquire, "and a load's");
    }

    #[test]
    fn core_accepts_each_ordering_an_operation_admits() {
        load_on_core::<Relaxed>();
        load_on_core::<Acquire>();
        load_on_core::<SeqCst>();
        store_on_core::<Relaxed>();
        store_on_core::<Release>();
        store_on_core::<SeqCst>();
        fence_on_core::<Acquire>();
        fence_on_core::<Release>();
        fence_on_core::<AcqRel>();
        fence_on_core::<SeqCst>();
        fence_on_core::<StoreStore>();
    }
}
