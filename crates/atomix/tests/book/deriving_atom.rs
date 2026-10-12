//! The listings of the chapter Deriving Atom.

#[test]
// ANCHOR: packed
fn a_packed_struct_lays_its_fields_out_from_bit_0() {
    use atomix::{Atom, Atomic};

    /// The side of the book a quote rests on: two values, so one bit.
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

    let repr = Quote { price: 10_050, quantity: 300, side: Side::Ask }.to_repr();
    assert_eq!(repr & 0xFFFF_FFFF, 10_050, "the price in bits 0 to 31");
    assert_eq!(repr >> 32 & 0xFFFF, 300, "the quantity in bits 32 to 47");
    assert_eq!(repr >> 48, 1, "and the side, `Ask`, in bit 48");
    assert_eq!(None::<Quote>.to_repr(), 1 << 49, "and `None` past the 49 bits the fields take");
    assert_eq!(size_of::<Atomic<Option<Quote>>>(), 8, "so a quote, or none, takes a `u64`");
}
// ANCHOR_END: packed

#[test]
// ANCHOR: signed
fn a_signed_field_takes_twos_complement_and_the_bits_above() {
    use atomix::{Atom, RangedI8};

    /// A price's move in ticks, from -5 to 5, beside a byte of flags.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Move {
        flags: u8,
        ticks: RangedI8<-5, 5>,
    }

    let down = Move { flags: 1, ticks: RangedI8::new(-1).expect("-1 is from -5 to 5") };
    let up = Move { flags: 1, ticks: RangedI8::new(1).expect("1 is from -5 to 5") };
    // `ticks` needs four bits in two's complement, and fills the `u16` above the flags: its sign
    // extends through bit 15.
    assert_eq!(down.to_repr(), 0xFF01, "-1 above the flags");
    assert_eq!(up.to_repr(), 0x0101, "and 1");
}
// ANCHOR_END: signed

#[test]
// ANCHOR: enum_with_fields
fn an_enum_with_fields_keeps_a_tag_above_them() {
    use atomix::Atom;

    /// A ring buffer's slot: two variants with fields, so a tag of two bits above a lap's 32.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Slot {
        Empty,
        Writing { lap: u32 },
        Ready { lap: u32 },
    }

    assert_eq!(Slot::Empty.to_repr(), 0, "tag 0");
    assert_eq!(Slot::Writing { lap: 7 }.to_repr(), 1 << 32 | 7, "tag 1 above the lap");
    assert_eq!(Slot::Ready { lap: 7 }.to_repr(), 2 << 32 | 7, "tag 2");
    assert_eq!(None::<Slot>.to_repr(), 3 << 32, "and `None` the tag after them");
}
// ANCHOR_END: enum_with_fields

#[test]
// ANCHOR: niche
fn an_enum_with_one_variant_of_fields_fills_its_niche() {
    use atomix::{Atom, Atomic, RangedU64};

    /// An owner's id, from 3, which leaves 0 to 2 spare.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct OwnerId(RangedU64<3>);

    /// A lock: one variant with fields, so the unit variants take the reprs below the ids.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum LockState {
        Free,
        Poisoned,
        Held(OwnerId),
    }

    let owner = OwnerId(RangedU64::new(7).expect("7 is from 3 up"));
    assert_eq!(LockState::Held(owner).to_repr(), 7, "an id as it is");
    assert_eq!(
        (LockState::Free.to_repr(), LockState::Poisoned.to_repr()),
        (1, 2),
        "the states below 3"
    );
    assert_eq!(None::<LockState>.to_repr(), 0, "and `None` below them");
    assert_eq!(size_of::<Atomic<Option<LockState>>>(), 8, "all in the id's `u64`");
}
// ANCHOR_END: niche

#[test]
// ANCHOR: generic
fn a_generic_type_is_laid_out_for_each_instance() {
    use atomix::ordering::{AcqRel, Acquire};
    use atomix::{Atom, Atomic, RangedU64};

    /// An owner's id, from 3.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct OwnerId(RangedU64<3>);

    /// A lock over any owner that fits a `u64` with its states.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    #[atom(repr = u64)]
    enum LockState<O> {
        Free,
        Poisoned,
        Held(O),
    }

    // The impl is `const` where `O`'s is, so a `static` of an instance builds.
    static LOCK: Atomic<LockState<OwnerId>> = Atomic::new(LockState::Free);

    // An id leaves reprs spare, so the states fill them; a `u32` leaves none, so a tag goes above.
    assert_eq!(LockState::<OwnerId>::Free.to_repr(), 1, "below the ids");
    assert_eq!(LockState::Held(7_u32).to_repr(), 2 << 32 | 7, "tag 2 above the `u32`");

    let owner = OwnerId(RangedU64::new(7).expect("7 is from 3 up"));
    let taken = LOCK.compare_exchange(LockState::Free, LockState::Held(owner), AcqRel, Acquire);
    assert_eq!(taken, Ok(LockState::Free), "the lock taken");
}
// ANCHOR_END: generic
