//! A misaligned tagged pointer's panic reports the line that encoded it: every operation that
//! encodes a value is `#[track_caller]`, down through `Option`, a derived newtype and the derived
//! pointer word or pointer enum, to the one refusal.
//!
//! One test, alone in its binary: it sets the process's panic hook, which nothing else here reads.

#![cfg(feature = "derive")]
// Loom's atomics exist only inside a model.
#![cfg(not(loom))]

#[cfg(test)]
mod tests {
    use core::cell::Cell;
    use core::panic::{Location, UnwindSafe};
    use core::ptr::{self, NonNull};
    use std::panic;

    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic};

    /// A node two-byte aligned, which leaves bit 0 of its address for a tag.
    #[repr(align(2))]
    struct Node;

    /// A Treiber stack's head: its top node, and whether the stack is closed, in bit 0.
    #[derive(Clone, Copy, Debug, Atom)]
    struct Head {
        top: Option<NonNull<Node>>,
        closed: bool,
    }

    /// A head, as a newtype holds it.
    #[derive(Clone, Copy, Debug, Atom)]
    struct Stack(#[atom(pointer)] Head);

    /// A stack whose default head has bit 0 of its pointer set, where its tag goes.
    #[derive(Clone, Copy, Debug, Atom)]
    struct OddStack(#[atom(pointer)] Head);

    impl Default for OddStack {
        fn default() -> Self {
            Self(Head { top: Some(odd()), closed: false })
        }
    }

    /// A pointer at an odd address, which has bit 0 set where a tag goes; nothing reads through it.
    fn odd() -> NonNull<Node> {
        NonNull::new(ptr::without_provenance_mut(0x1001)).expect("not null")
    }

    /// A list's link to a node, never null, so `Option`'s `None` takes the null repr, and whether
    /// its node is deleted.
    #[derive(Clone, Copy, Debug, Atom)]
    struct Link {
        next: NonNull<Node>,
        deleted: bool,
    }

    /// A block four-byte aligned, which leaves two bits for a pointer enum's tag.
    #[repr(align(4))]
    struct Block;

    /// A pair of slots: neither full, or the left or right one, each with a tag of its own.
    #[derive(Clone, Copy, Debug, Atom)]
    enum Slot {
        Empty,
        Left(NonNull<Block>),
        Right(NonNull<Block>),
    }

    thread_local! {
        /// The line the last panic on this thread reported.
        static PANIC_LINE: Cell<Option<u32>> = const { Cell::new(None) };
    }

    /// The line of `$operation`'s call, and the line its panic reports.
    macro_rules! lines {
        ($operation:expr) => {
            (line!(), panic_line(move || _ = $operation))
        };
    }

    /// The line `operation`'s panic reports, which the hook records in place of printing it, or
    /// `None` where it does not panic.
    fn panic_line<F: FnOnce() + UnwindSafe>(operation: F) -> Option<u32> {
        let hook = panic::take_hook();
        panic::set_hook(Box::new(|info| PANIC_LINE.set(info.location().map(Location::line))));
        let result = panic::catch_unwind(operation);
        panic::set_hook(hook);
        result.err().and_then(|_| PANIC_LINE.take())
    }

    #[test]
    fn a_misaligned_pointers_panic_reports_the_line_that_encoded_it() {
        let odd = odd();
        let (aligned, misaligned) =
            (Head { top: None, closed: false }, Head { top: Some(odd), closed: false });
        let head = &Atomic::new(aligned);
        let stack = &Atomic::new(Stack(aligned));
        let link = &Atomic::new(None);
        let slot = &Atomic::new(Slot::Empty);
        let lines = [
            ("new", lines!(Atomic::new(misaligned))),
            ("from", lines!(Atomic::from(misaligned))),
            ("default", lines!(Atomic::<OddStack>::default())),
            ("store", lines!(head.store(misaligned, Release))),
            ("swap", lines!(head.swap(misaligned, AcqRel))),
            (
                "an exchange's current",
                lines!(head.compare_exchange(misaligned, aligned, AcqRel, Acquire)),
            ),
            (
                "a weak one's new",
                lines!(head.compare_exchange_weak(aligned, misaligned, AcqRel, Acquire)),
            ),
            ("update", lines!(head.update(AcqRel, Acquire, |_| misaligned))),
            ("try_update", lines!(head.try_update(AcqRel, Acquire, |_| Some(misaligned)))),
            ("store_rmw", lines!(head.store_rmw(misaligned, Release))),
            ("set", lines!(Atomic::new(aligned).set(misaligned))),
            ("with_mut", lines!(Atomic::new(aligned).with_mut(|value| *value = misaligned))),
            ("an option", lines!(link.store(Some(Link { next: odd, deleted: true }), Release))),
            ("a newtype", lines!(stack.store(Stack(misaligned), Release))),
            ("a pointer enum", lines!(slot.store(Slot::Left(odd.cast()), Release))),
        ];
        for (operation, (line, reported)) in lines {
            assert_eq!(reported, Some(line), "{operation}: the panic's line is the caller's");
        }
    }
}
