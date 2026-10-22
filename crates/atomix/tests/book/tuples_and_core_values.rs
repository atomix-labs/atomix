//! The listings of the chapter Tuples and Core's Values.

#[test]
// ANCHOR: slots
fn each_field_of_a_tuple_takes_a_slot_as_wide_as_its_repr() {
    use atomix::Atom;

    /// The side of the book a quote rests on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Side {
        Bid,
        Ask,
    }

    /// Two sides, a derived struct.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Sides {
        first: Side,
        second: Side,
    }

    // A side's repr is a `u8`, so a tuple gives each a byte; the struct packs a bit each.
    assert_eq!((Side::Ask, Side::Ask).to_repr(), 0x101, "a byte's slot each, in a `u16`");
    let sides = Sides { first: Side::Ask, second: Side::Ask };
    assert_eq!(sides.to_repr(), 0b11, "a bit each, in a `u8`");
}
// ANCHOR_END: slots

#[test]
// ANCHOR: tuples
fn a_tuples_fields_are_places() {
    use atomix::Atomic;
    use atomix::ordering::{AcqRel, Acquire, Release};

    // A price, whether the quote is live, and its size: 32, 1 and 8 bits in a `u64`.
    let quote = Atomic::new((10_050_u32, false, 3_u8));
    assert!(!quote.fields().1.test_and_set(AcqRel), "made live once: `lock bts`, or `ldsetal`");
    quote.fields().2.or(0x80, Release);
    assert_eq!(quote.load(Acquire), (10_050, true, 0x83), "each field changed alone");
    assert_eq!(size_of_val(&quote), 8, "in one `u64`");
}
// ANCHOR_END: tuples

#[test]
// ANCHOR: beside_a_pointer
fn a_tuple_keeps_small_fields_in_its_pointers_clear_bits() {
    use core::ptr::NonNull;

    use atomix::{Atom, Atomic};

    /// A node, aligned to 8: three low bits clear.
    #[repr(align(8))]
    struct Node {
        value: u64,
    }

    /// The struct of the same fields, a pointer word.
    #[derive(Clone, Copy, Atom)]
    struct Head {
        top: NonNull<Node>,
        closed: bool,
    }

    let node = Node { value: 7 };
    let top = NonNull::from_ref(&node);
    assert_eq!((top, true).to_repr(), Head { top, closed: true }.to_repr(), "the struct's repr");
    assert_eq!(size_of::<Atomic<(NonNull<Node>, bool)>>(), 8, "one word");
    assert_eq!(node.value, 7, "the node it points to");
}
// ANCHOR_END: beside_a_pointer

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
#[test]
// ANCHOR: beside_a_function
fn a_function_pointer_keeps_a_tuples_other_fields_in_a_word_of_their_own() {
    use atomix::Atomic;
    use atomix::ordering::{Acquire, Release};

    /// Twice `value`: a handler.
    fn twice(value: u64) -> u64 {
        value.wrapping_mul(2)
    }

    // Code leaves no bit of its pointer clear, so the flag takes the second word.
    let armed = Atomic::new((None::<fn(u64) -> u64>, false));
    assert_eq!(size_of_val(&armed), 16, "two words");
    armed.store_rmw((Some(twice), true), Release);
    let (handler, live) = armed.load_rmw(Acquire);
    assert_eq!((handler.map(|handle| handle(21)), live), (Some(42), true), "both, read whole");
}
// ANCHOR_END: beside_a_function

#[test]
// ANCHOR: core_values
fn cores_small_values_are_atomic_values() {
    use core::cmp::Ordering;
    use core::net::Ipv4Addr;

    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic};

    /// The address a feed listens on: a `u32` of its octets, in the order core keeps them.
    static LISTEN: Atomic<Ipv4Addr> = Atomic::new(Ipv4Addr::new(10, 1, 2, 3));
    // A netmask applies octet by octet: one `lock and`, or `ldclral`.
    LISTEN.and(Ipv4Addr::new(255, 255, 0, 0), Release);
    assert_eq!(LISTEN.load(Acquire), Ipv4Addr::new(10, 1, 0, 0), "the network");

    // An `Ordering` is its `i8`: `fetch_max` is one `ldsmaxalb` on `aarch64`, `update` anywhere.
    let worst = Atomic::new(Ordering::Less);
    let before = worst.update(AcqRel, Acquire, |seen| seen.max(Ordering::Equal));
    assert_eq!(before, Ordering::Less, "the ordering before");
    assert_eq!(worst.load(Acquire), Ordering::Equal, "the worst seen");

    // A `Result` of a `u32` and a `u8` keeps its variant as a tag above the wider: a `u64`.
    let outcome: Atomic<Result<u32, u8>> = Atomic::new(Ok(7));
    outcome.store(Err(3), Release);
    assert_eq!(outcome.load(Acquire), Err(3), "the refusal's code");
    assert_eq!(Err::<u32, u8>(3).to_repr(), 1 << 32 | 3, "its tag, 1, above the payload");
    assert_eq!(size_of_val(&outcome), 8, "in one `u64`");
}
// ANCHOR_END: core_values

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
#[test]
// ANCHOR: durations
fn a_duration_keeps_its_seconds_above_its_nanoseconds() {
    use core::time::Duration;

    use atomix::Atomic;
    use atomix::ordering::{AcqRel, Acquire};

    // Seconds in the upper word and nanoseconds in the lower, so the repr orders as the time.
    let longest = Atomic::new(Duration::ZERO);
    for wait in [Duration::from_millis(3), Duration::from_micros(2_500)] {
        longest.update(AcqRel, Acquire, |seen| seen.max(wait));
    }
    assert_eq!(longest.load_rmw(Acquire), Duration::from_millis(3), "the longest wait");
}
// ANCHOR_END: durations

#[cfg(all(
    unix,
    feature = "std",
    any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b")
))]
#[test]
// ANCHOR: std_values
fn stds_values_come_with_the_std_feature() {
    use core::time::Duration;
    use std::time::SystemTime;

    use atomix::Atomic;
    use atomix::ordering::{Acquire, Release};

    // Whole seconds since the Unix epoch above the nanoseconds, signed: zero is the epoch.
    let heartbeat = Atomic::from(SystemTime::UNIX_EPOCH);
    let beat = SystemTime::UNIX_EPOCH + Duration::new(1_760_000_000, 123_456_789);
    heartbeat.store_rmw(beat, Release);
    assert_eq!(heartbeat.load_rmw(Acquire), beat, "the last heartbeat, to the nanosecond");
}
// ANCHOR_END: std_values
