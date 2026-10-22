//! Tuples beside the structs `#[derive(Atom)]` writes: a tuple is laid out as the struct of its
//! fields where each field fills its repr, a pointer beside a flag in one word, a function pointer
//! beside one in two, and integers side by side; a field narrower than its repr, which a struct
//! packs at its range's width, its sign extended above it where it is signed and on top, a tuple
//! keeps in its whole slot, the bits above clear; a tuple of derived values; and a tuple as a field
//! of a derived struct, its fields places through the struct's.

#![cfg(feature = "derive")]
#![cfg(not(loom))]

#[cfg(test)]
#[path = "../../atomix-core/tests/testing/mod.rs"]
mod testing;

#[cfg(test)]
mod tests {
    use core::ptr::NonNull;

    #[cfg(wide)]
    use atomix::Primitive;
    use atomix::ordering::{AcqRel, Acquire, Relaxed, Release};
    use atomix::validity::ZeroValid;
    use atomix::{Atom, Atomic, RangedI8};

    use crate::testing::atom::repr_and_validity_are;
    use crate::testing::law::{assert_holds, decodes_as_promised};

    /// A node aligned to eight bytes.
    #[repr(align(8))]
    struct Node;

    /// A node that lives as long as the tests.
    static NODE: Node = Node;

    /// A Treiber stack's head: the struct of the tuple `(Option<NonNull<Node>>, bool)`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Head {
        /// The top node.
        top: Option<NonNull<Node>>,
        /// Whether the stack is closed.
        closed: bool,
    }

    /// A quote: the struct of the tuple `(u32, u16)`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Quote(u32, u16);

    /// The side of the book an order rests on: one bit in a struct, a byte in a tuple.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    enum Side {
        /// A buy.
        Bid,
        /// A sell.
        Ask,
    }

    /// Two sides, packed a bit each.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Sides(Side, Side);

    /// An order's quantity, whether it is live, and its skew from the mid price: the struct of the
    /// tuple `(u32, bool, RangedI8<-5, 5>)`, its skew in four bits at its top.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Order(u32, bool, RangedI8<-5, 5>);

    /// The best bid and ask beside whether the book trades: a tuple as a field.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Book {
        /// The best bid and ask, in ticks.
        best: (u16, u16),
        /// Whether the book trades.
        live: bool,
    }

    #[test]
    fn a_tuple_is_laid_out_as_the_struct_of_its_fields() {
        let top = Some(NonNull::from(&NODE));
        for closed in [false, true] {
            assert_eq!(
                (top, closed).to_repr(),
                Head { top, closed }.to_repr(),
                "a pointer beside a flag, in one word"
            );
        }
        assert_eq!((7_u32, 9_u16).to_repr(), Quote(7, 9).to_repr(), "integers side by side");
    }

    /// A hook beside whether it is armed: the struct of the tuple `(Option<Hook>, bool)`, its flag
    /// in a word of its own, since a function's code leaves no low bit clear.
    #[cfg(wide)]
    #[derive(Clone, Copy, Atom)]
    #[atom(repr = u128)]
    struct Armed {
        /// What handles each message.
        hook: Option<fn(u64) -> u64>,
        /// Whether it runs.
        armed: bool,
    }

    /// Twice `value`: a hook.
    #[cfg(wide)]
    fn twice(value: u64) -> u64 {
        value.wrapping_mul(2)
    }

    #[cfg(wide)]
    #[test]
    fn a_function_pointer_beside_a_flag_is_laid_out_as_the_struct_of_them() {
        let hook: Option<fn(u64) -> u64> = Some(twice);
        for armed in [false, true] {
            assert_eq!(
                (hook, armed).to_repr().packed_bits(),
                Armed { hook, armed }.to_repr().packed_bits(),
                "the hook in one word, the flag in the next"
            );
        }
    }

    #[test]
    fn a_tuple_keeps_a_narrow_value_in_its_slot() {
        repr_and_validity_are::<Sides, u8, ZeroValid>();
        assert_eq!(Sides(Side::Ask, Side::Ask).to_repr(), 0b11, "a struct packs a bit each");
        assert_eq!((Side::Ask, Side::Ask).to_repr(), 0x0101, "a tuple keeps a byte each");
        for bits in 0..=u16::MAX {
            assert_holds(decodes_as_promised::<_, (Side, Side)>(bits));
        }
    }

    #[test]
    fn a_tuple_keeps_a_narrow_signed_top_field_in_its_slot() {
        let skew = RangedI8::<-5, 5>::new(-1).expect("-1 lies in -5 to 5");
        assert_eq!(
            Order(1, false, skew).to_repr(),
            0xFFFF_FFFE_0000_0001,
            "a struct extends its top field's sign through the bits above"
        );
        assert_eq!(
            (1_u32, false, skew).to_repr(),
            0x0000_01FE_0000_0001,
            "a tuple keeps the field in its slot, the bits above clear"
        );
    }

    #[test]
    fn a_tuple_field_projects_through_the_struct() {
        repr_and_validity_are::<Book, u64, ZeroValid>();
        let book = Atomic::new(Book { best: (100, 101), live: false });
        assert_eq!(book.fields().best.fields().1.load(Acquire), 101, "the ask");
        book.fields().best.fields().0.update(AcqRel, Acquire, |bid| bid.wrapping_sub(1));
        book.fields().live.set(Release);
        assert_eq!(book.load(Relaxed), Book { best: (99, 101), live: true }, "each alone changed");
    }

    #[cfg(wide)]
    #[test]
    fn a_tuple_of_derived_values_projects_each_one() {
        // The quote's 64 bits are too many to share the head's word: two words, which change in
        // a compare-exchange loop.
        let pair = Atomic::new((Quote(1, 2), Head { top: None, closed: false }));
        let (quote, head) = pair.fields();
        assert_eq!(quote.load_rmw(Acquire), Quote(1, 2), "the quote");
        head.fields().closed.update(AcqRel, Acquire, |_| true);
        assert_eq!(pair.load_rmw(Acquire).1, Head { top: None, closed: true }, "set");
    }
}
