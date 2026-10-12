//! Models of a derived packed struct's field operations under loom, through the seam: each is one
//! read-modify-write of loom's atomic, or `update`'s loop of them, so concurrent operations on
//! different fields all land, one test-and-set wins, and a bit set with Release publishes what came
//! before it. A pointer enum, one pointer word, exchanges as any atomic does, publishes the node a
//! Release store holds, and keeps a tag beside it that changes alone. A stack whose nodes each hold
//! an atomic of its head takes a push before it is closed, and its pop reads the node whole, or
//! refuses the push. A word over a reference publishes what it points to through a Release store,
//! and a tag set Relaxed beside it publishes it still, as one updated with Acquire alone does,
//! whose read sees it whole.
//!
//! The structures the cookbook and the examples build are modelled from the files their pages and
//! the examples include: a free list hands each node to one thread and takes each back, a lock word
//! keeps two holders apart, a quote is taken once and whole, a stale key finds its value or
//! nothing, a sequence lock is read whole, a bitmap hands each slot out once, and a node linked
//! after one on its way out is not lost.
//!
//! Each `should_panic` model is a bug loom must report; the rest hold under every interleaving loom
//! explores.

// Loom's types exist only under `--cfg loom`; a crate-level `cfg` would empty the crate, and with
// it the attributes `just check-rust-lints` injects.
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
mod tests {
    use core::ptr::NonNull;

    use atomix::cell::UnsafeCell;
    use atomix::model::{Arc, check, thread};
    use atomix::ordering::{AcqRel, Acquire, Relaxed, Release, RmwOrdering, StoreOrdering};
    use atomix::{Atom, AtomBitwise, Atomic, AtomicU64, RangedU32};

    /// Flags, each pattern of a byte a value.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom, AtomBitwise)]
    struct Flags(u8);

    /// Flags and a bit, then a count in the top half of the word.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Counted {
        flags: Flags,
        ready: bool,
        spare: RangedU32<0, 0x7F_FFFF>,
        count: u32,
    }

    /// The value each model starts from.
    const ZERO: Counted =
        Counted { flags: Flags(0), ready: false, spare: RangedU32::MIN, count: 0 };

    #[test]
    fn operations_on_different_fields_each_land() {
        check(|| {
            let counted = Arc::new(Atomic::new(ZERO));
            let others: Vec<_> = [Flags(0b01), Flags(0b10)]
                .into_iter()
                .map(|flag| {
                    let counted = Arc::clone(&counted);
                    thread::spawn(move || {
                        // ORDERING: Relaxed throughout: the join publishes each write.
                        counted.fields().count.fetch_add(1, Relaxed);
                        counted.fields().flags.or(flag, Relaxed);
                    })
                })
                .collect();
            counted.fields().ready.set(Relaxed);
            // The loop rebuilds the word from what it read: a stale read would drop a write.
            counted.fields().spare.update(Relaxed, Relaxed, |_| RangedU32::MAX);
            for other in others {
                other.join().expect("no thread panics");
            }
            let last = counted.load_rmw(Acquire);
            let expected =
                Counted { flags: Flags(0b11), ready: true, spare: RangedU32::MAX, count: 2 };
            assert_eq!(last, expected, "every operation landed");
        });
    }

    #[test]
    fn one_test_and_set_wins() {
        check(|| {
            let counted = Arc::new(Atomic::new(ZERO));
            let other = {
                let counted = Arc::clone(&counted);
                thread::spawn(move || counted.fields().ready.test_and_set(AcqRel))
            };
            let mine = counted.fields().ready.test_and_set(AcqRel);
            let theirs = other.join().expect("no thread panics");
            assert_ne!(mine, theirs, "exactly one finds the bit clear");
        });
    }

    // ANCHOR: publish
    /// A value published behind the bit `ready`, which the writer sets with `order`.
    fn publish<O: RmwOrdering<bool> + Send + Sync + 'static>(order: O) {
        check(move || {
            let shared = Arc::new((AtomicU64::new(0), Atomic::new(ZERO)));
            let writer = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    shared.0.store(7, Relaxed);
                    shared.1.fields().ready.set(order);
                })
            };
            if shared.1.fields().ready.test_and_clear(Acquire) {
                assert_eq!(shared.0.load(Relaxed), 7, "the bit publishes the value");
            }
            writer.join().expect("the writer does not panic");
        });
    }

    #[test]
    fn a_release_set_publishes() {
        publish(Release);
    }

    #[test]
    #[should_panic(expected = "the bit publishes the value")]
    fn a_relaxed_set_does_not() {
        publish(Relaxed);
    }
    // ANCHOR_END: publish

    /// A node, aligned to 8, its value a loom atomic so a model sees its accesses.
    #[repr(align(8))]
    struct Node {
        value: AtomicU64,
    }

    /// Empty, a value held inline, or a node: one pointer word, two tag bits.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Slot {
        Empty,
        Inline(u32),
        Node(NonNull<Node>),
    }

    /// A slot beside a lock at bit 2, above the slot's tag.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct LockedSlot {
        #[atom(pointer)]
        slot: Slot,
        locked: bool,
    }

    /// A node on the heap holding `value`, which `free` frees.
    fn node(value: u64) -> NonNull<Node> {
        NonNull::from(Box::leak(Box::new(Node { value: AtomicU64::new(value) })))
    }

    /// Frees a node `node` made.
    #[expect(unsafe_code, reason = "frees what `node` leaked")]
    fn free(node: NonNull<Node>) {
        // SAFETY: `node` leaked it, and no pointer to it is used after.
        drop(unsafe { Box::from_raw(node.as_ptr()) });
    }

    #[test]
    fn one_exchange_of_a_pointer_enum_wins_and_the_other_sees_it() {
        check(|| {
            let slot = Arc::new(Atomic::new(Slot::Empty));
            let other = {
                let slot = Arc::clone(&slot);
                thread::spawn(move || {
                    let mine = node(7);
                    let won = slot.compare_exchange(Slot::Empty, Slot::Node(mine), AcqRel, Acquire);
                    if won.is_err() {
                        free(mine);
                    }
                    won.map(|_| ()).map_err(|seen| seen == Slot::Inline(5))
                })
            };
            let mine = slot.compare_exchange(Slot::Empty, Slot::Inline(5), AcqRel, Acquire);
            let theirs = other.join().expect("no thread panics");
            assert_ne!(mine.is_ok(), theirs.is_ok(), "exactly one exchange wins");
            assert_ne!(theirs, Err(false), "the loser saw the winner's value");
            match slot.load(Acquire) {
                #[expect(unsafe_code, reason = "reads the node through the pointer decoded")]
                Slot::Node(won) => {
                    // SAFETY: the winner's node is live until freed below.
                    assert_eq!(unsafe { won.as_ref() }.value.load(Relaxed), 7, "the node whole");
                    assert_eq!(mine, Err(Slot::Node(won)), "and the value lost to it");
                    free(won);
                },
                last @ (Slot::Empty | Slot::Inline(_)) => {
                    assert_eq!(last, Slot::Inline(5), "else the value won");
                },
            }
        });
    }

    /// A node published through a pointer enum, stored with `order`.
    fn publish_a_node<O: StoreOrdering<Slot> + Send + Sync + 'static>(order: O) {
        check(move || {
            let slot = Arc::new(Atomic::new(Slot::Empty));
            let writer = {
                let slot = Arc::clone(&slot);
                thread::spawn(move || {
                    let published = node(0);
                    // SAFETY: the node is live, and no other thread holds it yet.
                    #[expect(unsafe_code, reason = "writes the node before it is published")]
                    unsafe { published.as_ref() }.value.store(7, Relaxed);
                    slot.store(Slot::Node(published), order);
                })
            };
            if let Slot::Node(seen) = slot.load(Acquire) {
                // SAFETY: the node is live until freed below.
                #[expect(unsafe_code, reason = "reads the node through the pointer decoded")]
                let value = unsafe { seen.as_ref() }.value.load(Relaxed);
                assert_eq!(value, 7, "the word publishes the node");
            }
            writer.join().expect("the writer does not panic");
            let Slot::Node(published) = slot.load(Acquire) else {
                panic!("the node is published once the writer joins");
            };
            free(published);
        });
    }

    #[test]
    fn a_release_store_of_a_pointer_enum_publishes_its_node() {
        publish_a_node(Release);
    }

    // Loom reports the node's construction racing the read through the pointer.
    #[test]
    #[should_panic(expected = "Causality violation")]
    fn a_relaxed_store_of_a_pointer_enum_does_not() {
        publish_a_node(Relaxed);
    }

    #[test]
    fn a_tag_beside_a_pointer_enum_and_an_exchange_of_the_enum_both_land() {
        check(|| {
            let locked = Arc::new(Atomic::new(LockedSlot { slot: Slot::Empty, locked: false }));
            let other = {
                let locked = Arc::clone(&locked);
                thread::spawn(move || locked.fields().locked.test_and_set(AcqRel))
            };
            // The loop rebuilds the word from what it read: a stale read would drop the lock.
            let before = locked.fields().slot.load(Relaxed);
            locked.update(AcqRel, Acquire, |seen| LockedSlot { slot: Slot::Inline(9), ..seen });
            let was_locked = other.join().expect("no thread panics");
            assert!(!was_locked, "the lock was clear before");
            assert_eq!(before, Slot::Empty, "the slot read through the word");
            let last = locked.load(Acquire);
            assert_eq!(last, LockedSlot { slot: Slot::Inline(9), locked: true }, "both landed");
        });
    }

    /// A node of a stack that closes, which holds the head below it: a word held in its own
    /// pointee. Its value is a loom atomic, so a model sees its accesses.
    struct StackNode {
        value: AtomicU64,
        next: Atomic<StackHead>,
    }

    /// A Treiber stack's head: the top node or none, and whether the stack is closed.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct StackHead {
        top: Option<NonNull<StackNode>>,
        closed: bool,
    }

    /// An open stack's head, and the head a node holds before it is pushed.
    const OPEN: StackHead = StackHead { top: None, closed: false };

    /// Frees a node `push` made.
    #[expect(unsafe_code, reason = "frees what `push` leaked")]
    fn free_stack_node(node: NonNull<StackNode>) {
        // SAFETY: `push` leaked it, and no pointer to it is used after.
        drop(unsafe { Box::from_raw(node.as_ptr()) });
    }

    /// The node `node` points to, read through it.
    #[expect(unsafe_code, reason = "reads a node through the pointer to it")]
    fn stack_node<'a>(node: NonNull<StackNode>) -> &'a StackNode {
        // SAFETY: no node is freed while another thread may read it: `push` frees one the closed
        // stack refused before any thread could see it, and the model frees one it popped once
        // the pusher has joined.
        unsafe { node.as_ref() }
    }

    /// Pushes a node of `value` with `order`, so a pop with Acquire reads it whole where that is
    /// Release; or, where the stack is closed, frees it. Whether it pushed.
    ///
    /// Its first exchange guesses the stack open and empty, and one that fails reads the head: loom
    /// backtracks to an atomic's last access alone, so a load before the exchange would hide the
    /// exchange from the pop's load, and no model would pop the node beside the push.
    fn push<O: RmwOrdering<StackHead>>(stack: &Atomic<StackHead>, value: u64, order: O) -> bool {
        let node = StackNode { value: AtomicU64::new(0), next: Atomic::new(OPEN) };
        let pushed = NonNull::from(Box::leak(Box::new(node)));
        // ORDERING: Relaxed but for the exchange that lands, whose `order` publishes the node,
        // and its value, to `pop`'s Acquire where it is Release.
        stack_node(pushed).value.store(value, Relaxed);
        let mut seen = OPEN;
        loop {
            if seen.closed {
                free_stack_node(pushed);
                return false;
            }
            stack_node(pushed).next.store(seen, Relaxed);
            let next = StackHead { top: Some(pushed), ..seen };
            match stack.compare_exchange(seen, next, order, Relaxed) {
                Ok(_) => return true,
                Err(found) => seen = found,
            }
        }
    }

    /// Pops the top node, where there is one, which the caller frees.
    fn pop(stack: &Atomic<StackHead>) -> Option<NonNull<StackNode>> {
        // ORDERING: Acquire on every read of the head, pairing with a Release push, so the top
        // node it names reads whole, its own head below Relaxed.
        let mut seen = stack.load(Acquire);
        loop {
            let top = seen.top?;
            let below = stack_node(top).next.load(Relaxed).top;
            match stack.compare_exchange(seen, StackHead { top: below, ..seen }, Acquire, Acquire) {
                Ok(_) => return Some(top),
                Err(found) => seen = found,
            }
        }
    }

    /// A push with `order` beside a pop and a close of a stack held in its own nodes.
    fn push_beside_a_close<O: RmwOrdering<StackHead> + Send + Sync + 'static>(order: O) {
        check(move || {
            let stack = Arc::new(Atomic::new(OPEN));
            let pusher = {
                let stack = Arc::clone(&stack);
                thread::spawn(move || push(&stack, 7, order))
            };
            // A pop beside the push reads its node through the Release the push stores with.
            let early = pop(&stack);
            let was_closed = stack.fields().closed.test_and_set(AcqRel);
            let landed = pusher.join().expect("the pusher does not panic");
            assert!(!was_closed, "open until closed");
            match early.or_else(|| pop(&stack)) {
                Some(popped) => {
                    assert!(landed, "a node popped was pushed");
                    assert_eq!(stack_node(popped).value.load(Relaxed), 7, "and is read whole");
                    free_stack_node(popped);
                },
                None => assert!(!landed, "else the push was refused"),
            }
            assert_eq!(stack.load(Acquire), StackHead { closed: true, ..OPEN }, "and it is closed");
        });
    }

    #[test]
    fn a_stack_held_in_its_own_nodes_takes_a_push_before_it_closes_or_refuses_it() {
        push_beside_a_close(Release);
    }

    // Loom reports the node's construction racing the pop that reads through the pointer.
    #[test]
    #[should_panic(expected = "Causality violation")]
    fn a_relaxed_push_does_not_publish_its_node() {
        push_beside_a_close(Relaxed);
    }

    /// A configuration written in place before a reference to it is published, and leaked for
    /// the rest of the model.
    struct Config(UnsafeCell<u64>);

    // SAFETY: each model writes a configuration only before publishing a reference to it, and
    // reads it only through a reference it loaded.
    #[expect(unsafe_code, reason = "a configuration read only through its publication")]
    unsafe impl Sync for Config {}

    /// A configuration holding `value`, written in place.
    fn config(value: u64) -> &'static Config {
        let config: &'static Config = Box::leak(Box::new(Config(UnsafeCell::new(0))));
        // SAFETY: no other thread holds a reference to the configuration yet.
        #[expect(unsafe_code, reason = "writes a configuration before it is published")]
        config.0.with_mut(|place| unsafe { *place = value });
        config
    }

    /// What `config` holds, read in place.
    fn read(config: &Config) -> u64 {
        // SAFETY: the configuration was written before its reference was published, and is not
        // written again; loom reports a read the publication does not order after that write.
        #[expect(unsafe_code, reason = "reads a published configuration")]
        config.0.with(|place| unsafe { *place })
    }

    /// The configuration in force, or none, and whether it is stale: a pointer word over a
    /// reference.
    #[derive(Clone, Copy, Atom)]
    struct Published {
        config: Option<&'static Config>,
        stale: bool,
    }

    // A tag's `Relaxed` read-modify-write continues the release sequence the reference's store
    // heads, so a load that reads the tag's write still sees the configuration whole.
    #[test]
    fn a_tag_set_relaxed_beside_a_published_reference_keeps_it_published() {
        check(|| {
            let published = Arc::new(Atomic::new(Published { config: None, stale: false }));
            let writer = {
                let published = Arc::clone(&published);
                thread::spawn(move || {
                    published.store(Published { config: Some(config(7)), stale: false }, Release);
                })
            };
            let marker = {
                let published = Arc::clone(&published);
                thread::spawn(move || published.fields().stale.set(Relaxed))
            };
            if let Some(config) = published.load(Acquire).config {
                assert_eq!(read(config), 7, "the configuration, whole");
            }
            writer.join().expect("the writer does not panic");
            marker.join().expect("the marker does not panic");
        });
    }

    // A tag's `update` hands out the word, so its read acquires; its write puts the reference back
    // as it read it, with no `Release`, and continues the release sequence the reference's store
    // heads, so a load that reads that write still sees the configuration whole.
    #[test]
    fn a_tag_updated_with_acquire_beside_a_published_reference_reads_it_and_keeps_it_published() {
        check(|| {
            let published = Arc::new(Atomic::new(Published { config: None, stale: false }));
            let writer = {
                let published = Arc::clone(&published);
                thread::spawn(move || {
                    published.store(Published { config: Some(config(7)), stale: false }, Release);
                })
            };
            let marker = {
                let published = Arc::clone(&published);
                thread::spawn(move || {
                    let before = published.fields().stale.update(Acquire, Acquire, |_| true);
                    if let Some(config) = before.config {
                        assert_eq!(read(config), 7, "the configuration the update read, whole");
                    }
                })
            };
            if let Some(config) = published.load(Acquire).config {
                assert_eq!(read(config), 7, "the configuration, whole");
            }
            writer.join().expect("the writer does not panic");
            marker.join().expect("the marker does not panic");
        });
    }
}

// The structures the cookbook and the examples build, from the files their pages and the examples
// include, so the code the book shows is the code loom checks.
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
#[path = "cookbook/bitmap_allocator.rs"]
mod bitmap_allocator;
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
#[path = "../examples/free-list/free_list.rs"]
mod free_list;
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
#[path = "../examples/lock-word/lock_word.rs"]
mod lock_word;
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
#[path = "../examples/quote-book/quote_book.rs"]
mod quote_book;
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
#[path = "cookbook/sequence_lock.rs"]
mod sequence_lock;
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
#[path = "cookbook/slot_map.rs"]
mod slot_map;
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
#[path = "cookbook/sorted_list.rs"]
mod sorted_list;

/// Models of the structures the cookbook and the examples build, two or three threads at a time,
/// under every interleaving loom explores.
#[cfg(all(loom, feature = "derive"))]
#[cfg(test)]
mod structures {
    use core::num::NonZero;

    use atomix::model::{Arc, check, thread};
    use atomix::ordering::Relaxed;
    use atomix::{AtomicU64, RangedU64};

    use crate::bitmap_allocator::Bitmap;
    use crate::free_list::{FreeList, Index};
    use crate::lock_word::{LockWord, OwnerId};
    use crate::quote_book::{Book, Side};
    use crate::sequence_lock::SequenceLock;
    use crate::slot_map::SlotMap;
    use crate::sorted_list::{List, Node};

    #[test]
    fn a_free_list_hands_its_node_from_holder_to_holder() {
        check(|| {
            let shared = Arc::new((FreeList::new(1), AtomicU64::new(0)));
            // Bumps the node's count, by a load and a store that only the list's pop and push keep
            // in order: a holder that missed the last one's bump would lose it.
            let hold_once = |shared: &(FreeList, AtomicU64)| {
                let (list, bumps) = shared;
                let node = list.pop()?;
                // ORDERING: Relaxed: the pop's Acquire and the push's Release order the two.
                bumps.store(bumps.load(Relaxed).wrapping_add(1), Relaxed);
                list.push(node);
                Some(node)
            };
            let other = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || hold_once(&shared))
            };
            let held = [hold_once(&shared), other.join().expect("no thread panics")];
            let holders: u64 = held.iter().map(|node| u64::from(node.is_some())).sum();
            assert_eq!(shared.1.load(Relaxed), holders, "each holder saw the last one's bump");
            assert_eq!(shared.0.pop().map(Index::get), Some(0), "and the node came back");
        });
    }

    #[test]
    fn a_lock_word_keeps_two_holders_apart() {
        check(|| {
            let shared = Arc::new((LockWord::new(), AtomicU64::new(0)));
            let count_once = |shared: &(LockWord, AtomicU64), owner: u64| {
                let (lock, count) = shared;
                let owner = OwnerId(RangedU64::MIN.saturating_add(owner));
                let _guard = lock.lock(owner).expect("no holder panics");
                // ORDERING: Relaxed: the lock orders the two accesses.
                count.store(count.load(Relaxed).wrapping_add(1), Relaxed);
            };
            let other = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || count_once(&shared, 1))
            };
            count_once(&shared, 0);
            other.join().expect("no thread panics");
            assert_eq!(shared.1.load(Relaxed), 2, "neither count lost");
        });
    }

    #[test]
    fn a_quote_is_taken_once_whole() {
        check(|| {
            let book = Arc::new(Book::new(1));
            let quantity = NonZero::new(5).expect("5 is not zero");
            let feed = {
                let book = Arc::clone(&book);
                thread::spawn(move || {
                    assert!(book.post(0, Side::Ask, quantity), "the level is free");
                })
            };
            let other = {
                let book = Arc::clone(&book);
                thread::spawn(move || book.take(0))
            };
            let mine = book.take(0);
            feed.join().expect("the feed does not panic");
            let theirs = other.join().expect("no taker panics");
            let taken: Vec<_> = [mine, theirs, book.take(0)].into_iter().flatten().collect();
            assert_eq!(taken.len(), 1, "one taker took the quote");
            assert_eq!(taken[0].quantity, 5, "whole");
            assert_eq!(taken[0].side, Side::Ask, "on its side");
        });
    }

    #[test]
    fn a_stale_key_finds_its_value_or_nothing() {
        check(|| {
            let map = Arc::new(SlotMap::new(1));
            let key = map.insert(7).expect("the slot is free");
            let reader = {
                let map = Arc::clone(&map);
                thread::spawn(move || map.get(key))
            };
            assert_eq!(map.remove(key), Some(7), "the value removed");
            let other = map.insert(9).expect("the slot is free again");
            let seen = reader.join().expect("the reader does not panic");
            assert!(matches!(seen, Some(7) | None), "the stale key never finds 9: {seen:?}");
            assert_eq!(map.get(other), Some(9), "which its own key finds");
        });
    }

    /// Reads the lock with `read` while a writer writes it twice: two writes, so a read whose
    /// words are older than its first load of the sequence can read one word of each.
    fn read_beside_two_writes(read: fn(&SequenceLock) -> [u64; 2]) {
        check(move || {
            let lock = Arc::new(SequenceLock::new([0, 0]));
            let writer = {
                let lock = Arc::clone(&lock);
                thread::spawn(move || {
                    lock.write([1, 2]);
                    lock.write([2, 4]);
                })
            };
            let [first, second] = read(&lock);
            writer.join().expect("the writer does not panic");
            assert_eq!(second, first.wrapping_mul(2), "both words from one write");
            assert_eq!(read(&lock), [2, 4], "and the last write read once it joined");
        });
    }

    #[test]
    fn a_sequence_lock_is_read_whole() {
        read_beside_two_writes(SequenceLock::read);
    }

    #[test]
    #[should_panic(expected = "both words from one write")]
    fn a_sequence_lock_read_from_a_relaxed_first_load_is_not() {
        read_beside_two_writes(SequenceLock::read_with_a_relaxed_first_load);
    }

    #[test]
    #[should_panic(expected = "both words from one write")]
    fn a_sequence_lock_read_without_its_fence_is_not() {
        read_beside_two_writes(SequenceLock::read_without_its_fence);
    }

    #[test]
    fn a_bitmap_hands_out_each_slot_once() {
        check(|| {
            let bitmap = Arc::new(Bitmap::<1>::new());
            let other = {
                let bitmap = Arc::clone(&bitmap);
                thread::spawn(move || bitmap.allocate())
            };
            let mine = bitmap.allocate().expect("a slot is free");
            let theirs = other.join().expect("no thread panics").expect("a slot is free");
            assert_ne!(mine, theirs, "each slot handed out once");
            bitmap.free(mine);
            assert_eq!(bitmap.allocate(), Some(mine), "and the first free one again");
        });
    }

    /// Links 25 with `insert` while 20, the node it goes after, is taken out.
    fn insert_beside_a_removal(insert: fn(&List<'static>, &'static Node) -> bool) {
        check(move || {
            let arena: &'static [Node; 4] = Box::leak(Box::new([10, 20, 30, 25].map(Node::new)));
            let list = Arc::new(List::new());
            for node in &arena[..3] {
                assert!(list.insert(node), "each key goes in once");
            }
            let inserter = {
                let list = Arc::clone(&list);
                thread::spawn(move || insert(&list, &arena[3]))
            };
            assert!(list.remove(20), "20 was in");
            assert!(inserter.join().expect("the inserter does not panic"), "25 was not in");
            let keys: Vec<u64> =
                [10, 20, 25, 30].into_iter().filter(|key| list.contains(*key)).collect();
            assert_eq!(keys, [10, 25, 30], "20 out, 25 in");
        });
    }

    #[test]
    fn a_node_linked_after_one_on_its_way_out_is_not_lost() {
        insert_beside_a_removal(List::insert);
    }

    #[test]
    #[should_panic(expected = "20 out, 25 in")]
    fn an_insert_that_ignores_marks_loses_its_node() {
        insert_beside_a_removal(List::insert_ignoring_marks);
    }
}
