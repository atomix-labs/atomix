//! The listings of the chapter Fields, whose book of quotes is the quote-book example's.

#[path = "../../examples/quote-book/quote_book.rs"]
mod quote_book;

#[test]
fn a_quote_packs_its_side_and_live_bit_above_its_quantity() {
    use atomix::Atom;
    use quote_book::{Quote, Side};

    let repr = Quote { quantity: 300, side: Side::Ask, live: true }.to_repr();
    assert_eq!((repr >> 32, repr & 0xFFFF_FFFF), (0b11, 300), "live and the side above 300");
}

#[test]
// ANCHOR: book
fn each_quote_is_taken_whole_by_one_taker() {
    use core::num::NonZero;
    use std::thread;

    use quote_book::{Book, Quote, Side};

    let posted = Quote { quantity: 300, side: Side::Ask, live: true };
    let book = Book::new(1);
    let quantity = NonZero::new(300).expect("300 is not zero");
    assert!(book.post(0, Side::Ask, quantity), "the level was free");
    assert!(!book.post(0, Side::Bid, quantity), "and holds one quote at a time");
    let taken: Vec<Quote> = thread::scope(|scope| {
        let takers = [(); 4].map(|()| scope.spawn(|| book.take(0)));
        takers.into_iter().filter_map(|taker| taker.join().expect("no taker panics")).collect()
    });
    assert_eq!(taken, [Quote { live: false, ..posted }], "one taker took the quote, whole");
    assert!(book.post(0, Side::Bid, quantity), "and freed the level for the next");
}
// ANCHOR_END: book

#[test]
// ANCHOR: order
fn each_field_changes_alone() {
    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic, RangedU32};

    /// What an order may do: whether it is halted, and whether it only rests on the book.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Flags {
        halted: bool,
        post_only: bool,
    }

    /// An order: its flags, its price in ticks below 2^30, then how much of it filled, at the top.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Order {
        flags: Flags,
        price: RangedU32<0, 0x3FFF_FFFF>,
        filled: u32,
    }

    let price = RangedU32::new(10_050).expect("below 2^30 ticks");
    let flags = Flags { halted: false, post_only: true };
    let order = Atomic::new(Order { flags, price, filled: 0 });

    // The count at the word's top bit adds in place, its carry leaving the word: `lock xadd`, or
    // `ldaddal`, which returns the whole order before.
    assert_eq!(order.fields().filled.fetch_add(300, AcqRel).filled, 0, "nothing filled before");
    // A field of a field: `lock bts`, or `ldsetal`, on bit 0 of the order.
    assert!(!order.fields().flags.fields().halted.test_and_set(AcqRel), "this call halted it");
    order.fields().flags.fields().post_only.clear(Release);
    // Any other change of a field is a loop, by name.
    order.fields().price.update(AcqRel, Acquire, |price| price.saturating_sub(50));

    let flags = Flags { halted: true, post_only: false };
    let repriced = RangedU32::new(10_000).expect("below 2^30 ticks");
    assert_eq!(order.load(Acquire), Order { flags, price: repriced, filled: 300 }, "each change");
}
// ANCHOR_END: order

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[test]
// ANCHOR: fetch_forms
fn a_fetch_form_returns_the_container_before() {
    use atomix::ordering::AcqRel;
    use atomix::{Atom, Atomic};

    /// A price level's quantity, and its flags.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Level {
        quantity: u32,
        flags: u8,
    }

    let level = Atomic::new(Level { quantity: 300, flags: 0b01 });
    // `ldsetal` returns the word it found: every field, as one snapshot.
    let before = level.fields().flags.fetch_or(0b10, AcqRel);
    assert_eq!(before, Level { quantity: 300, flags: 0b01 }, "the level the flag went into");
}
// ANCHOR_END: fetch_forms
