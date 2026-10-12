//! The listings of the chapter Getting Started.

#[test]
// ANCHOR: first
fn a_quote_or_its_absence_takes_one_word() {
    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic};

    /// The side of the book a quote rests on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Side {
        Bid,
        Ask,
    }

    /// A resting quote: 32 bits of price, 16 of quantity, then a bit of side, in a `u64`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Quote {
        price: u32,
        quantity: u16,
        side: Side,
    }

    // `None` takes a bit pattern no quote has, so the `Option` costs no bit.
    static BEST_BID: Atomic<Option<Quote>> = Atomic::new(None);

    BEST_BID.store(Some(Quote { price: 10_050, quantity: 300, side: Side::Bid }), Release);
    let taken = BEST_BID.swap(None, AcqRel);
    assert_eq!(taken.map(|quote| quote.quantity), Some(300), "the quote taken whole");
    assert_eq!(BEST_BID.load(Acquire), None, "and the level empty");
    assert_eq!(size_of_val(&BEST_BID), 8, "a quote, or none, in one `u64`");
}
// ANCHOR_END: first
