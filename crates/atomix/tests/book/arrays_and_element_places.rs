//! The listings of the chapter Arrays and Element Places.

#[test]
// ANCHOR: slots
fn an_array_takes_a_slot_as_wide_as_its_elements_repr_for_each() {
    use core::num::NonZero;

    use atomix::{Atom, Atomic};

    // Four bytes are their own bytes, element 0 lowest, in a `u32`.
    assert_eq!([1_u8, 2, 3, 4].to_repr(), u32::from_le_bytes([1, 2, 3, 4]), "a byte a slot");
    // 64 `bool`s are a bit each, in a `u64`.
    let mut busy = [false; 64];
    busy[3] = true;
    assert_eq!(busy.to_repr(), 1 << 3, "a bit a slot");
    // Three bytes leave the top byte of a `u32` clear, so `None` takes a repr there.
    assert_eq!(size_of::<Atomic<Option<[u8; 3]>>>(), 4, "three bytes, or none, in a `u32`");
    // An element that is never zero leaves zero to `None`.
    assert_eq!(None::<[NonZero<u8>; 4]>.to_repr(), 0, "`None` at zero");
}
// ANCHOR_END: slots

#[test]
// ANCHOR: elements
fn each_element_is_a_place_of_its_own() {
    use atomix::Atomic;
    use atomix::ordering::{AcqRel, Acquire, Release};

    /// Which of 64 workers are busy, a bit each, in one `u64`.
    static BUSY: Atomic<[bool; 64]> = Atomic::new([false; 64]);
    /// How deep each of three price levels is, a byte each, and how many quotes the book has
    /// taken, in the top byte: one `u32`.
    static BOOK: Atomic<[u8; 4]> = Atomic::new([0; 4]);

    // At an index known at run time: the index's check, then `lock bts`, or `ldsetal`.
    let worker = 17;
    assert!(!BUSY.element(worker).test_and_set(AcqRel), "this thread claimed the worker");
    assert!(BUSY.element_checked(64).is_none(), "and none lies past the end");

    // At a constant index, a field's place: no check.
    BOOK.place::<1>().or(0x80, Release);
    // The last element adds in place, its carry leaving the word: `lock xadd`, or `ldaddal`.
    BOOK.last().fetch_add(1, AcqRel);
    let [best, second, _, taken] = BOOK.elements();
    assert_eq!((best.load(Acquire), second.load(Acquire)), (0, 0x80), "each level its own");
    assert_eq!(taken.load(Acquire), 1, "and one quote taken");
}
// ANCHOR_END: elements

#[test]
// ANCHOR: element_fields
fn an_elements_fields_are_places_at_a_run_time_index_too() {
    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic};

    /// A slot of a ring a producer fills: whether it holds a message, and the message's length.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Slot {
        written: bool,
        length: u8,
    }

    /// Four slots, sixteen bits each, in one `u64`.
    static RING: Atomic<[Slot; 4]> = Atomic::new([Slot { written: false, length: 0 }; 4]);

    let index = 2;
    // The index's check, a shift by the slot, then `lock bts`, or `ldsetal`.
    assert!(!RING.element(index).fields().written.test_and_set(AcqRel), "the slot claimed");
    RING.element(index).fields().length.update(AcqRel, Acquire, |_| 9);
    assert_eq!(RING.load(Acquire)[index], Slot { written: true, length: 9 }, "slot 2 alone");
    RING.element(index).fields().written.clear(Release);
}
// ANCHOR_END: element_fields

#[test]
// ANCHOR: ranged_index
fn an_index_held_to_the_range_needs_no_check() {
    use atomix::ordering::{AcqRel, Acquire};
    use atomix::{Atomic, RangedUsize};

    /// A worker's index, which the type holds below 64.
    type Worker = RangedUsize<0, 63>;

    let busy = Atomic::new([false; 64]);
    let worker = Worker::new(17).expect("17 lies below 64");
    // LLVM knows the index's range, so the check folds away: `lock bts`, or `ldsetal`, alone.
    assert!(!busy.element(worker.get()).test_and_set(AcqRel), "this thread claimed it");
    assert!(busy.load(Acquire)[17], "the one bit set");
}
// ANCHOR_END: ranged_index

#[test]
// ANCHOR: grid
fn an_element_of_an_element_is_a_place_too() {
    use atomix::Atomic;
    use atomix::ordering::{AcqRel, Acquire};

    // Which squares of an eight by eight board are taken, a bit each, in one `u64`.
    let board = Atomic::new([[false; 8]; 8]);
    // Both indices checked, then their shifts joined: one `lock bts`, or `ldsetal`.
    assert!(!board.element(3).element(5).test_and_set(AcqRel), "the square taken once");
    assert!(board.load(Acquire)[3][5], "row 3, column 5");
}
// ANCHOR_END: grid

#[test]
// ANCHOR: packed
fn a_packed_array_takes_the_bits_its_elements_need() {
    use atomix::ordering::{AcqRel, Acquire};
    use atomix::{Atom, Atomic, Packed};

    /// The side of the book a venue quotes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Side {
        Bid,
        Ask,
    }

    // A side takes one bit as a field, so eight fit a byte; as an array's slots, eight bytes.
    let sides = Atomic::new(Packed::<[Side; 8], u8>::new([Side::Bid; 8]));
    assert_eq!(size_of_val(&sides), 1, "eight sides in a byte");
    assert_eq!(size_of::<Atomic<[Side; 8]>>(), 8, "where slots take a byte each");

    sides.element(5).update(AcqRel, Acquire, |_| Side::Ask);
    let read = sides.load(Acquire);
    assert_eq!(read[5], Side::Ask, "venue 5's side, read through the array");
    assert_eq!(read.to_repr(), 1 << 5, "its one bit");
}
// ANCHOR_END: packed
