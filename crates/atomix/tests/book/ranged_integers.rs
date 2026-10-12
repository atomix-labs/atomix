//! The listings of the chapter Ranged Integers.

#[test]
// ANCHOR: niche
fn a_ranged_integer_leaves_its_option_a_niche_in_plain_rust_too() {
    use atomix::{Atomic, RangedU64};
    use deranged::RangedU64 as DerangedU64;

    /// How many levels of an order book a feed sends, 1 to 10.
    type Depth = RangedU64<1, 10>;

    // The range is the type's validity, as `NonZero`'s is, so rustc keeps `None` outside it.
    assert_eq!(size_of::<Option<Depth>>(), 8, "a `u64`, in a struct or a `Vec`");
    assert_eq!(size_of::<Option<DerangedU64<1, 10>>>(), 16, "where deranged's takes two");
    // In an atomic, atomix's `REPRS` keeps `None` outside it for either.
    assert_eq!(size_of::<Atomic<Option<Depth>>>(), 8, "and a `u64` in an atomic");
}
// ANCHOR_END: niche

#[test]
// ANCHOR: api
fn a_ranged_integer_holds_its_range_through_each_operation() {
    use atomix::RangedU8;

    /// How many levels of an order book a feed sends, 1 to 10.
    type Depth = RangedU8<1, 10>;

    let depth = Depth::new(4).expect("4 is from 1 to 10");
    assert_eq!(Depth::new(11), None, "11 is refused");
    assert_eq!(depth.checked_add(7), None, "as is a sum past 10");
    assert_eq!(depth.saturating_add(7), Depth::MAX, "which saturates at 10");

    // Text and other integers convert with a check, and the error names the integer and range.
    assert_eq!("10".parse::<Depth>(), Ok(Depth::MAX), "10, parsed");
    let refused = Depth::try_from(11_u8).expect_err("11 is past 10");
    assert_eq!(refused.to_string(), "range error: 11 is not in 1..=10", "refused, with why");

    // Another range of the integer: wider, which needs no check, or narrower, which does.
    let kept: RangedU8<0, 50> = depth.expand();
    assert_eq!(kept.narrow::<1, 3>(), None, "4 is not from 1 to 3");
    assert_eq!(depth.to_non_zero().get(), 4, "and a range without zero, to a `NonZero`");

    // A range of values iterates, each value in the range.
    let levels: Vec<u8> = (Depth::MIN..=depth).map(Depth::get).collect();
    assert_eq!(levels, [1, 2, 3, 4], "the levels up to 4");
}
// ANCHOR_END: api

#[test]
// ANCHOR: field_width
fn a_ranged_field_takes_the_bits_its_range_needs() {
    use atomix::ordering::{AcqRel, Acquire};
    use atomix::{Atom, Atomic, RangedU32};

    /// An order: whether it may fill, its price in ticks below 2^31, then how much filled.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Order {
        live: bool,
        price: RangedU32<0, 0x7FFF_FFFF>,
        filled: u32,
    }

    let price = RangedU32::new(10_050).expect("below 2^31 ticks");
    let order = Order { live: true, price, filled: 0 };
    // One bit, then 31 for the price, so `filled` starts at bit 32, the top half of a `u64`.
    assert_eq!(order.to_repr(), 10_050 << 1 | 1, "the price above `live`");

    // At the top, `filled` adds in place: one `lock xadd`, or `ldaddal`.
    let atomic = Atomic::new(order);
    atomic.fields().filled.fetch_add(300, AcqRel);
    assert_eq!(atomic.load(Acquire).filled, 300, "filled, the rest as it was");
}
// ANCHOR_END: field_width
