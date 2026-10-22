//! A tuple as an atomic value, each field in a slot as wide as its repr, the struct of its fields'
//! layout where each fills its repr: fields stored as bits side by side in slots of their reprs'
//! widths; a pointer in a word of its own, the others in its low bits where they take 29 bits or
//! fewer, else in an integer word beside it, as they take beside a function pointer whatever their
//! bits; two pointers in two words; its validity, its range and `Option`'s niche; a reference's
//! floor, which a tuple that holds one keeps; its fields as places, an array element's at a
//! run-time index too; and the table its shapes come from, as its generator writes it.

#![feature(const_trait_impl)]
#![cfg(not(loom))]

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests {
    use core::num::NonZero;
    use core::ops::RangeInclusive;
    use core::ptr::{self, NonNull};
    use std::path::Path;
    use std::{env, fs};

    use atomix_core::ordering::{AcqRel, Acquire, Relaxed, Release};
    use atomix_core::validity::{Partial, Total, TotalZeroNiche, ZeroNiche, ZeroValid};
    use atomix_core::{
        Atom, AtomAdd, AtomBitwise, Atomic, FieldBitwise, RangedI8, ReprOf, ReprRange,
    };

    use crate::testing::atom::repr_and_validity_are;
    use crate::testing::law::{assert_holds, decodes_as_promised, round_trips};

    /// A node aligned to eight bytes, whose pointer leaves three low bits clear.
    #[derive(Debug)]
    #[repr(align(8))]
    struct Node {
        /// What it holds.
        value: u64,
    }

    /// Nodes that live as long as the tests.
    static NODES: [Node; 2] = [Node { value: 1 }, Node { value: 2 }];

    /// A pointer to node `index`, with the static's provenance.
    fn node(index: usize) -> NonNull<Node> {
        NonNull::from(&NODES[index])
    }

    #[test]
    fn fields_stored_as_bits_sit_side_by_side_in_slots() {
        type Twelve = (bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool);
        repr_and_validity_are::<(u8,), u8, Total>();
        repr_and_validity_are::<(u8, u8), u16, Total>();
        repr_and_validity_are::<(u32, u32), u64, Total>();
        repr_and_validity_are::<(u32, u16), u64, ZeroValid>();
        repr_and_validity_are::<(bool, bool), u8, ZeroValid>();
        repr_and_validity_are::<(bool, u8, u16), u32, ZeroValid>();
        repr_and_validity_are::<(NonZero<u8>, u8), u16, ZeroNiche>();
        repr_and_validity_are::<(NonZero<u8>,), u8, TotalZeroNiche>();
        repr_and_validity_are::<(char, bool), u64, ZeroValid>();
        repr_and_validity_are::<(RangedI8<-1, 1>, u8), u16, Partial>();
        repr_and_validity_are::<Twelve, u16, ZeroValid>();
        assert_eq!((1_u8, 2_u16, true).to_repr(), 0x0100_0201, "field 0 lowest, each in its slot");
    }

    #[cfg(wide)]
    #[test]
    fn a_wide_tuple_takes_128_bits() {
        type TwelveBytes = (u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8);
        repr_and_validity_are::<(u64, u64), u128, Total>();
        repr_and_validity_are::<TwelveBytes, u128, ZeroValid>();
    }

    #[test]
    fn every_repr_of_two_bytes_obeys_the_laws() {
        for bits in 0..=u16::MAX {
            assert_holds(decodes_as_promised::<Total, (u8, u8)>(bits));
            assert_holds(decodes_as_promised::<Total, (i8, u8)>(bits));
            assert_holds(decodes_as_promised::<ZeroNiche, (bool, NonZero<u8>)>(bits));
            assert_holds(decodes_as_promised::<_, (RangedI8<-1, 1>, u8)>(bits));
            assert_holds(decodes_as_promised::<_, (u8, RangedI8<-1, 1>)>(bits));
            assert_holds(decodes_as_promised::<ZeroValid, (bool, u8)>(bits));
            let [low, _] = bits.to_le_bytes();
            assert_holds(decodes_as_promised::<ZeroValid, (bool, bool, bool)>(low));
            assert_holds(decodes_as_promised::<Total, (u8,)>(low));
        }
    }

    #[test]
    fn option_takes_a_spare_repr() {
        assert_eq!(<Option<(u8, u8, u8)> as Atom>::to_repr(None), 1 << 24, "above the fields");
        assert_eq!(
            <Option<(NonZero<u8>, u8)> as Atom>::to_repr(None),
            0,
            "zero: no first field is"
        );
        assert_eq!(
            <Option<(NonNull<Node>, bool)> as Atom>::to_repr(None),
            ptr::null_mut(),
            "null: the pointer is never"
        );
    }

    #[test]
    fn a_field_loads_alone() {
        let quote = Atomic::new((7_u32, false, 3_u8));
        let (price, _, size) = quote.fields();
        assert_eq!((price.load(Acquire), size.load(Acquire)), (7, 3), "fields 0 and 2");
    }

    #[test]
    fn a_bool_field_tests_and_sets_its_bit() {
        let quote = Atomic::new((7_u32, false, 3_u8));
        assert!(!quote.fields().1.test_and_set(AcqRel), "clear before");
        assert_eq!(quote.load(Relaxed), (7, true, 3), "set, the others kept");
    }

    #[test]
    fn a_fields_or_changes_it_alone() {
        let quote = Atomic::new((7_u32, false, 3_u8));
        quote.fields().2.or(0x80, Release);
        assert_eq!(quote.load(Relaxed), (7, false, 0x83), "the last field alone changed");
    }

    #[test]
    fn an_elements_fields_are_places_at_a_run_time_index() {
        let slots = Atomic::new([(false, 0_u8); 4]);
        for (index, length) in (0..4).zip(10..) {
            assert!(!slots.element(index).fields().0.test_and_set(AcqRel), "slot {index}, once");
            slots.element(index).fields().1.update(AcqRel, Acquire, |_| length);
        }
        assert_eq!(slots.load(Acquire), [(true, 10), (true, 11), (true, 12), (true, 13)], "each");
        let book = Atomic::new(([(false, 0_u8); 2], 0x55_u16));
        book.fields().0.element(1).fields().1.or(0x81, Release);
        assert_eq!(book.load(Acquire), ([(false, 0), (false, 0x81)], 0x55), "a tuple in an array");
        let grid = Atomic::new([[(false, 7_u8); 2]; 2]);
        assert!(!grid.element(1).element(0).fields().0.test_and_set(Relaxed), "one cell");
        assert_eq!(grid.element(1).element(0).fields().1.load(Acquire), 7, "its length kept");
        assert_eq!(grid.load(Acquire)[1], [(true, 7), (false, 7)], "that cell alone");
    }

    #[test]
    fn the_last_field_at_the_top_adds_in_place() {
        let counters = Atomic::new((1_u8, 2_u8, u16::MAX));
        assert_eq!(counters.fields().2.fetch_add(1, AcqRel), (1, 2, u16::MAX), "the tuple before");
        assert_eq!(counters.load(Relaxed), (1, 2, 0), "its carry leaves the word");
    }

    #[test]
    fn a_signed_field_keeps_its_sign_in_its_slot() {
        let [minus_one, zero, one] =
            [-1, 0, 1].map(|sign| RangedI8::<-1, 1>::new(sign).expect("each lies in -1 to 1"));
        let signs = Atomic::new((zero, 9_u8));
        signs.fields().0.update(AcqRel, Acquire, |_| minus_one);
        assert_eq!(signs.load(Acquire).0.get(), -1, "set");
        assert_eq!(signs.load(Acquire).to_repr(), 0x09FF, "its slot all ones, the next field kept");
        signs.fields().0.update(AcqRel, Acquire, |_| one);
        assert_eq!(signs.load(Acquire).to_repr(), 0x0901, "and cleared with it");
        for (first, second) in [(minus_one, 0_u8), (one, 0xFF), (zero, 7)] {
            assert_holds(round_trips((first, second)));
        }
    }

    /// A count to 127 in a byte, every repr of which decodes, its top bit dropped: `Total`, its
    /// range narrower than its slot.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Seven(u8);

    /// The bits of a `Seven`'s count, its range's end.
    const SEVEN: u8 = 0x7F;

    // SAFETY: `to_repr` gives 0 to 127, within `REPRS`; `from_repr` decodes every repr, by the repr
    // alone, as `Total` promises, and the default unchecked decode unwraps it; a `Seven` holds a
    // byte, so it may cross threads.
    #[expect(unsafe_code, reason = "a `Total` value whose range leaves its slot's top bit clear")]
    const unsafe impl Atom for Seven {
        type Repr = u8;
        type Validity = Total;
        const REPRS: ReprRange<u8> = ReprRange::new(0, 0x7F);
        fn to_repr(self) -> u8 {
            self.0 & SEVEN
        }
        fn from_repr(repr: u8) -> Option<Self> {
            Some(Self(repr & SEVEN))
        }
    }

    impl AtomAdd for Seven {}

    #[test]
    fn a_carry_into_the_last_slots_top_bit_leaves_a_repr_that_decodes() {
        let counts = Atomic::new((1_u8, Seven(SEVEN)));
        assert_eq!(counts.fields().1.fetch_add(1, AcqRel), (1, Seven(SEVEN)), "the tuple before");
        assert_eq!(
            counts.load(Relaxed),
            (1, Seven(0)),
            "the carry kept in its slot, which decodes"
        );
        assert_eq!(counts.fields().1.load(Relaxed), Seven(0), "and the field read alone");
    }

    // Bitwise fields make a bitwise tuple, or a bitwise field.
    const _: () = {
        /// Compiles only where `T` has `AtomBitwise`.
        const fn bitwise<T: AtomBitwise>() {}
        /// Compiles only where `T` has `FieldBitwise`.
        const fn field_bitwise<T: FieldBitwise>() {}
        bitwise::<(u32, u32)>();
        field_bitwise::<(u32, u16)>();
    };

    #[test]
    fn a_whole_tuple_compare_exchanges() {
        let pair = Atomic::new((1_u32, 2_u32));
        pair.store((3, 4), Release);
        assert_eq!(pair.compare_exchange((3, 4), (5, 6), AcqRel, Acquire), Ok((3, 4)), "whole");
        assert_eq!(pair.load(Acquire), (5, 6), "exchanged");
    }

    #[test]
    fn or_combines_each_field_with_its_own() {
        let pair = Atomic::new((5_u32, 6_u32));
        pair.or((0x10, 0x20), Relaxed);
        assert_eq!(pair.load(Acquire), (0x15, 0x26), "field by field");
    }

    #[test]
    fn a_pointer_shares_its_word_with_small_fields() {
        repr_and_validity_are::<(NonNull<Node>,), *mut Node, TotalZeroNiche>();
        repr_and_validity_are::<(NonNull<Node>, bool), *mut Node, ZeroNiche>();
        repr_and_validity_are::<(bool, NonNull<Node>, bool), *mut Node, ZeroNiche>();
        repr_and_validity_are::<(Option<NonNull<Node>>, bool), *mut Node, Total>();
        assert_eq!(<(NonNull<Node>, bool, bool) as Atom>::TAG_WIDTH, 2, "two tag bits");
    }

    #[test]
    fn each_flag_in_a_pointers_low_bits_changes_alone() {
        let head = Atomic::new((node(0), false, true));
        assert!(!head.fields().1.test_and_set(AcqRel), "the first flag clear before");
        assert!(head.fields().2.test_and_clear(AcqRel), "the second set before");
        assert_eq!(head.load(Acquire), (node(0), true, false), "each flag changed alone");
    }

    #[test]
    fn a_pointer_beside_flags_loads_its_tags_cleared_and_its_provenance_kept() {
        let head = Atomic::new((node(0), true, true));
        let pointer = head.fields().0.load(Acquire);
        assert_eq!(pointer, node(0), "the tags cleared");
        // SAFETY: the pointer is a static's.
        #[expect(unsafe_code, reason = "reads through the pointer read back")]
        let value = unsafe { pointer.as_ref() }.value;
        assert_eq!(value, 1, "its provenance kept");
    }

    #[cfg(wide)]
    #[test]
    fn a_pointer_beside_wide_fields_takes_an_integer_word() {
        use atomix_core::DoubleWord;

        repr_and_validity_are::<(NonNull<Node>, u32), DoubleWord<*mut Node, usize>, Partial>();
        repr_and_validity_are::<(u32, NonNull<Node>), DoubleWord<*mut Node, usize>, Partial>();
        let entry = Atomic::new((node(0), 5_u32));
        let (_, seen) = entry.fields().1.update(AcqRel, Acquire, |count| count.wrapping_add(1));
        assert_eq!(seen, 5, "the count before");
        assert_eq!(entry.load_rmw(Acquire), (node(0), 6), "the pointer kept, the count added");
        assert_eq!(entry.fields().0.load_rmw(Acquire), node(0), "read through the word");
        assert_eq!(
            <Option<(NonNull<Node>, u32)> as Atom>::to_repr(None).first,
            ptr::null_mut(),
            "`None` a null pointer beside it"
        );
    }

    #[cfg(wide)]
    #[test]
    fn a_bit_above_the_fields_in_the_integer_word_does_not_decode() {
        use atomix_core::DoubleWord;

        /// The first bit past a `u32`.
        const PAST: usize = 1 << 32;
        let within = DoubleWord { first: node(0).as_ptr(), second: 5 };
        assert_eq!(<(NonNull<Node>, u32)>::from_repr(within), Some((node(0), 5)), "within it");
        let past = DoubleWord { first: node(0).as_ptr(), second: PAST };
        assert_eq!(<(NonNull<Node>, u32)>::from_repr(past), None, "past the `u32`");
    }

    #[cfg(wide)]
    #[test]
    fn two_pointers_keep_small_fields_in_the_first_ones_low_bits() {
        use atomix_core::DoubleWord;

        type Linked = (NonNull<Node>, bool, NonNull<Node>);
        repr_and_validity_are::<Linked, DoubleWord<*mut Node, *mut Node>, Partial>();
        repr_and_validity_are::<(*mut Node, *mut Node), DoubleWord<*mut Node, *mut Node>, ZeroValid>(
        );
        let link = Atomic::new((node(0), false, node(1)));
        assert_eq!(link.load_rmw(Acquire).to_repr().first, node(0).as_ptr(), "no tag set");
        link.fields().1.update(AcqRel, Acquire, |_| true);
        let repr = link.load_rmw(Acquire).to_repr();
        assert_eq!(repr.first.addr(), node(0).as_ptr().addr() | 1, "the flag in the first's bit 0");
        assert_eq!(link.load_rmw(Acquire), (node(0), true, node(1)), "each field read back");
    }

    /// A configuration a reference in a tuple points to.
    #[derive(Debug, PartialEq)]
    #[repr(align(4))]
    struct Config {
        /// How many price levels a quote shows.
        depth: u32,
    }

    /// The configurations in force.
    static CONFIGS: [Config; 2] = [Config { depth: 10 }, Config { depth: 20 }];

    #[test]
    fn a_reference_beside_flags_keeps_its_floor_and_its_flags_take_any_ordering() {
        type Published = (Option<&'static Config>, bool);
        repr_and_validity_are::<Published, ReprOf<&'static Config>, ZeroValid>();
        let published = Atomic::new((Some(&CONFIGS[0]), false));
        // A tag field's bit publishes nothing of the reference: `Relaxed` is enough.
        published.fields().1.set(Relaxed);
        assert!(published.fields().1.load(Relaxed), "set");
        // The reference itself loads with `Acquire` at least: `load(Relaxed)` does not compile.
        let (config, stale) = published.load(Acquire);
        assert_eq!((config.map(|config| config.depth), stale), (Some(10), true), "beside its flag");
        published.store((Some(&CONFIGS[1]), false), Release);
        let read = published.fields().0.load(Acquire).expect("a config is published");
        assert!(ptr::eq(read, &raw const CONFIGS[1]), "the reference, its tag cleared");
    }

    #[cfg(wide)]
    #[test]
    fn a_reference_in_the_second_pointers_place_reads_through() {
        let route = Atomic::new((node(0), &CONFIGS[0]));
        let swapped =
            route.compare_exchange((node(0), &CONFIGS[0]), (node(1), &CONFIGS[1]), AcqRel, Acquire);
        assert_eq!(swapped, Ok((node(0), &CONFIGS[0])), "exchanged");
        let (pointer, config) = route.load_rmw(Acquire);
        assert_eq!(
            (pointer, config.depth),
            (node(1), 20),
            "each pointer, the reference read through"
        );
        let read = route.fields().1.load_rmw(Acquire);
        assert!(ptr::eq(read, &raw const CONFIGS[1]), "its place reads it back");
    }

    /// A hook: what handles a feed's messages.
    type Hook = fn(u64) -> u64;

    /// Twice `value`: a hook.
    fn twice(value: u64) -> u64 {
        value.wrapping_mul(2)
    }

    #[test]
    fn a_function_pointer_alone_is_its_own_word() {
        repr_and_validity_are::<(Hook,), ReprOf<Hook>, TotalZeroNiche>();
        let twice: Hook = twice;
        let hook = Atomic::new((twice,));
        assert_eq!((hook.load(Relaxed).0)(21), 42, "called through the pointer loaded");
    }

    #[cfg(wide)]
    #[test]
    fn a_function_pointer_beside_a_flag_takes_an_integer_word() {
        use atomix_core::DoubleWord;

        // A function's code leaves no low bit clear, so the flag takes a word of its own, as a
        // derived struct's does with `#[atom(repr = u128)]`.
        type Armed = (Option<Hook>, bool);
        repr_and_validity_are::<Armed, DoubleWord<ReprOf<Hook>, usize>, ZeroValid>();
        repr_and_validity_are::<(bool, Hook), DoubleWord<ReprOf<Hook>, usize>, Partial>();
        let twice: Hook = twice;
        let armed = Atomic::new((Some(twice), false));
        let (_, was_armed) = armed.fields().1.update(Relaxed, Relaxed, |_| true);
        assert!(!was_armed, "disarmed before");
        let (hook, now_armed) = armed.load_rmw(Relaxed);
        assert_eq!((hook.map(|hook| hook(21)), now_armed), (Some(42), true), "both, in two words");
        assert_eq!(armed.load_rmw(Relaxed).to_repr().second, 1, "the flag in the second word");
        assert!(armed.fields().1.load_rmw(Relaxed), "read alone");
        for value in [(Some(twice), true), (None, false)] {
            assert_holds(round_trips(value));
        }
    }

    #[cfg(wide)]
    #[test]
    fn a_function_pointer_after_a_pointer_to_data_leaves_the_flag_in_the_first() {
        use atomix_core::DoubleWord;

        type Routed = (NonNull<Node>, Hook, bool);
        repr_and_validity_are::<Routed, DoubleWord<*mut Node, ReprOf<Hook>>, Partial>();
        let twice: Hook = twice;
        let routed = Atomic::new((node(0), twice, true));
        let repr = routed.load_rmw(Relaxed).to_repr();
        assert_eq!(repr.first.addr(), node(0).as_ptr().addr() | 1, "the flag in the node's bit 0");
        let (pointer, hook, flag) = routed.load_rmw(Acquire);
        assert_eq!((pointer, hook(4), flag), (node(0), 8, true), "each field read back");
        assert_holds(round_trips((node(1), twice, false)));
    }

    /// Where the table lies, beside the crate's manifest.
    const TABLE_PATH: &str = "src/atom/tuple/table.rs";

    #[test]
    fn the_shape_table_is_what_its_generator_writes() {
        let table = shape_table();
        if env::var_os("TUPLE_TABLE").is_some_and(|value| value == "overwrite") {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(TABLE_PATH);
            fs::write(path, &table).expect("the table's file is writable");
        }
        assert_eq!(
            include_str!("../src/atom/tuple/table.rs"),
            table,
            "`{TABLE_PATH}` is stale: run this test with `TUPLE_TABLE=overwrite` to write it again"
        );
    }

    /// The table of a tuple's shapes, field by field: a tuple's fields stored as bits share its
    /// pointer's word up to 29 bits, the most any alignment leaves clear, as a derived struct's do,
    /// and past them take an integer word of their own, as they do of any bits beside a function
    /// pointer; an atomic holds 64 bits, or 128 where it holds two words.
    fn shape_table() -> String {
        /// A word's bits.
        const WORD: u32 = 64;
        /// Two words' bits.
        const TWO_WORDS: u32 = 128;
        /// The low bits Rust's largest alignment, 2^29 bytes, leaves clear.
        const MAX_ALIGNMENT_WIDTH: u32 = 29;
        /// The first count of bits past them.
        const PAST_MAX_ALIGNMENT_WIDTH: u32 = 30;
        /// The widths of a field stored as bits.
        const WIDTHS: [u32; 6] = [1, 8, 16, 32, 64, 128];
        /// The lines of the table macro `name`'s rows, each a width and its counts of bits, under
        /// `cfg(wide)` where `wide`.
        fn rows(name: &str, rows: &[(u32, Vec<u32>)], wide: bool) -> Vec<String> {
            let rows: Vec<_> = rows.iter().filter(|(_, counts)| !counts.is_empty()).collect();
            if rows.is_empty() {
                return Vec::new();
            }
            let mut out = Vec::new();
            if wide {
                out.push("#[cfg(wide)]".to_owned());
            }
            out.push(format!("{name}! {{"));
            for (width, counts) in rows {
                let counts: Vec<String> = counts.iter().map(u32::to_string).collect();
                out.push(format!("    {width}: {};", counts.join(" ")));
            }
            out.push("}".to_owned());
            out
        }
        /// The line of the table macro `name` of `counts`, under `cfg(wide)` where `wide`.
        fn flat(name: &str, counts: &RangeInclusive<u32>, wide: bool) -> Vec<String> {
            let counts: Vec<String> = counts.clone().map(|count| count.to_string()).collect();
            let mut out = Vec::new();
            if wide {
                out.push("#[cfg(wide)]".to_owned());
            }
            out.push(format!("{name}!({});", counts.join(" ")));
            out
        }
        /// For each width, the counts of bits in `counts` whose sum with it lies in `sums`.
        fn counts_by_width(
            counts: &RangeInclusive<u32>, sums: &RangeInclusive<u32>,
        ) -> Vec<(u32, Vec<u32>)> {
            WIDTHS
                .iter()
                .map(|&width| {
                    let fit = |&bits: &u32| sums.contains(&bits.strict_add(width));
                    (width, counts.clone().filter(fit).collect())
                })
                .collect()
        }
        let mut out: Vec<String> = [
            "//! The transitions of a tuple's shape, field by field, which `tests/tuples.rs` writes, and",
            "//! fails where this file is not what it writes: a tuple's fields stored as bits share its",
            "//! pointer's word up to 29 bits, the most any alignment leaves clear, as a derived struct's do,",
            "//! and past them take an integer word of their own, as they do of any bits beside a function",
            "//! pointer.",
            "",
            "#[cfg(wide)]",
            "use super::{IntegerWord, TwoPointers};",
            "use super::{",
            "    Fill, Integer, Integers, IntegersBeside, Padding, Pointer, Pointers, Push, Shape, Tagged,",
            "};",
            "use crate::__private::{SelectRepr, Width, narrowest_width};",
            "use crate::atomic::Reach;",
            "#[cfg(wide)]",
            "use crate::primitive::DoubleWord;",
            "use crate::primitive::{ExactBits, PointerRepr, Primitive};",
            "use crate::range::{Tags, mask};",
            "use crate::validity::Validity;",
            "",
        ]
        .map(str::to_owned)
        .to_vec();
        out.extend(rows("integers_push", &counts_by_width(&(0..=WORD), &(0..=WORD)), false));
        out.extend(rows(
            "integers_push",
            &counts_by_width(&(0..=TWO_WORDS), &(WORD + 1..=TWO_WORDS)),
            true,
        ));
        out.extend(flat("integers_to_tagged", &(0..=0), false));
        out.extend(flat("integers_to_pointer", &(1..=MAX_ALIGNMENT_WIDTH), false));
        out.extend(flat("integers_to_integer_word", &(PAST_MAX_ALIGNMENT_WIDTH..=WORD), true));
        let alone_widths = WIDTHS.iter().filter(|&&width| width <= MAX_ALIGNMENT_WIDTH);
        let alone_widths: Vec<String> = alone_widths.map(u32::to_string).collect();
        out.push(format!("pointer_push!({});", alone_widths.join(" ")));
        out.extend(rows(
            "tagged_push",
            &counts_by_width(&(1..=MAX_ALIGNMENT_WIDTH), &(0..=MAX_ALIGNMENT_WIDTH)),
            false,
        ));
        out.extend(rows(
            "tagged_to_integer_word",
            &counts_by_width(&(0..=MAX_ALIGNMENT_WIDTH), &(PAST_MAX_ALIGNMENT_WIDTH..=WORD)),
            true,
        ));
        out.extend(rows("integer_word_push", &counts_by_width(&(1..=WORD), &(0..=WORD)), true));
        out.extend(flat("integers_finish", &(1..=WORD), false));
        out.extend(flat("integers_finish", &(WORD + 1..=TWO_WORDS), true));
        out.extend(flat("tagged_finish", &(0..=MAX_ALIGNMENT_WIDTH), false));
        out.extend(flat("integer_word_finish", &(1..=WORD), true));
        out.extend(flat("tagged_to_two_pointers", &(0..=MAX_ALIGNMENT_WIDTH), true));
        out.extend(rows(
            "two_pointers_push",
            &counts_by_width(&(0..=MAX_ALIGNMENT_WIDTH), &(0..=MAX_ALIGNMENT_WIDTH)),
            true,
        ));
        out.extend(flat("two_pointers_finish", &(0..=MAX_ALIGNMENT_WIDTH), true));
        out.join("\n") + "\n"
    }
}
