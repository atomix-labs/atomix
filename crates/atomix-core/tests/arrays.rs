//! An array as an atomic value: each element in a slot as wide as its repr, element 0 lowest, two
//! pointers in a double word; its validity, its range and `Option`'s niche; its elements as places,
//! at an index known at run time, whose place holds where its bits lie, or a constant, a row of an
//! array of arrays lending each place a whole array lends; and `Packed`, an array at its elements'
//! width in the integer it states, and the traits it shares with its array.

#![feature(const_trait_impl)]
#![cfg(not(loom))]

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests {
    use core::num::NonZero;
    use core::ptr::NonNull;

    use atomix_core::ordering::{AcqRel, Acquire, Relaxed, Release, SeqCst};
    use atomix_core::validity::{Partial, Total, TotalZeroNiche, ZeroNiche, ZeroValid};
    use atomix_core::{
        Atom, AtomBitwise, Atomic, AtomicField, FieldBitwise, Indexed, RangedI8, RangedU8,
        ReprRange,
    };

    use crate::testing::atom::repr_and_validity_are;
    use crate::testing::law::{assert_holds, decodes_as_promised, round_trips};

    #[test]
    fn each_array_takes_the_repr_and_validity_its_slots_give() {
        repr_and_validity_are::<[u8; 1], u8, Total>();
        repr_and_validity_are::<[u8; 2], u16, Total>();
        repr_and_validity_are::<[u8; 3], u32, ZeroValid>();
        repr_and_validity_are::<[u8; 4], u32, Total>();
        repr_and_validity_are::<[u8; 8], u64, Total>();
        repr_and_validity_are::<[i16; 4], u64, Total>();
        repr_and_validity_are::<[bool; 1], bool, Total>();
        repr_and_validity_are::<[bool; 8], u8, Total>();
        repr_and_validity_are::<[bool; 3], u8, ZeroValid>();
        repr_and_validity_are::<[bool; 64], u64, Total>();
        repr_and_validity_are::<[f32; 2], u64, Total>();
        repr_and_validity_are::<[NonZero<u8>; 1], u8, TotalZeroNiche>();
        repr_and_validity_are::<[NonZero<u8>; 4], u32, ZeroNiche>();
        repr_and_validity_are::<[char; 2], u64, ZeroValid>();
        repr_and_validity_are::<[RangedU8<0, 3>; 4], u32, Partial>();
        repr_and_validity_are::<[[u8; 2]; 2], u32, Total>();
        repr_and_validity_are::<[Option<NonZero<u16>>; 4], u64, Total>();
        repr_and_validity_are::<[NonNull<u64>; 1], *mut u64, TotalZeroNiche>();
    }

    #[cfg(wide)]
    #[test]
    fn a_wide_array_takes_128_bits_or_two_words() {
        use atomix_core::DoubleWord;

        repr_and_validity_are::<[u8; 16], u128, Total>();
        repr_and_validity_are::<[bool; 128], u128, Total>();
        repr_and_validity_are::<[u32; 3], u128, ZeroValid>();
        repr_and_validity_are::<[NonNull<u64>; 2], DoubleWord<*mut u64, *mut u64>, Partial>();
        repr_and_validity_are::<[Option<NonNull<u64>>; 2], DoubleWord<*mut u64, *mut u64>, ZeroValid>(
        );
    }

    #[test]
    fn an_array_of_bytes_is_its_own_bytes() {
        let bytes = [1_u8, 2, 3, 4];
        assert_eq!(bytes.to_repr(), u32::from_le_bytes(bytes), "element 0 lowest, as the bytes");
        let flags = [true, false, true, true];
        assert_eq!(flags.to_repr(), 0b1101, "a bool takes one bit");
        let signs: [i8; 2] = [-1, 1];
        assert_eq!(signs.to_repr(), 0x01FF, "a signed element keeps its own bits");
    }

    #[test]
    fn every_repr_of_two_bytes_obeys_the_laws() {
        for bits in 0..=u16::MAX {
            assert_holds(decodes_as_promised::<Total, [u8; 2]>(bits));
            assert_holds(decodes_as_promised::<Total, [i8; 2]>(bits));
            assert_holds(decodes_as_promised::<Total, [bool; 16]>(bits));
            assert_holds(decodes_as_promised::<ZeroNiche, [NonZero<u8>; 2]>(bits));
            assert_holds(decodes_as_promised::<Partial, [RangedU8<0, 3>; 2]>(bits));
            assert_holds(decodes_as_promised::<_, [RangedI8<-1, 1>; 2]>(bits));
            assert_holds(decodes_as_promised::<ZeroValid, [bool; 9]>(bits));
            let [low, _] = bits.to_le_bytes();
            assert_holds(decodes_as_promised::<ZeroValid, [bool; 3]>(low));
            assert_holds(decodes_as_promised::<Total, [u8; 1]>(low));
        }
    }

    #[test]
    fn each_range_places_the_slots_bounds_side_by_side() {
        for first in -1..=1 {
            for second in -1..=1 {
                let value =
                    [RangedI8::<-1, 1>::new(first), RangedI8::new(second)].map(Option::unwrap);
                assert_holds(round_trips(value));
                let [low, high] = value;
                assert_holds(round_trips([low, high, low]));
            }
        }
        assert_eq!(
            <[RangedI8<-1, 1>; 2] as Atom>::REPRS,
            ReprRange::from_signed(-0x100, 0x01FF),
            "the top slot's range, wrapping through zero, the slot below free"
        );
        assert_eq!(
            <[RangedU8<1, 3>; 2] as Atom>::REPRS,
            ReprRange::new(0x0101, 0x0303),
            "each slot's bounds side by side"
        );
        assert_eq!(
            <[RangedI8<-1, 1>; 3] as Atom>::REPRS,
            ReprRange::new(0, 0xFF_FFFF),
            "with clear bits above the slots, every repr below them"
        );
        assert_eq!(<[bool; 10] as Atom>::REPRS, ReprRange::new(0, 0x3FF), "ten bits");
    }

    #[test]
    fn option_takes_a_spare_repr() {
        assert_eq!(<Option<[NonZero<u8>; 4]> as Atom>::to_repr(None), 0, "zero: no element is");
        assert_eq!(<Option<[u8; 3]> as Atom>::to_repr(None), 1 << 24, "above the slots");
        assert_eq!(<Option<[bool; 7]> as Atom>::to_repr(None), 1 << 7, "above the seven bits");
        let quotes = Atomic::<Option<[NonZero<u16>; 4]>>::new(None);
        assert_eq!(size_of_val(&quotes), 8, "four prices and their absence in a word");
    }

    #[test]
    fn an_atomic_array_loads_stores_and_exchanges_whole() {
        let levels = Atomic::new([1_u8, 2, 3, 4]);
        levels.store([5, 6, 7, 8], Release);
        assert_eq!(levels.load(Acquire), [5, 6, 7, 8], "the array stored");
        assert_eq!(levels.swap([9; 4], AcqRel), [5, 6, 7, 8], "swapped");
        assert_eq!(
            levels.compare_exchange([9; 4], [0; 4], AcqRel, Acquire),
            Ok([9; 4]),
            "compared and exchanged whole"
        );
        levels.or([1, 2, 4, 8], Relaxed);
        levels.xor([1, 0, 0, 0], Relaxed);
        assert_eq!(levels.load(Relaxed), [0, 2, 4, 8], "each element with its own");
    }

    // A full array of bitwise elements is bitwise, and a padded one in a field.
    const _: () = {
        /// Compiles only where `T` has `AtomBitwise`.
        const fn bitwise<T: AtomBitwise>() {}
        /// Compiles only where `T` has `FieldBitwise`.
        const fn field_bitwise<T: FieldBitwise>() {}
        bitwise::<[u8; 4]>();
        bitwise::<[bool; 8]>();
        bitwise::<[[u8; 2]; 2]>();
        field_bitwise::<[u8; 4]>();
        field_bitwise::<[u8; 3]>();
        field_bitwise::<[bool; 10]>();
        field_bitwise::<[i16; 3]>();
    };

    #[test]
    fn each_element_is_a_place() {
        let levels = Atomic::new([1_u8, 2, 3, 4]);
        assert_eq!(levels.element(2).load(Acquire), 3, "element 2");
        let before = levels.element(1).update(AcqRel, Acquire, |level| level.wrapping_add(10));
        assert_eq!(before, [1, 2, 3, 4], "the array before");
        assert_eq!(levels.load(Acquire), [1, 12, 3, 4], "element 1 alone changed");
        let declined = levels.element(0).try_update(AcqRel, Acquire, |_| None);
        assert_eq!(declined, Err([1, 12, 3, 4]), "declined, nothing written");
        let [first, .., last] = levels.elements();
        assert_eq!((first.load(Relaxed), last.load(Relaxed)), (1, 4), "destructured");
        assert!(levels.element_checked(4).is_none(), "past the end");
        assert_eq!(levels.element_checked(3).map(|level| level.load(Relaxed)), Some(4), "within");
        levels.place::<3>().or(0x80, Release);
        assert_eq!(levels.load(Acquire), [1, 12, 3, 0x84], "a constant index's place");
        levels.element(2).and(0b10, Release);
        levels.element(0).xor(0xFF, Release);
        levels.element(1).not(Release);
        assert_eq!(levels.load(Acquire), [0xFE, 0xF3, 2, 0x84], "each its own bits");
        // SAFETY: 1 lies below the array's length, 4.
        #[expect(unsafe_code, reason = "the unchecked index")]
        let second = unsafe { levels.element_unchecked(1) };
        assert_eq!(format!("{second:?}"), "243", "an element prints its value");
    }

    #[test]
    fn a_bool_element_is_a_bit() {
        let busy = Atomic::new([false; 64]);
        for worker in [0, 5, 63] {
            assert!(!busy.element(worker).test_and_set(AcqRel), "worker {worker} was idle");
            assert!(busy.element(worker).test_and_set(AcqRel), "and now is busy");
        }
        assert!(!busy.bit_set(7, AcqRel), "an array's bit, as a word's");
        assert!(busy.load(Relaxed)[7], "set");
        busy.element(5).clear(Release);
        assert!(!busy.place::<5>().test_and_clear(AcqRel), "cleared");
        assert!(busy.place::<63>().test_and_clear(AcqRel), "the top bit");
        busy.element(9).toggle(Release);
        assert!(busy.element(9).test_and_toggle(AcqRel), "toggled on, then off");
        busy.element(10).store(true, Release);
        busy.element(0).store(false, SeqCst);
        let bits = busy.load(Acquire);
        let set: Vec<usize> = (0..64).filter(|&bit| bits[bit]).collect();
        assert_eq!(set, [7, 10], "the bits each operation left");
        assert!(
            busy.element_checked(10).is_some_and(|bit| bit.test_and_clear(AcqRel)),
            "a checked bit's test, as `is_some_and` keeps it one instruction"
        );
    }

    #[test]
    fn a_signed_element_keeps_its_sign_in_its_slot() {
        let zero = RangedI8::<-1, 1>::new(0).expect("zero lies in the range");
        let minus_one = RangedI8::<-1, 1>::new(-1).expect("so does -1");
        let signs = Atomic::new([zero; 4]);
        signs.element(3).update(AcqRel, Acquire, |_| minus_one);
        assert_eq!(signs.load(Acquire).map(RangedI8::get), [0, 0, 0, -1], "the top slot");
        assert_eq!(signs.element(3).load(Acquire).get(), -1, "read back");
        signs.element(0).update(AcqRel, Acquire, |_| minus_one);
        assert_eq!(signs.load(Acquire).map(RangedI8::get), [-1, 0, 0, -1], "the bottom slot");
        assert_eq!(signs.load(Relaxed).to_repr(), 0xFF00_00FF, "each slot its i8's bits");
        let padded = Atomic::new([zero; 3]);
        padded.element(2).update(AcqRel, Acquire, |_| minus_one);
        assert_eq!(padded.load(Relaxed).to_repr(), 0x00FF_0000, "the bits above the slots clear");
    }

    #[test]
    #[should_panic(expected = "index out of bounds: the len is 4 but the index is 4")]
    fn an_index_past_the_end_panics() {
        let levels = Atomic::new([0_u8; 4]);
        assert_eq!(levels.element(4).load(Relaxed), 0, "never read");
    }

    #[test]
    fn an_element_of_no_bits_lies_at_its_arrays_offset() {
        let units = Atomic::new([(); 4]);
        assert_eq!(units.element(3).update(AcqRel, Acquire, |unit| unit), [(); 4], "a unit");
        assert_eq!(units.element(3).load(Relaxed), (), "read back from no bits");
        assert_eq!(units.place::<3>().load(Relaxed), (), "alike at a constant index");
    }

    #[test]
    fn nested_arrays_project_twice() {
        let grid = Atomic::new([[1_u8, 2], [3, 4]]);
        assert_eq!(grid.place::<1>().place::<0>().load(Acquire), 3, "row 1, column 0");
        assert_eq!(grid.element(1).element(0).load(Acquire), 3, "alike at run time");
        grid.place::<0>().element(1).update(AcqRel, Acquire, |cell| cell.wrapping_mul(10));
        assert_eq!(grid.load(Acquire), [[1, 20], [3, 4]], "row 0, column 1");
        let [first_row, _] = grid.elements();
        assert_eq!(first_row.element_checked(2).map(|cell| cell.load(Relaxed)), None, "past");
    }

    #[test]
    fn a_row_lends_each_place_a_whole_array_lends() {
        let grid = Atomic::new([[1_u8, 2, 3], [4, 5, 6]]);
        let row = grid.element(1);
        assert_eq!(row.place::<0>().load(Acquire), 4, "a constant column of a row at run time");
        assert_eq!(row.last().load(Acquire), 6, "its last");
        // SAFETY: column 1 lies within a row of three.
        #[expect(unsafe_code, reason = "a column the test knows lies within the row")]
        let middle = unsafe { row.element_unchecked(1) };
        middle.or(0x10, Relaxed);
        let [first, second, third] = row.elements();
        let columns = [first, second, third].map(|column| column.load(Acquire));
        assert_eq!(columns, [4, 0x15, 6], "each column of the row");
        grid.place::<0>().last().and(0b10, Relaxed);
        assert_eq!(grid.load(Acquire), [[1, 2, 2], [4, 0x15, 6]], "a row field's last column");
    }

    #[test]
    fn a_place_at_a_run_time_index_holds_where_its_bits_lie() {
        let grid = Atomic::new([[false; 8]; 8]);
        let cell: &AtomicField<Indexed<[[bool; 8]; 8], bool>> = grid.element(5).element(3);
        assert!(!cell.test_and_set(AcqRel), "row 5, column 3");
        assert_eq!(
            grid.load(Acquire)[5],
            [false, false, false, true, false, false, false, false],
            "bit 43 alone"
        );
        assert!(
            grid.element(5).place::<3>().test_and_clear(AcqRel),
            "the same bit, at a constant column"
        );
        assert!(!grid.element(5).last().test_and_set(AcqRel), "the row's last column");
        assert!(grid.load(Acquire)[5][7], "bit 47");
        let words = size_of::<&AtomicField<Indexed<[[bool; 8]; 8], bool>>>();
        assert_eq!(words, 2 * size_of::<usize>(), "the atomic's address, and where the bit lies");
        assert_eq!(size_of_val(cell), size_of::<u64>(), "the place spans the atomic alone");
        assert_eq!(format!("{cell:?}"), "false", "it prints its bit");
    }

    // A place at a run-time index, and a row's, in a constant.
    const _: () = {
        let grid = Atomic::new([[0_u8; 2]; 2]);
        grid.element(1).element(0).or(0b101, Relaxed);
        assert!(grid.element(1).place::<0>().load(Relaxed) == 0b101, "row 1, column 0");
        assert!(grid.element(1).last().load(Relaxed) == 0, "its last column untouched");
        let [row, _] = grid.elements();
        row.element(1).xor(1, Relaxed);
        assert!(grid.element(0).element(1).load(Acquire) == 1, "row 0, column 1, in a constant");
    };

    #[test]
    fn the_last_element_adds_in_place_where_the_slots_fill_the_repr() {
        let phase = Atomic::new([3_u16, u16::MAX]);
        assert_eq!(phase.last().fetch_add(1, AcqRel), [3, u16::MAX], "the array before");
        assert_eq!(phase.load(Acquire), [3, 0], "the carry left the word");
        assert_eq!(phase.last().fetch_sub(2, AcqRel), [3, 0], "and the borrow");
        assert_eq!(phase.load(Acquire), [3, u16::MAX - 1], "the element below unchanged");
        let one = Atomic::new([7_u32]);
        assert_eq!(one.last().fetch_add(1, Relaxed), [7], "one element, the whole word");
    }

    /// An order resting on the book.
    #[derive(Debug, PartialEq)]
    struct Order {
        /// Its price.
        price: u64,
    }

    #[cfg(wide)]
    #[test]
    fn two_pointers_take_a_double_word() {
        static BID: Order = Order { price: 100 };
        static ASK: Order = Order { price: 101 };
        static NEXT: Order = Order { price: 99 };
        let best = Atomic::new([&BID, &ASK]);
        let [bid, ask] = best.load_rmw(Acquire);
        assert_eq!((bid.price, ask.price), (100, 101), "both, read through");
        let seen = best.update(AcqRel, Acquire, |[_, ask]| [&NEXT, ask]);
        assert_eq!(seen[0].price, 100, "the pair before");
        assert_eq!(best.load_rmw(Acquire)[0].price, 99, "the bid replaced");

        let mut one = 1_u64;
        let mut two = 2_u64;
        let pointers = Atomic::new([NonNull::from(&mut one), NonNull::from(&mut two)]);
        let [first, second] = pointers.load_rmw(Acquire);
        // SAFETY: it points to `one`, which lives, read through the pointer stored.
        #[expect(unsafe_code, reason = "reads through the pointer stored")]
        let first = unsafe { first.read() };
        // SAFETY: and it to `two`.
        #[expect(unsafe_code, reason = "reads through the pointer stored")]
        let second = unsafe { second.read() };
        assert_eq!((first, second), (1, 2), "each pointer keeps its provenance");
    }

    #[test]
    fn one_pointer_is_its_own_repr() {
        static BID: Order = Order { price: 100 };
        let best = Atomic::new([&BID]);
        assert_eq!(size_of_val(&best), size_of::<usize>(), "one word");
        assert_eq!(best.load(Acquire)[0].price, 100, "read through");
        let none = Atomic::new(None::<[&Order; 1]>);
        assert_eq!(none.load(Acquire), None, "null is its `None`");
    }
}

#[cfg(test)]
mod packed {
    use core::cmp::Ordering;
    use core::hash::BuildHasher;
    use core::num::NonZero;
    use std::collections::hash_map::RandomState;

    use atomix_core::ordering::{AcqRel, Acquire, Relaxed, Release};
    use atomix_core::validity::{Partial, ZeroNiche, ZeroValid};
    use atomix_core::{Atom, Atomic, Packed, RangedI8, RangedU8};

    use crate::testing::atom::repr_and_validity_are;
    use crate::testing::law::{assert_holds, decodes_as_promised};

    /// How full a queue is, in quarters.
    type Quarter = RangedU8<0, 3>;

    /// `count` quarters.
    fn quarter(count: u8) -> Quarter {
        Quarter::new(count).expect("each count passed lies in 0 to 3")
    }

    #[test]
    fn each_element_takes_the_bits_its_range_needs() {
        repr_and_validity_are::<Packed<[Quarter; 16], u32>, u32, Partial>();
        repr_and_validity_are::<Packed<[bool; 3], u8>, u8, ZeroValid>();
        repr_and_validity_are::<Packed<[NonZero<u8>; 2], u16>, u16, ZeroNiche>();
        let packed = Packed::<_, u8>::new([0, 1, 2, 3].map(quarter));
        assert_eq!(packed.to_repr(), 0b11_10_01_00, "two bits each, element 0 lowest");
        assert_eq!(<Option<Packed<[bool; 3], u8>> as Atom>::to_repr(None), 0b1000, "above them");
    }

    #[test]
    fn every_repr_of_a_byte_obeys_the_laws() {
        for bits in 0..=u8::MAX {
            assert_holds(decodes_as_promised::<_, Packed<[RangedI8<-1, 1>; 3], u8>>(bits));
            assert_holds(decodes_as_promised::<_, Packed<[RangedU8<0, 2>; 4], u8>>(bits));
            assert_holds(decodes_as_promised::<_, Packed<[bool; 8], u8>>(bits));
            assert_holds(decodes_as_promised::<_, Packed<[bool; 5], u8>>(bits));
            assert_holds(decodes_as_promised::<_, Packed<[NonZero<u8>; 1], u8>>(bits));
            assert_holds(decodes_as_promised::<_, Option<Packed<[bool; 5], u8>>>(bits));
        }
    }

    #[test]
    fn each_element_is_a_place_in_its_bits() {
        let fill = Atomic::new(Packed::<_, u32>::new([quarter(0); 16]));
        fill.element(15).update(AcqRel, Acquire, |_| quarter(3));
        fill.place::<7>().update(AcqRel, Acquire, |_| quarter(2));
        assert_eq!(fill.element(15).load(Acquire), quarter(3), "the last element");
        assert_eq!(fill.load(Relaxed).to_repr(), 0b11 << 30 | 0b10 << 14, "in its two bits");
        let [first, ..] = fill.elements();
        assert_eq!(first.load(Relaxed), quarter(0), "every element's place");
        let flags = Atomic::new(Packed::<_, u8>::new([false; 5]));
        flags.element(4).set(Release);
        assert_eq!(flags.load(Relaxed).to_repr(), 0b1_0000, "the fifth");
    }

    #[test]
    fn the_last_elements_sign_fills_the_bits_above() {
        let zero = RangedI8::<-1, 1>::new(0).expect("zero lies in the range");
        let signs = Atomic::new(Packed::<_, u8>::new([zero; 3]));
        signs.element(2).update(AcqRel, Acquire, |_| RangedI8::new(-1).expect("so does -1"));
        assert_eq!(signs.load(Relaxed).to_repr(), 0b1111_0000, "its sign above it");
        signs.element(2).update(AcqRel, Acquire, |_| RangedI8::new(1).expect("and 1"));
        assert_eq!(signs.load(Relaxed).to_repr(), 0b0001_0000, "and cleared again");
        signs.element(0).update(AcqRel, Acquire, |_| RangedI8::new(-1).expect("and -1"));
        assert_eq!(signs.load(Relaxed).to_repr(), 0b0001_0011, "below it, its own two bits");
    }

    #[test]
    fn a_packed_array_dereferences_to_its_array_and_gives_it_back() {
        let mut packed = Packed::<[u8; 3], u32>::from([3, 1, 2]);
        assert_eq!(packed[0], 3, "indexed");
        packed[0] = 0;
        assert_eq!(packed.iter().sum::<u8>(), 3, "written and iterated");
        assert_eq!(Packed::into_inner(packed), [0, 1, 2], "and given back");
    }

    #[test]
    fn a_packed_array_compares_and_orders_as_its_array() {
        let packed = Packed::<[u8; 3], u32>::new([0, 1, 2]);
        let other = Packed::new([0, 1, 3]);
        assert_eq!(packed.cmp(&other), [0_u8, 1, 2].cmp(&[0, 1, 3]), "by its elements in turn");
        assert_eq!(packed.partial_cmp(&other), Some(Ordering::Less), "partially too");
        assert_ne!(packed, other, "unequal where an element is");
        assert_eq!(packed, packed.clone(), "and equal to its clone");
    }

    #[test]
    fn a_packed_array_hashes_prints_and_defaults_as_its_array() {
        let packed = Packed::<[u8; 3], u32>::new([0, 1, 2]);
        let hasher = RandomState::new();
        assert_eq!(hasher.hash_one(packed), hasher.hash_one([0_u8, 1, 2]), "its hash");
        assert_eq!(format!("{packed:?}"), "[0, 1, 2]", "its print");
        assert_eq!(Packed::<[u8; 2], u16>::default(), Packed::new([0, 0]), "its default");
    }

    // A packed array crosses threads as its array does.
    const _: () = {
        /// Compiles only where `T` may cross threads and be shared between them.
        const fn crosses_threads<T: Send + Sync>() {}
        crosses_threads::<Packed<[u8; 2], *mut u8>>();
        crosses_threads::<Packed<[bool; 3], u8>>();
    };
}
