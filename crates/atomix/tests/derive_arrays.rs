//! Arrays in derived values: a newtype over an array, a packed struct with an array field, padded
//! or filling its slots, an array of packed structs, a pointer word whose tags are an array, beside
//! a pointer or a reference, and two words whose integer word is one; and the places each projects
//! onto, at an index known at run time or a constant, an element's fields among them.

#![cfg(feature = "derive")]
#![cfg(not(loom))]

#[cfg(test)]
#[path = "../../atomix-core/tests/testing/mod.rs"]
mod testing;

#[cfg(test)]
mod tests {
    use atomix::ordering::{AcqRel, Acquire, Relaxed, Release};
    use atomix::validity::{Total, ZeroValid};
    use atomix::{Atom, AtomBitwise, Atomic, RangedI8};

    use crate::testing::atom::{field_width, repr_and_validity_are};
    use crate::testing::law::{assert_holds, decodes_as_promised};

    /// The depth of each of four price levels, a byte each.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom, AtomBitwise)]
    struct Levels([u8; 4]);

    /// A book's four levels beside whether it trades.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Book {
        /// The depth of each level.
        levels: [u8; 4],
        /// Whether the book trades.
        live: bool,
    }

    /// Which of ten seats are taken, beside whether the table is open: the array leaves bits of
    /// its `u16` above it, but its field is its ten bits alone.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Table {
        /// One bit per seat.
        taken: [bool; 10],
        /// Whether the table takes players.
        open: bool,
    }

    /// The side of the book an order rests on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Side {
        /// A buy.
        Bid,
        /// A sell.
        Ask,
    }

    /// A board of four cells, each a byte, the side whose turn it is above them.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Board {
        /// Who holds each cell.
        cells: [Side; 4],
        /// Who moves next.
        turn: Side,
    }

    /// A ring buffer slot: whether it was written, and in which lap.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Slot {
        /// Whether a writer filled it.
        written: bool,
        /// The lap it was filled in.
        lap: u8,
    }

    /// A ring of three slots, a byte each, then which is its head.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Ring {
        /// The slots.
        slots: [Slot; 3],
        /// Which slot is the head.
        head: u8,
    }

    /// A tick: whether it traded, and how far the price moved, whose sign the bits above copy.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Tick {
        /// Whether it traded.
        traded: bool,
        /// How far the price moved.
        moved: RangedI8<-4, 3>,
    }

    /// A quantity, and four venues of no bits above its 32: a field of no bits lies at its
    /// running offset, the repr's top.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Venued {
        /// How much rests.
        quantity: u32,
        /// Four venues, each of no bits.
        venues: [(); 4],
    }

    #[test]
    fn an_array_of_no_bits_at_the_top_lends_each_element() {
        let venued = Atomic::new(Venued { quantity: 7, venues: [(); 4] });
        for venue in 0..4 {
            venued.fields().venues.element(venue).update(AcqRel, Acquire, |unit| unit);
            assert_eq!(venued.fields().venues.element(venue).load(Acquire), (), "read back");
        }
        assert_eq!(venued.load(Acquire).quantity, 7, "the quantity kept");
    }

    #[test]
    fn a_newtype_over_bytes_is_its_bytes() {
        repr_and_validity_are::<Levels, u32, Total>();
        let levels = Atomic::new(Levels([1, 2, 3, 4]));
        levels.or(Levels([0x10, 0, 0, 0]), Release);
        assert_eq!(levels.load(Acquire), Levels([0x11, 2, 3, 4]), "each element with its own");
    }

    #[test]
    fn an_array_field_packs_at_its_width_and_lends_its_elements() {
        repr_and_validity_are::<Book, u64, ZeroValid>();
        assert_eq!(field_width(<[u8; 4] as Atom>::REPRS), 32, "four bytes take 32 bits");
        let book = Atomic::new(Book { levels: [1, 2, 3, 4], live: true });
        assert_eq!(book.fields().levels.element(2).load(Acquire), 3, "a run-time index");
        book.fields().levels.place::<3>().or(0x80, Release);
        book.fields().levels.element(0).update(AcqRel, Acquire, |level| level.wrapping_add(1));
        assert_eq!(
            book.load(Acquire),
            Book { levels: [2, 2, 3, 0x84], live: true },
            "each element alone changed, the flag kept"
        );
        assert!(book.fields().live.test_and_clear(AcqRel), "the flag beside them");
        let [first, ..] = book.fields().levels.elements();
        assert_eq!(first.load(Relaxed), 2, "every element's place");
    }

    #[test]
    fn a_padded_array_field_has_its_bitwise_operations() {
        repr_and_validity_are::<Table, u16, ZeroValid>();
        let table = Atomic::new(Table { taken: [false; 10], open: true });
        let mut seats = [false; 10];
        seats[3] = true;
        table.fields().taken.or(seats, Release);
        table.fields().taken.not(Release);
        let read = table.load(Acquire);
        assert_eq!(read.taken.iter().filter(|&&taken| taken).count(), 9, "every seat but one");
        assert!(!read.taken[3] && read.open, "the seat, and the flag above the ten bits, kept");
        table.fields().taken.and([false; 10], Release);
        assert_eq!(table.load(Acquire), Table { taken: [false; 10], open: true }, "cleared");
        for bits in 0..=u16::MAX {
            assert_holds(decodes_as_promised::<ZeroValid, Table>(bits));
        }
    }

    #[test]
    fn an_array_of_a_narrow_enum_takes_a_byte_per_element() {
        repr_and_validity_are::<Board, u32, ZeroValid>();
        assert_eq!(field_width(<[Side; 4] as Atom>::REPRS), 25, "three bytes and the top bit");
        let board = Atomic::new(Board { cells: [Side::Bid; 4], turn: Side::Ask });
        board.fields().cells.element(3).update(AcqRel, Acquire, |_| Side::Ask);
        let cells = board.load(Relaxed).cells;
        assert_eq!(cells, [Side::Bid, Side::Bid, Side::Bid, Side::Ask], "the last cell");
        for bits in 0..=u16::MAX {
            assert_holds(decodes_as_promised::<_, [Side; 2]>(bits));
        }
    }

    #[test]
    fn an_array_of_packed_structs_projects_each_one() {
        repr_and_validity_are::<[Slot; 4], u64, ZeroValid>();
        let slots = Atomic::new([Slot { written: false, lap: 0 }; 4]);
        assert!(!slots.place::<2>().fields().written.test_and_set(AcqRel), "slot 2's flag");
        slots.place::<2>().fields().lap.or(7, Release);
        slots
            .element(1)
            .update(AcqRel, Acquire, |slot| Slot { lap: slot.lap.wrapping_add(1), ..slot });
        let [zero, one, two, three] = slots.load(Acquire);
        assert_eq!((zero, three), (Slot { written: false, lap: 0 }, zero), "untouched");
        assert_eq!(one, Slot { written: false, lap: 1 }, "updated at a run-time index");
        assert_eq!(two, Slot { written: true, lap: 7 }, "through its fields");
    }

    #[test]
    fn an_elements_fields_are_places_at_a_run_time_index() {
        const EMPTY: Slot = Slot { written: false, lap: 0 };
        let slots = Atomic::new([EMPTY; 4]);
        for (index, lap) in (0..4).zip(10..) {
            assert!(!slots.element(index).fields().written.test_and_set(AcqRel), "slot {index}");
            slots.element(index).fields().lap.update(AcqRel, Acquire, |_| lap);
        }
        assert_eq!(slots.element(3).fields().lap.load(Acquire), 13, "a field read back");
        let laps = slots.load(Acquire).map(|slot| (slot.written, slot.lap));
        assert_eq!(laps, [(true, 10), (true, 11), (true, 12), (true, 13)], "each slot its own");
        let ring = Atomic::new(Ring { slots: [EMPTY; 3], head: 9 });
        ring.fields().slots.element(2).fields().written.set(Release);
        let read = ring.load(Acquire);
        assert!(read.slots[2].written && !read.slots[1].written, "a slot in an array field");
        assert_eq!(read.head, 9, "the field beside the array kept");
        let grid = Atomic::new([[EMPTY; 2]; 2]);
        grid.element(1).element(0).fields().lap.or(5, Relaxed);
        assert_eq!(grid.load(Acquire)[1], [Slot { written: false, lap: 5 }, EMPTY], "row 1");
        assert_eq!(format!("{:?}", grid.element(1).element(0).fields().lap), "5", "it prints");
    }

    #[test]
    fn a_signed_field_of_an_element_keeps_its_sign_at_a_run_time_index() {
        let zero = RangedI8::<-4, 3>::new(0).expect("zero lies in the range");
        let minus_one = RangedI8::<-4, 3>::new(-1).expect("so does -1");
        let ticks = Atomic::new([Tick { traded: false, moved: zero }; 4]);
        let constant = Atomic::new([Tick { traded: false, moved: zero }; 4]);
        ticks.element(2).fields().moved.update(AcqRel, Acquire, |_| minus_one);
        constant.place::<2>().fields().moved.update(AcqRel, Acquire, |_| minus_one);
        let (run_time, built) = (ticks.load(Acquire).to_repr(), constant.load(Acquire).to_repr());
        assert_eq!(run_time, built, "the bits a constant index writes, its sign extended");
        assert_eq!(ticks.element(2).fields().moved.load(Acquire).get(), -1, "read back");
        ticks.element(2).fields().traded.set(Relaxed);
        let tick = ticks.load(Acquire)[2];
        assert_eq!(tick, Tick { traded: true, moved: minus_one }, "the flag beside it");
    }
}

#[cfg(test)]
mod pointer_words {
    use core::ptr::{self, NonNull};

    use atomix::ordering::{AcqRel, Acquire, Relaxed, Release};
    use atomix::{Atom, Atomic};

    /// A node aligned to eight bytes, whose pointer leaves three low bits clear.
    #[repr(align(8))]
    struct Node;

    /// A pointer word whose tags are an array of two marks.
    #[derive(Clone, Copy, Atom)]
    struct Head {
        /// The first node, or none.
        top: Option<NonNull<Node>>,
        /// Two marks, in the pointer's low bits.
        marks: [bool; 2],
    }

    #[test]
    fn an_array_of_tags_shares_the_pointer_word() {
        assert_eq!(<Head as Atom>::TAG_WIDTH, 2, "two tag bits");
        let node = Box::leak(Box::new(Node));
        let head = Atomic::new(Head { top: NonNull::new(node), marks: [false; 2] });
        assert!(!head.fields().marks.element(1).test_and_set(AcqRel), "mark 1, at run time");
        assert!(head.fields().marks.place::<1>().test_and_clear(AcqRel), "and at a constant");
        head.fields().marks.element(0).update(AcqRel, Acquire, |_| true);
        let loaded = head.load(Acquire);
        assert_eq!(
            (loaded.top, loaded.marks),
            (NonNull::new(node), [true, false]),
            "the pointer kept"
        );
    }

    /// The configuration in force.
    #[derive(Debug)]
    struct Config {
        /// How many price levels a quote shows.
        depth: u32,
    }

    /// The configuration in force, and two marks: a pointer word over a reference, which loads it
    /// no weaker than `Acquire`, though its marks take every ordering where they hand out no more
    /// than themselves.
    #[derive(Clone, Copy, Atom)]
    struct Marked {
        /// The configuration.
        config: &'static Config,
        /// Two marks, in the reference's low bits.
        marks: [bool; 2],
    }

    /// A configuration a word points to.
    static CONFIG: Config = Config { depth: 10 };

    #[test]
    fn an_array_of_tags_beside_a_reference_keeps_its_floor() {
        let marked = Atomic::new(Marked { config: &CONFIG, marks: [false; 2] });
        marked.fields().marks.element(1).set(Relaxed);
        marked.fields().marks.place::<0>().store(true, Release);
        assert!(marked.fields().marks.element(0).load(Relaxed), "a mark read alone");
        let read = marked.load(Acquire);
        assert!(ptr::eq(read.config, &raw const CONFIG), "the reference, its tags cleared");
        assert_eq!((read.config.depth, read.marks), (10, [true, true]), "beside both marks");
        // An element's update returns the word, so its read acquires the reference; its write puts
        // the reference back, and needs no `Release`.
        let before = marked.fields().marks.element(1).update(Acquire, Acquire, |mark| !mark);
        assert_eq!((before.config.depth, before.marks), (10, [true, true]), "the word before");
        marked.fields().marks.last().clear(Relaxed);
        assert_eq!(marked.load(Acquire).marks, [true, false], "the last mark cleared");
    }
}

#[cfg(test)]
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
mod double_words {
    use core::ptr::NonNull;

    use atomix::ordering::{AcqRel, Acquire};
    use atomix::{Atom, Atomic};

    /// A node aligned to eight bytes.
    #[repr(align(8))]
    struct Node;

    /// A pointer beside four counters, in an integer word of their own.
    #[derive(Clone, Copy, Atom)]
    #[atom(repr = u128)]
    struct Versioned {
        /// The node.
        node: NonNull<Node>,
        /// Four counters.
        counts: [u16; 4],
    }

    /// An order resting on the book.
    #[derive(Debug)]
    struct Order {
        /// Its price.
        price: u64,
    }

    /// The best bid and ask: two references in a double word, as a newtype over an array of them.
    #[derive(Clone, Copy, Atom)]
    struct Best([&'static Order; 2]);

    #[test]
    fn an_array_of_tags_fills_the_integer_word() {
        let node = Box::leak(Box::new(Node));
        let versioned = Atomic::new(Versioned { node: NonNull::from(&*node), counts: [0; 4] });
        let before = versioned
            .fields()
            .counts
            .element(3)
            .update(AcqRel, Acquire, |count| count.wrapping_add(1));
        assert_eq!(before.counts, [0; 4], "the counts before");
        let after = versioned.load_rmw(Acquire);
        assert_eq!(
            (after.node, after.counts),
            (NonNull::from(&*node), [0, 0, 0, 1]),
            "the pointer kept"
        );
    }

    #[test]
    fn a_newtype_over_two_references_takes_a_double_word() {
        static BID: Order = Order { price: 100 };
        static ASK: Order = Order { price: 101 };
        let best = Atomic::new(Best([&BID, &ASK]));
        let Best([bid, ask]) = best.load_rmw(Acquire);
        assert_eq!((bid.price, ask.price), (100, 101), "both, read through");
    }
}
