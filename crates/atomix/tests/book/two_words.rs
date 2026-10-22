//! The listings of the chapter Two Words.

#[test]
// ANCHOR: pop
fn a_pop_changes_the_top_and_its_version_in_one_exchange() {
    use core::ptr::NonNull;

    use atomix::ordering::{Acquire, Relaxed, Release};
    use atomix::{Atom, Atomic};

    /// A node of a stack.
    struct Node {
        value: u64,
        next: Atomic<Option<NonNull<Self>>>,
    }

    /// A Treiber stack's head: the top node, and a version that counts the head's changes. No
    /// alignment leaves 64 bits clear, so the version takes a word of its own.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Head {
        top: Option<NonNull<Node>>,
        version: u64,
    }

    let node = Node { value: 7, next: Atomic::new(None) };
    let head = Atomic::new(Head { top: None, version: 0 });
    let pushed = Head { top: Some(NonNull::from_ref(&node)), version: 1 };
    // ORDERING: Release, publishing the node to the pop that reads it.
    head.store_rmw(pushed, Release);

    // `load_rmw` reads with a compare-exchange, on every target; `load`, where LSE2 or AVX gives
    // a 16-byte load.
    // ORDERING: Acquire, pairing with the push's Release, so the node's `next` is read as the
    // push wrote it.
    let seen = head.load_rmw(Acquire);
    // ORDERING: Relaxed: the Acquire read of the head orders it after the push.
    let below = node.next.load(Relaxed);
    let popped = Head { top: below, version: seen.version.wrapping_add(1) };
    // One `caspa`, or `lock cmpxchg16b`: a top popped and pushed again since `seen` has another
    // version, so the exchange fails.
    // ORDERING: Acquire, as the read above; a pop publishes nothing.
    assert_eq!(head.compare_exchange(seen, popped, Acquire, Acquire), Ok(pushed), "popped");
    assert_eq!(node.value, 7, "the node, which the head pointed to");
    assert_eq!(size_of_val(&head), 16, "the top and its version in two words");
}
// ANCHOR_END: pop

#[test]
// ANCHOR: slice
fn a_slice_is_held_by_its_pointer_and_its_length() {
    use core::ptr::NonNull;

    use atomix::Atomic;
    use atomix::ordering::{Acquire, Release};

    // The bytes of the last message a feed received: their pointer and their length.
    let latest: Atomic<Option<NonNull<[u8]>>> = Atomic::new(None);

    let message = *b"fill 300";
    latest.store_rmw(Some(NonNull::from_ref(&message[..])), Release);
    let read = latest.load_rmw(Acquire).expect("a message was stored");
    assert_eq!(read.len(), 8, "its length, beside its pointer");
    assert_eq!(read.cast::<u8>(), NonNull::from_ref(&message[0]), "and the bytes it points to");
}
// ANCHOR_END: slice
