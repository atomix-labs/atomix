//! The listings of the chapter Values and Places.

#[test]
// ANCHOR: nesting
fn values_nest_within_one_word() {
    use atomix::ordering::{Acquire, Release};
    use atomix::{Atom, Atomic, RangedU16};

    /// The side of the book an order rests on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Side {
        Bid,
        Ask,
    }

    /// A session's id, from 3, so a holder's `Option` keeps its `None` below the ids.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct SessionId(RangedU16<3>);

    /// An order: its side, the session that holds it, if one does, and its quantity.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Order {
        side: Side,
        holder: Option<SessionId>,
        quantity: u32,
    }

    // A struct of an enum, an `Option` of a newtype of a ranged integer, and an integer: 49 bits.
    static ORDER: Atomic<Option<Order>> = Atomic::new(None);

    let holder = Some(SessionId(RangedU16::new(7).expect("7 is from 3 up")));
    let order = Order { side: Side::Ask, holder, quantity: 300 };
    ORDER.store(Some(order), Release);
    assert_eq!(ORDER.load(Acquire), Some(order), "the order, whole");
    assert_eq!(size_of_val(&ORDER), 8, "in one `u64`, `None` too");
}
// ANCHOR_END: nesting

#[test]
// ANCHOR: side_by_side
fn atomics_sit_side_by_side() {
    use atomix::ordering::Relaxed;
    use atomix::{AtomicBool, AtomicU64};

    /// Fills counted per venue, each its own atomic.
    static FILLS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

    /// A session's state: atomics side by side in a struct, each changed alone.
    struct Session {
        sequence: AtomicU64,
        halted: AtomicBool,
    }

    // ORDERING: Relaxed throughout: each atomic publishes only itself.
    FILLS[2].fetch_add(1, Relaxed);
    assert_eq!(FILLS[2].load(Relaxed), 1, "one venue's count");

    // A count per level, as many as a book has, in a boxed slice.
    let levels: Box<[AtomicU64]> = (0..16).map(|_| AtomicU64::new(0)).collect();
    levels[15].fetch_add(300, Relaxed);
    assert_eq!(levels[15].load(Relaxed), 300, "the last level's");

    let session = Session { sequence: AtomicU64::new(0), halted: AtomicBool::new(false) };
    session.sequence.fetch_add(1, Relaxed);
    session.halted.store(true, Relaxed);
    assert!(session.halted.load(Relaxed), "the session halted");
    assert_eq!(session.sequence.load(Relaxed), 1, "after one message");
}
// ANCHOR_END: side_by_side

#[test]
// ANCHOR: exclusive
fn a_mutable_borrow_reads_and_writes_the_value_in_place() {
    use atomix::ordering::Relaxed;
    use atomix::{Atom, Atomic, AtomicU32, AtomicU64};

    /// A resting quote.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Quote {
        price: u32,
        quantity: u16,
    }

    // Before the atomic is shared, or once it is shared no more, `&mut` reads and writes it.
    let mut quote = Atomic::new(Quote { price: 10_050, quantity: 300 });
    quote.set(Quote { price: 10_100, quantity: 200 });
    quote.with_mut(|quote| quote.quantity += 100);
    assert_eq!(quote.get(), Quote { price: 10_100, quantity: 300 }, "written in place");
    assert_eq!(quote.into_inner().price, 10_100, "and taken out");

    // A primitive, every bit pattern of which is a value, lends its place both ways.
    let mut fills = AtomicU64::new(7);
    *fills.get_mut() += 1;
    let mut plain = 8_u64;
    assert_eq!(
        AtomicU64::from_mut(&mut plain).load(Relaxed),
        fills.load(Relaxed),
        "8 written through `get_mut`, and read through `from_mut`"
    );

    let mut counts = [const { AtomicU32::new(5) }; 3];
    Atomic::get_mut_slice(&mut counts).fill(0);
    let mut plain_counts = [0_u32; 3];
    let shared: &[AtomicU32] = Atomic::from_mut_slice(&mut plain_counts);
    shared[1].fetch_add(1, Relaxed);
    assert_eq!(counts.map(AtomicU32::into_inner), [0; 3], "every count reset");
    assert_eq!(plain_counts, [0, 1, 0], "and a plain array counted through atomics");
}
// ANCHOR_END: exclusive
