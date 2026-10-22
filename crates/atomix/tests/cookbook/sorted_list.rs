//! A sorted list whose nodes are taken out by marking their links first, as Harris's list takes
//! them: a node's link to the next keeps a mark in a bit the node's alignment leaves clear, and a
//! node whose link is marked is on its way out, so no thread links a node after it.
//!
//! Nodes come from an arena that outlives the list, so a thread that reads a node another has
//! unlinked reads a whole node, never freed memory: the list frees nothing. A list that frees its
//! nodes takes crossbeam-epoch, or another reclaimer, beside atomix.

use core::marker::PhantomData;
use core::ptr::NonNull;

use atomix::ordering::{Acquire, Relaxed, Release};
use atomix::{Atom, Atomic, AtomicBool};

// ANCHOR: link
/// A link to the next node, and whether the node that holds it is marked, on its way out though
/// maybe still linked: one pointer word, the mark in bit 0, which a node's alignment of 8 leaves
/// clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
struct Link {
    /// The next node, or none at the end.
    next: Option<NonNull<Node>>,
    /// Whether the node that holds the link is on its way out.
    marked: bool,
}

/// A node of a list: its key, its link to the next, and whether a list has taken it.
#[derive(Debug)]
pub(crate) struct Node {
    /// The key it holds.
    key: u64,
    // INVARIANT: null, or a node the list that took this one took too; that list's `insert` and
    // `sweep_to` write it, and `remove` marks it.
    /// The next node, and whether this one is on its way out.
    link: Atomic<Link>,
    /// Whether a list has taken it, so it goes in once, into one list.
    taken: AtomicBool,
}
// ANCHOR_END: link

impl Node {
    /// A node of `key`, in no list yet.
    pub(crate) fn new(key: u64) -> Self {
        let link = Atomic::new(Link { next: None, marked: false });
        Self { key, link, taken: AtomicBool::new(false) }
    }
}

/// A sorted list of nodes from an arena that lives for `'arena`, each key in it once.
#[derive(Debug)]
pub(crate) struct List<'arena> {
    // INVARIANT: null, or a node this list took as `&'arena Node`; `insert` and `sweep_to` write
    // it.
    /// The first node.
    head: Atomic<Link>,
    /// The arena the nodes come from, invariant in `'arena`, so a list of a longer arena takes no
    /// node of a shorter one.
    arena: PhantomData<fn(&'arena Node) -> &'arena Node>,
}

impl<'arena> List<'arena> {
    /// An empty list.
    pub(crate) fn new() -> Self {
        Self { head: Atomic::new(Link { next: None, marked: false }), arena: PhantomData }
    }

    // ANCHOR: insert
    /// Links `node` where its key goes, and returns whether it did: not where its key is in the
    /// list already, nor where a list took the node before, since a node goes in once.
    pub(crate) fn insert(&self, node: &'arena Node) -> bool {
        // ORDERING: Relaxed: the claim publishes nothing; the exchange below publishes the node.
        if node.taken.swap(true, Relaxed) {
            return false;
        }
        loop {
            let (before, next) = self.sweep_to(node.key);
            if next.is_some_and(|next| next.key == node.key) {
                return false;
            }
            let past = Link { next: next.map(NonNull::from_ref), marked: false };
            // ORDERING: Relaxed: the exchange below publishes it.
            node.link.store(past, Relaxed);
            let to_node = Link { next: Some(NonNull::from_ref(node)), marked: false };
            // ORDERING: Release, publishing the node's key and link to whoever reads through this
            // link with Acquire; Relaxed where it fails, since the search starts again.
            if before.compare_exchange(past, to_node, Release, Relaxed).is_ok() {
                return true;
            }
        }
    }
    // ANCHOR_END: insert

    // ANCHOR: remove
    /// Takes the node of `key` out, and returns whether it was in the list.
    pub(crate) fn remove(&self, key: u64) -> bool {
        let (_, Some(node)) = self.sweep_to(key) else { return false };
        // ORDERING: Relaxed: the mark publishes only itself; whoever unlinks the node reached it,
        // and its successor, through Acquire loads of links an insert's Release published.
        if node.key != key || node.link.fields().marked.test_and_set(Relaxed) {
            // Not in the list, or another thread is taking it out.
            return false;
        }
        // Unlinks it, or leaves it to the next search that passes it.
        let _unlinked = self.sweep_to(key);
        true
    }

    /// Whether `key` is in the list: a walk that writes nothing, passing each marked node.
    pub(crate) fn contains(&self, key: u64) -> bool {
        // ORDERING: Acquire on every read of a link, as in `sweep_to`.
        let mut next = self.head.load(Acquire).next;
        while let Some(node) = next.map(Self::node) {
            let link = node.link.load(Acquire);
            if node.key >= key {
                return node.key == key && !link.marked;
            }
            next = link.next;
        }
        false
    }

    /// Unlinks each marked node it passes on the way to `key`, and returns the link that leads
    /// past every key below `key` and the node it leads to: the first whose key is `key` or above,
    /// if any.
    fn sweep_to(&self, key: u64) -> (&Atomic<Link>, Option<&'arena Node>) {
        'search: loop {
            let mut before = &self.head;
            // ORDERING: Acquire on every read of a link, pairing with the Release of the exchange
            // that wrote it, so the node it leads to reads as its insert published it.
            let mut next = before.load(Acquire).next;
            while let Some(node) = next.map(Self::node) {
                let link = node.link.load(Acquire);
                if link.marked {
                    // Leads `before` past the node, where it still leads to it unmarked.
                    let to_node = Link { next, marked: false };
                    let past_node = Link { next: link.next, marked: false };
                    // ORDERING: Release, publishing the successor this thread read with Acquire
                    // to whoever reads through `before`; Relaxed where it fails, since the search
                    // starts again from the head.
                    if before.compare_exchange(to_node, past_node, Release, Relaxed).is_err() {
                        continue 'search;
                    }
                    next = link.next;
                } else if node.key >= key {
                    return (before, Some(node));
                } else {
                    before = &node.link;
                    next = link.next;
                }
            }
            return (before, None);
        }
    }
    // ANCHOR_END: remove

    /// `insert`, but linking `node` after a node whatever its link's mark, the race the marks
    /// close: the list's model must report the node lost.
    #[cfg(loom)]
    pub(crate) fn insert_ignoring_marks(&self, node: &'arena Node) -> bool {
        // ORDERING: as in `insert`.
        if node.taken.swap(true, Relaxed) {
            return false;
        }
        loop {
            let (before, next) = self.sweep_to(node.key);
            if next.is_some_and(|next| next.key == node.key) {
                return false;
            }
            let next = next.map(NonNull::from_ref);
            // ORDERING: Relaxed, as in `insert`: the exchange below publishes it.
            node.link.store(Link { next, marked: false }, Relaxed);
            let to_node = |link: Link| {
                (link.next == next).then_some(Link { next: Some(NonNull::from_ref(node)), ..link })
            };
            // ORDERING: Release and Relaxed, as `insert`'s exchange.
            if before.try_update(Release, Relaxed, to_node).is_ok() {
                return true;
            }
        }
    }

    /// The node `pointer` points to.
    #[expect(
        unsafe_code,
        reason = "reads a node through a link's pointer: a word holds no reference"
    )]
    fn node(pointer: NonNull<Node>) -> &'arena Node {
        // SAFETY: by the INVARIANTs of `head` and of each node's `link`, `pointer` is to a node
        // this list took as `&'arena Node`, once, and frees none, so it is a whole node for
        // `'arena`, shared, written only through its atomics.
        unsafe { pointer.as_ref() }
    }
}
