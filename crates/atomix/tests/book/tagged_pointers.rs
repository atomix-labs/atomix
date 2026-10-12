//! The listings of the chapter Tagged Pointers.

#[test]
// ANCHOR: head
fn a_stack_keeps_a_version_and_a_closed_bit_in_its_top_pointer() {
    use core::ptr::NonNull;

    use atomix::ordering::{AcqRel, Acquire, Relaxed, Release};
    use atomix::{Atom, Atomic, RangedU8};

    /// A node of a stack: aligned to 8, so a pointer to one leaves three low bits clear.
    #[repr(align(8))]
    struct Node {
        value: u64,
        next: Atomic<Option<NonNull<Self>>>,
    }

    /// A Treiber stack's head: the top node, then in its pointer's three clear bits a version,
    /// which counts the head's changes, and whether the stack is closed.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Head {
        top: Option<NonNull<Node>>,
        version: RangedU8<0, 3>,
        closed: bool,
    }

    /// The version after `version`, wrapping at 3.
    fn next(version: RangedU8<0, 3>) -> RangedU8<0, 3> {
        version.checked_add(1).unwrap_or(RangedU8::MIN)
    }

    /// Pushes `node` where the stack is open, and returns whether it did.
    fn push(head: &Atomic<Head>, node: &Node) -> bool {
        // ORDERING: Release on the exchange that lands, publishing the node to the pop that takes
        // it; Relaxed on each read, which only feeds the exchange.
        let pushed = head.try_update(Release, Relaxed, |seen| {
            if seen.closed {
                return None;
            }
            node.next.store(seen.top, Relaxed);
            Some(Head { top: Some(NonNull::from_ref(node)), version: next(seen.version), ..seen })
        });
        pushed.is_ok()
    }

    /// Pops the top node, where there is one.
    ///
    /// # Safety
    /// Every node pushed onto `head` outlives it.
    #[expect(unsafe_code, reason = "reads each top node through its pointer")]
    unsafe fn pop(head: &Atomic<Head>) -> Option<NonNull<Node>> {
        // ORDERING: Acquire on each read of the head, pairing with the push's Release, so the
        // top's `next` is read as the push wrote it.
        let popped = head.try_update(Acquire, Acquire, |seen| {
            let top = seen.top?;
            // SAFETY: the caller's nodes outlive the stack, so `top` points to a whole node, even
            // where another thread has popped it since.
            let below = unsafe { top.as_ref() }.next.load(Relaxed);
            Some(Head { top: below, version: next(seen.version), ..seen })
        });
        popped.ok()?.top
    }

    let nodes = [7, 8].map(|value| Node { value, next: Atomic::new(None) });
    let head = Atomic::new(Head { top: None, version: RangedU8::MIN, closed: false });
    assert!(nodes.iter().all(|node| push(&head, node)), "both pushed onto the open stack");
    // SAFETY: `nodes`, declared before `head`, outlives it, and no other node is pushed.
    #[expect(unsafe_code, reason = "pops nodes that outlive the stack")]
    let popped = unsafe { pop(&head) };
    assert_eq!(popped, Some(NonNull::from_ref(&nodes[1])), "the last pushed, popped first");

    // `lock bts`, or `ldsetal`, on the top pointer: the one thread that finds it open closes it.
    assert!(!head.fields().closed.test_and_set(AcqRel), "this thread closed the stack");
    assert!(!push(&head, &nodes[1]), "and a closed stack refuses a push");
    assert_eq!(head.fields().top.load(Acquire), Some(NonNull::from_ref(&nodes[0])), "its top");
    assert_eq!(head.load(Acquire).version.get(), 3, "three changes, counted in the tags");
    assert_eq!(nodes.map(|node| node.value), [7, 8], "the nodes' values, untouched");
    assert_eq!(size_of::<Atomic<Head>>(), 8, "and the head in one word");
}
// ANCHOR_END: head

#[test]
// ANCHOR: pointer_enum
fn a_pointer_enum_keeps_its_variant_in_the_tags_and_a_word_holds_it() {
    use core::ptr::NonNull;

    use atomix::ordering::{AcqRel, Acquire};
    use atomix::{Atom, Atomic};

    /// A quote too wide for a word, which a cache points to: aligned to 8, for its `u64`s.
    struct Quote {
        price: u64,
        quantity: u64,
    }

    /// A cache's entry: missing, a price that fits 32 bits, held inline, or a pointer to a whole
    /// quote. Each variant's tag is its discriminant, in two of the bits a quote's alignment
    /// leaves clear, and an inline price lies above all three.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Cached {
        Missing,
        Price(u32),
        Quote(NonNull<Quote>),
    }

    /// An entry, and whether a reader has pinned it, in bit 2, the last one clear. The derive
    /// sees a pointer where its type is written as one, so a word inside a word is marked.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Pinned {
        #[atom(pointer)]
        entry: Cached,
        pinned: bool,
    }

    let quote = Quote { price: 10_050, quantity: 300 };
    let pointer = NonNull::from_ref(&quote);
    assert_eq!(Cached::Price(5).to_repr().addr(), 5 << 3 | 1, "a price above its tag, 1");
    assert_eq!(Cached::Quote(pointer).to_repr().addr(), pointer.addr().get() | 2, "a pointer's, 2");

    let cache = Atomic::new(Pinned { entry: Cached::Price(10_000), pinned: false });
    // The tag beside the entry is a place of its own: `lock bts`, or `ldsetal`, on the pointer.
    assert!(!cache.fields().pinned.test_and_set(AcqRel), "this reader pinned it");
    let before =
        cache.update(AcqRel, Acquire, |seen| Pinned { entry: Cached::Quote(pointer), ..seen });
    assert_eq!(before.entry, Cached::Price(10_000), "the price it held");
    assert_eq!(cache.fields().entry.load(Acquire), Cached::Quote(pointer), "the quote's pointer");
    assert!(cache.load(Acquire).pinned, "beside the pin, which the update kept");
    assert_eq!((quote.price, quote.quantity), (10_050, 300), "and the quote's fields, untouched");
}
// ANCHOR_END: pointer_enum
