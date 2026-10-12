//! A free list: a Treiber stack of the free nodes of an arena, by index, whose head packs its top
//! index and a version into one `u64`, so it keeps strict provenance on every target.
//!
//! Every node lives as long as the list, so a thread that read a node another has since taken
//! reads a stale successor, never freed memory, and the version, which counts each change of the
//! head, fails the exchange built on that read.

use atomix::ordering::{Acquire, Relaxed, Release};
use atomix::{Atom, Atomic, RangedU16};

// ANCHOR: head
/// A node's index in the arena: any `u16` but the last, which `Option`'s `None` takes.
pub(crate) type Index = RangedU16<0, 0xFFFE>;

/// The top free node, and a count of the head's changes, which tells a top taken and given back
/// from the one read: 16 bits and 32, in a `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
struct Head {
    /// The top free node, or none where every node is taken.
    top: Option<Index>,
    /// How many times the head has changed, wrapping.
    version: u32,
}
// ANCHOR_END: head

/// The free nodes of an arena of up to 65,535, as a stack.
#[derive(Debug)]
pub(crate) struct FreeList {
    /// The top free node.
    head: Atomic<Head>,
    /// Each node's successor on the stack, written by whoever holds the node.
    next: Box<[Atomic<Option<Index>>]>,
}

impl FreeList {
    /// A list of every node of an arena of `len`, the first on top.
    pub(crate) fn new(len: u16) -> Self {
        // Node `i`'s successor is `i + 1`, and the last's is none.
        let next = (1..len).map(Index::new).chain([None]).take(usize::from(len));
        let top = (len > 0).then_some(Index::MIN);
        Self { head: Atomic::new(Head { top, version: 0 }), next: next.map(Atomic::new).collect() }
    }

    // ANCHOR: pop
    /// Takes a free node, or `None` where every node is taken.
    pub(crate) fn pop(&self) -> Option<Index> {
        // ORDERING: Acquire on every read of the head, so the successor the top's push wrote, and
        // whatever its last holder wrote to the node, is seen.
        let taken = self.head.try_update(Acquire, Acquire, |head| {
            let top = head.top?;
            Some(Head { top: self.next(top).load(Relaxed), version: head.version.wrapping_add(1) })
        });
        taken.ok()?.top
    }
    // ANCHOR_END: pop

    // ANCHOR: push
    /// Gives back `node`, which `pop` took.
    ///
    /// # Panics
    /// Where `node` lies past the arena.
    #[track_caller]
    pub(crate) fn push(&self, node: Index) {
        // ORDERING: Release on the exchange that lands, publishing the successor, and whatever this
        // thread wrote to the node, to the pop that takes it; Relaxed on each read, which feeds the
        // exchange alone.
        self.head.update(Release, Relaxed, |head| {
            self.next(node).store(head.top, Relaxed);
            Head { top: Some(node), version: head.version.wrapping_add(1) }
        });
    }
    // ANCHOR_END: push

    /// The slot of `node`'s successor.
    ///
    /// # Panics
    /// Where `node` lies past the arena, which no node `pop` takes does.
    #[track_caller]
    #[cfg_attr(
        not(test),
        expect(clippy::indexing_slicing, reason = "a node past the arena is the caller's bug")
    )]
    fn next(&self, node: Index) -> &Atomic<Option<Index>> {
        &self.next[usize::from(node.get())]
    }
}
