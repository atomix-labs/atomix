//! The listings of the chapter Working with Other Crates, each under the feature that brings its
//! crate's traits.

#[cfg(feature = "serde")]
#[test]
// ANCHOR: serde
fn an_atomic_is_written_as_its_value_and_a_range_is_held_on_the_way_in() {
    use atomix::ordering::Acquire;
    use atomix::{Atomic, RangedU8};
    use serde::{Deserialize, Serialize};

    /// An order book's config, as a file spells it: how many levels the book shows, and whether
    /// it is halted.
    #[derive(Serialize, Deserialize)]
    struct BookConfig {
        depth: Atomic<RangedU8<1, 10>>,
        halted: Atomic<bool>,
    }

    let config: BookConfig =
        serde_json::from_str(r#"{"depth":5,"halted":false}"#).expect("a depth of 5 lies in 1..=10");
    assert_eq!(config.depth.load(Acquire).get(), 5, "each atomic holds the value read");
    let text = serde_json::to_string(&config).expect("serde_json writes any config");
    assert_eq!(text, r#"{"depth":5,"halted":false}"#, "and each is written as its value");

    let refused = serde_json::from_str::<BookConfig>(r#"{"depth":11,"halted":false}"#);
    let message = refused.err().map(|error| error.to_string());
    let expected = "invalid value: integer `11`, expected an integer in 1..=10 at line 1 column 11";
    assert_eq!(message.as_deref(), Some(expected), "a depth past the range, refused");
}
// ANCHOR_END: serde

#[cfg(feature = "zerocopy-08")]
#[test]
// ANCHOR: zerocopy
fn a_page_of_shared_memory_holds_atomics() {
    use core::num::NonZero;

    use atomix::ordering::{Acquire, Release};
    use atomix::{Atomic, AtomicU64};
    use zerocopy::{FromBytes, IntoBytes, KnownLayout};

    /// The header of a page processes share: every repr of each field decodes, so any bytes do,
    /// and `repr(C)` of two words leaves no padding.
    #[derive(FromBytes, IntoBytes, KnownLayout)]
    #[repr(C)]
    struct Header {
        sequence: AtomicU64,
        owner: Atomic<Option<NonZero<u64>>>,
    }

    // Words, so the page is aligned as `Header` is.
    let mut page = [0_u64; 2];
    {
        // This process lays the page out before another maps it, so it holds the bytes alone, and
        // `&mut` reaches them; the header is shared from here on.
        let header: &Header =
            Header::mut_from_bytes(page.as_mut_bytes()).expect("16 aligned bytes");
        header.sequence.store(1, Release);
        header.owner.store(NonZero::new(42), Release);
    }

    // Another process may write the page as this one reads it, so a `&mut [u8]` over it would be
    // aliased: each atomic is reached through a pointer instead.
    let owner = &raw mut page[1];
    // SAFETY: `owner` points to the page's second word, aligned, live while `seen` is, and every
    // repr of an `Option<NonZero<u64>>` decodes; only atomics reach the word while `seen` lives.
    #[expect(unsafe_code, reason = "a view of memory only the caller knows")]
    let seen = unsafe { Atomic::<Option<NonZero<u64>>>::from_ptr(owner) };
    assert_eq!(seen.load(Acquire), NonZero::new(42), "the owner the header stored");
}
// ANCHOR_END: zerocopy

#[cfg(feature = "bytemuck")]
#[test]
// ANCHOR: bytemuck
fn a_slot_of_atomics_is_zeroed_where_each_zero_decodes() {
    use core::num::NonZero;

    use atomix::ordering::Acquire;
    use atomix::{Atom, Atomic, AtomicU64};
    use bytemuck::Zeroable;

    /// What a slot holds: `Empty` is zero, so zero decodes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum State {
        Empty,
        Full,
    }

    /// A slot of a ring buffer, which a buffer of zeros holds empty.
    #[derive(Zeroable)]
    #[repr(C)]
    struct Slot {
        sequence: AtomicU64,
        state: Atomic<State>,
        owner: Atomic<Option<NonZero<u32>>>,
    }

    let slot = Slot::zeroed();
    let read = (slot.sequence.load(Acquire), slot.state.load(Acquire), slot.owner.load(Acquire));
    assert_eq!(read, (0, State::Empty, None), "zero, empty, and no owner");
    assert_ne!(State::Full.to_repr(), 0, "where a full slot is not zero");
}
// ANCHOR_END: bytemuck

#[cfg(feature = "arbitrary")]
#[test]
// ANCHOR: arbitrary
fn an_atomic_is_built_from_unstructured_bytes() {
    use arbitrary::{Arbitrary, Unstructured};
    use atomix::{Atomic, RangedU8};

    // A fuzzer's bytes, which an atomic reads as its value does.
    let bytes = [200, 7, 3];
    let depth = Atomic::<RangedU8<1, 10>>::arbitrary(&mut Unstructured::new(&bytes))
        .expect("a depth draws from any non-empty range");
    let alone = RangedU8::<1, 10>::arbitrary(&mut Unstructured::new(&bytes))
        .expect("a depth draws from any non-empty range");
    assert_eq!(depth.into_inner(), alone, "the depth the same bytes give alone");
}
// ANCHOR_END: arbitrary

#[cfg(feature = "arbitrary-int")]
#[test]
// ANCHOR: arbitrary_int
fn a_field_of_four_bits_or_sixty_takes_that_many() {
    use arbitrary_int::{u4, u60};
    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic};

    /// A task: four bits of flags, then a count of references that fills the word, at its top.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Task {
        flags: u4,
        references: u60,
    }

    let task = Atomic::new(Task { flags: u4::new(0), references: u60::new(1) });
    // `or` on the four bits alone, and an add whose carry leaves the word: one instruction each.
    task.fields().flags.or(u4::new(0b10), Release);
    let before = task.fields().references.fetch_add(1, AcqRel);
    assert_eq!(before.references, u60::new(1), "one reference before");
    assert_eq!(task.load(Acquire), Task { flags: u4::new(0b10), references: u60::new(2) }, "both");
    assert_eq!(size_of::<Atomic<Task>>(), 8, "in one word");
}
// ANCHOR_END: arbitrary_int

#[cfg(feature = "deranged-05")]
#[test]
// ANCHOR: deranged
fn deranged_integers_are_values_and_convert_to_atomix_integers() {
    use atomix::ordering::{Acquire, Release};
    use atomix::{Atomic, RangedU8};
    use deranged::RangedU8 as DerangedU8;

    /// How deep a level lies in an order book, as deranged's integer.
    type Depth = DerangedU8<1, 10>;

    let depth: Atomic<Option<Depth>> = Atomic::new(None);
    depth.store(Depth::new(5), Release);
    let read = depth.load(Acquire).expect("a depth was stored");
    let own: RangedU8<1, 10> = read.into();
    assert_eq!(own.get(), 5, "atomix's own, of the same bounds");
    assert_eq!(Depth::from(own), read, "and back, the same integer");
}
// ANCHOR_END: deranged
