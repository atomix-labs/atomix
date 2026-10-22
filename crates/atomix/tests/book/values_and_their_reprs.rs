//! The listings of the chapter Values and Their Reprs.

#[test]
// ANCHOR: reprs
fn a_value_goes_in_as_its_repr_and_comes_out_decoded() {
    use atomix::ordering::{Acquire, Release};
    use atomix::{Atom, Atomic, ReprRange};

    // A `char` is stored as its scalar value, a `u32` from 0 to 0x10FFFF.
    assert_eq!('é'.to_repr(), 0xE9, "a char's repr");
    assert_eq!(char::REPRS, ReprRange::new(0, 0x10_FFFF), "and the reprs a char takes");
    // `from_repr` decodes a repr, and refuses one no value encodes to: a surrogate is no char.
    assert_eq!(char::from_repr(0xE9), Some('é'), "a repr decoded");
    assert_eq!(char::from_repr(0xD800), None, "and a surrogate refused");

    // An atomic holds only reprs a value encoded to, so a load decodes them without a check.
    let letter = Atomic::new('é');
    letter.store('ß', Release);
    assert_eq!(letter.load(Acquire), 'ß', "the char stored");
}
// ANCHOR_END: reprs

#[test]
// ANCHOR: none
fn none_takes_a_repr_no_value_takes() {
    use core::num::NonZero;

    use atomix::{Atom, Atomic, ReprRange};

    /// The side of the book a quote rests on, 0 or 1.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Side {
        Bid,
        Ask,
    }

    /// A price's last move: -1, 0 or 1, as an `i8`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    #[repr(i8)]
    enum Tick {
        Down = -1,
        Flat = 0,
        Up   = 1,
    }

    // -1 is 0xFF in an `i8`'s bits, so `Tick`'s reprs run from 0xFF through zero to 0x01.
    assert_eq!(Tick::REPRS, ReprRange::from_signed(-1, 1), "a range through zero");
    let ends = (Tick::REPRS.contains(0xFF), Tick::REPRS.contains(0xFE));
    assert_eq!(ends, (true, false), "from -1, not -2");

    // Zero, where no value is zero.
    assert_eq!(None::<NonZero<u64>>.to_repr(), 0, "no `NonZero` is zero");
    // Else a repr beside the range: past its end, or before its start where that keeps the field
    // narrower.
    assert_eq!(None::<char>.to_repr(), 0x11_0000, "past the last scalar value");
    assert_eq!(None::<Side>.to_repr(), 2, "past `Ask`");
    assert_eq!(None::<Tick>.to_repr(), -2, "before `Down`, so -2 to 1 fits two bits");
    // So an `Option` costs no width.
    assert_eq!(size_of::<Atomic<Option<char>>>(), size_of::<Atomic<char>>(), "a `u32` still");
}
// ANCHOR_END: none

#[test]
// ANCHOR: hand_impl
fn a_hand_impl_decodes_exactly_its_range() {
    use atomix::ordering::{Acquire, Release};
    use atomix::{Atom, Atomic, RangedU8, ReprRange};

    /// A share of a whole, from 0 to 100 percent.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Percent(
        // INVARIANT: at most 100; `new` is its one writer.
        u8,
    );

    impl Percent {
        /// The share of `percent`, or `None` past 100.
        const fn new(percent: u8) -> Option<Self> {
            if percent <= 100 { Some(Self(percent)) } else { None }
        }
    }

    // SAFETY: by the field's INVARIANT, `to_repr` gives 0 to 100, within `REPRS`;
    // `from_repr` decodes exactly those, by the repr alone, each as the percent it came from; the
    // default `from_repr_unchecked` unwraps `from_repr`; `Partial`, the default validity, promises
    // no other repr; and a percent holds no pointer, so it may cross threads.
    #[expect(unsafe_code, reason = "a load decodes a percent without a check, trusting the impl")]
    const unsafe impl Atom for Percent {
        type Repr = u8;
        const REPRS: ReprRange<u8> = ReprRange::new(0, 100);
        fn to_repr(self) -> u8 {
            self.0
        }
        fn from_repr(repr: u8) -> Option<Self> {
            Self::new(repr)
        }
    }

    /// The same share, derived over a ranged integer: no `unsafe`, and nothing to prove.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct DerivedPercent(RangedU8<0, 100>);

    static PROGRESS: Atomic<Option<Percent>> = Atomic::new(None);

    PROGRESS.store(Percent::new(42), Release);
    assert_eq!(PROGRESS.load(Acquire), Percent::new(42), "the share stored");
    assert_eq!(Percent::from_repr(100), Percent::new(100), "100 decodes");
    assert_eq!(Percent::from_repr(101), None, "and 101 does not");
    assert_eq!(None::<Percent>.to_repr(), 101, "so `None` takes it, in the same byte");
    assert_eq!(DerivedPercent::REPRS, Percent::REPRS, "and the same reprs, derived");
}
// ANCHOR_END: hand_impl
