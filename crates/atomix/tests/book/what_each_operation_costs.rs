//! The listings of the chapter What Each Operation Costs.

#[test]
// ANCHOR: by_name
fn each_operation_is_one_instruction_or_a_loop_by_name() {
    use atomix::AtomicU64;
    use atomix::ordering::{AcqRel, Acquire, Relaxed, Release};

    let (flags, next, highest) = (AtomicU64::new(0), AtomicU64::new(1), AtomicU64::new(0));

    // `or` returns nothing: `lock or` on `x86_64`, `ldsetl` on `aarch64`.
    flags.or(0b10, Release);
    // A `fetch_add` whose value goes unused is `lock add` already, so there is no `add`.
    next.fetch_add(1, Relaxed);
    // A maximum is `ldumaxal` on `aarch64`, and a loop on `x86_64`: written once, as a loop.
    assert_eq!(highest.update(AcqRel, Acquire, |seen| seen.max(42)), 0, "the highest before");
    #[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
    assert_eq!(highest.fetch_max(7, AcqRel), 42, "and `aarch64`'s one instruction");

    assert_eq!((flags.load(Acquire), next.load(Acquire)), (0b10, 2), "the flag and the count");
}
// ANCHOR_END: by_name

#[test]
// ANCHOR: capability
fn a_newtype_adds_as_its_field_does() {
    use atomix::ordering::{Acquire, Relaxed};
    use atomix::{Atom, AtomAdd, Atomic};

    /// A count of shares, which adds as its field does.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom, AtomAdd)]
    struct Shares(u64);

    let filled = Atomic::new(Shares(0));
    // The delta is the repr's, a `u64`: an add on the repr is an add on the value.
    assert_eq!(filled.fetch_add(300, Relaxed), Shares(0), "the count before");
    assert_eq!(filled.fetch_sub(100, Relaxed), Shares(300), "and after the first fill");
    assert_eq!(filled.load(Acquire), Shares(200), "one fill of 300, one of 100 taken back");
}
// ANCHOR_END: capability

#[test]
// ANCHOR: bit
fn a_bit_chosen_at_run_time_is_one_instruction() {
    use atomix::AtomicU64;
    use atomix::ordering::{Acquire, Release};

    /// Which of 64 clients are subscribed, a bit each.
    static SUBSCRIBED: AtomicU64 = AtomicU64::new(0);

    /// Subscribes `client`, and returns whether this call did: one `lock bts`, or `ldsetl`.
    fn subscribe(client: u32) -> bool {
        !SUBSCRIBED.bit_set(client, Release)
    }

    assert!(subscribe(37), "client 37 subscribed");
    assert!(!subscribe(37), "once");
    // The bit counts modulo the width, as a shift by `wrapping_shl` does: 101 is 37.
    assert!(!subscribe(101), "so client 101 finds client 37's bit");
    assert_eq!(SUBSCRIBED.load(Acquire), 1 << 37, "one bit set");
}
// ANCHOR_END: bit

#[test]
// ANCHOR: pointer_mask
fn a_raw_pointer_takes_a_mask_and_keeps_its_provenance() {
    use core::ptr;

    use atomix::AtomicPtr;
    use atomix::ordering::{Acquire, Release};

    /// A node, aligned to 8, so a pointer to one leaves bit 0 clear for a mark.
    #[repr(align(8))]
    struct Node {
        value: u64,
    }

    let node = Node { value: 7 };
    let next = AtomicPtr::new(ptr::from_ref(&node).cast_mut());
    // A `usize` mask on the address: `lock or`, or `ldsetl`, on the pointer itself.
    next.or(1, Release);
    let marked = next.load(Acquire);
    assert_eq!(marked.addr() & 1, 1, "marked in bit 0");

    let unmarked = marked.map_addr(|address| address & !1);
    #[expect(
        unsafe_code,
        reason = "only a read through the pointer read back shows the mask kept its provenance"
    )]
    // SAFETY: `unmarked` points to `node`, which lives, with the provenance the mask kept.
    let value = unsafe { (*unmarked).value };
    assert_eq!(value, 7, "the node, read through the pointer the atomic gave back");
}
// ANCHOR_END: pointer_mask

#[test]
// ANCHOR: constant
fn an_atomic_is_read_and_written_in_a_constant() {
    use atomix::AtomicU64;
    use atomix::ordering::Relaxed;

    // A constant runs on one thread, so its orderings order nothing, but each operation works
    // as it does at run time.
    const HANDED_OUT: u64 = {
        let next = AtomicU64::new(1);
        let (first, second) = (next.fetch_add(1, Relaxed), next.fetch_add(1, Relaxed));
        assert!(first == 1 && second == 2, "two ids handed out, in order");
        next.load(Relaxed)
    };
    assert_eq!(HANDED_OUT, 3, "the id after them, worked out as the crate built");
}
// ANCHOR_END: constant
