//! Core's small values as atomic values: an IP address stored as its octets, a socket address's
//! port above its address, a duration's seconds above its nanoseconds, an ordering as its
//! discriminant, a nonzero `char` leaving zero for `None`, and `Reverse` as the value it wraps, its
//! tags too; an enum of payloads its tag above the wider payload, a range the array of its bounds,
//! `Poll` an `Option`, and an alignment, a layout and an ASCII character their numbers.

#![cfg(not(loom))]
#![feature(ascii_char, ptr_alignment_type)]

#[cfg(test)]
mod tests {
    #[cfg(wide)]
    use core::alloc::Layout;
    use core::ascii::Char as AsciiChar;
    use core::cmp::{Ordering, Reverse};
    use core::convert::Infallible;
    use core::mem::{Alignment, ManuallyDrop};
    #[cfg(wide)]
    use core::net::Ipv6Addr;
    use core::net::{Ipv4Addr, SocketAddrV4};
    use core::num::{FpCategory, NonZero};
    use core::ops::{Bound, ControlFlow};
    use core::ptr::NonNull;
    use core::range::{Range, RangeFrom};
    use core::task::Poll;
    #[cfg(wide)]
    use core::time::Duration;

    use atomix_core::ordering::{AcqRel, Acquire, Relaxed};
    use atomix_core::{Atom, Atomic};

    /// A node aligned to eight bytes, whose pointer leaves three low bits clear.
    #[repr(align(8))]
    struct Node;

    /// A node that lives as long as the tests.
    static NODE: Node = Node;

    #[test]
    fn an_ipv4_address_is_stored_as_its_octets() {
        let octets = [192, 168, 7, 9];
        assert_eq!(Ipv4Addr::from(octets).to_repr(), u32::from_ne_bytes(octets), "no byte swap");
    }

    #[cfg(wide)]
    #[test]
    fn an_ipv6_address_is_stored_as_its_octets() {
        let address = Ipv6Addr::LOCALHOST;
        assert_eq!(address.to_repr(), u128::from_ne_bytes(address.octets()), "no byte swap");
    }

    #[test]
    fn a_netmask_applies_in_place() {
        let peer = Atomic::new(Ipv4Addr::new(192, 168, 7, 9));
        peer.and(Ipv4Addr::new(255, 255, 255, 0), AcqRel);
        assert_eq!(peer.load(Acquire), Ipv4Addr::new(192, 168, 7, 0), "the network kept");
    }

    #[test]
    fn a_socket_address_holds_its_port_above_its_address() {
        let address = Ipv4Addr::new(10, 0, 0, 1);
        let repr = SocketAddrV4::new(address, 8080).to_repr();
        assert_eq!(repr, u64::from(address.to_repr()) | 8080 << 32, "the port from bit 32");
    }

    #[test]
    fn a_socket_address_refuses_a_bit_past_its_port() {
        assert_eq!(SocketAddrV4::from_repr(1 << 48), None, "bit 48, past the port");
    }

    #[cfg(wide)]
    #[test]
    fn a_duration_holds_its_seconds_above_its_nanoseconds() {
        let repr = Duration::new(3, 7).to_repr();
        assert_eq!(
            repr,
            u128::from(3_u64) << 64 | u128::from(7_u32),
            "the seconds in the upper word"
        );
    }

    #[cfg(wide)]
    #[test]
    fn a_duration_refuses_a_second_of_nanoseconds() {
        assert_eq!(Duration::from_repr(1_000_000_000), None, "a billion nanoseconds");
        assert_eq!(Duration::from_repr(1 << 32), None, "a bit between the fields");
    }

    #[cfg(wide)]
    #[test]
    fn the_longest_duration_round_trips() {
        let longest = Atomic::new(Duration::MAX);
        assert_eq!(longest.load_rmw(Acquire), Duration::MAX, "every second and nanosecond");
    }

    #[cfg(wide)]
    #[test]
    fn none_is_no_duration() {
        let none = <Option<Duration> as Atom>::to_repr(None);
        assert_eq!(Duration::from_repr(none), None, "`None` decodes as no duration");
    }

    #[test]
    fn an_ordering_is_its_discriminant() {
        let reprs = [Ordering::Less, Ordering::Equal, Ordering::Greater].map(Atom::to_repr);
        assert_eq!(reprs, [-1, 0, 1], "its `#[repr(i8)]`");
    }

    #[test]
    fn an_option_of_an_ordering_spends_a_spare_repr_on_none() {
        assert_eq!(size_of::<Atomic<Option<Ordering>>>(), 1, "one byte");
        assert_eq!(
            Ordering::from_repr(<Option<Ordering> as Atom>::to_repr(None)),
            None,
            "`None` is no ordering"
        );
    }

    #[cfg(aarch64_code)]
    #[test]
    fn the_greater_ordering_is_kept_by_fetch_max() {
        let last = Atomic::new(Ordering::Less);
        assert_eq!(last.fetch_max(Ordering::Greater, AcqRel), Ordering::Less, "the value before");
        assert_eq!(last.load(Relaxed), Ordering::Greater, "the greater kept");
    }

    #[test]
    fn a_nonzero_char_leaves_zero_for_none() {
        assert_eq!(<Option<NonZero<char>> as Atom>::to_repr(None), 0, "`None` is zero");
    }

    #[test]
    fn a_surrogate_is_no_nonzero_char() {
        assert_eq!(NonZero::<char>::from_repr(0xD800), None, "the first surrogate");
    }

    #[test]
    fn reverse_keeps_its_values_bitwise_operations() {
        let reversed = Atomic::new(Reverse(5_u32));
        reversed.or(Reverse(2), Relaxed);
        assert_eq!(reversed.load(Relaxed), Reverse(7), "or, on the value");
    }

    #[test]
    fn reverse_of_a_pointer_keeps_its_pointees_alignment_for_tags() {
        let pointer = Reverse(NonNull::from(&NODE));
        let marked = Atomic::new((pointer, true));
        assert_eq!(marked.load(Acquire), (pointer, true), "a flag in the pointer's low bit");
    }

    #[test]
    fn a_result_holds_its_tag_above_the_wider_payload() {
        assert_eq!(Ok::<u32, u8>(7).to_repr(), 7_u64, "`Ok`'s tag zero");
        assert_eq!(Err::<u32, u8>(3).to_repr(), 1 << 32 | 0x3, "`Err`'s one, at bit 32");
    }

    #[test]
    fn a_result_refuses_a_bit_past_its_payload() {
        assert_eq!(Result::<u32, u8>::from_repr(1 << 32 | 0x100), None, "a ninth bit of the `u8`");
    }

    #[test]
    fn a_result_refuses_a_tag_past_its_last_variant() {
        assert_eq!(Result::<u32, u8>::from_repr(2 << 32), None, "a tag past `Err`'s");
    }

    #[test]
    fn an_atomic_result_changes_variant_through_update() {
        let reply = Atomic::new(Ok::<u32, u8>(5));
        let before = reply.update(AcqRel, Acquire, |reply| reply.and(Err(9)));
        assert_eq!((before, reply.load(Acquire)), (Ok(5), Err(9)), "the error replaces the value");
    }

    #[test]
    fn control_flow_continues_at_tag_zero() {
        assert_eq!(ControlFlow::<u8, u16>::Continue(4).to_repr(), 4_u32, "`Continue` first");
        assert_eq!(ControlFlow::<u8, u16>::Break(4).to_repr(), 1 << 16 | 0x4, "`Break` above");
    }

    #[test]
    fn an_unbounded_bound_holds_no_payload() {
        assert_eq!(Bound::<u16>::Unbounded.to_repr(), 2 << 16, "tag two, the payload clear");
        assert_eq!(Bound::<u16>::from_repr(2 << 16 | 0x1), None, "a payload beside it is none");
    }

    #[test]
    fn a_range_is_the_array_of_its_bounds() {
        let range = Range { start: 1_u32, end: 5 };
        assert_eq!(range.to_repr(), [1_u32, 5].to_repr(), "the start lowest");
        assert_eq!(RangeFrom { start: 3_u8 }.to_repr(), 3, "and one bound, itself");
    }

    #[cfg(wide)]
    #[test]
    fn a_range_of_pointers_takes_two_words() {
        let node = NonNull::from(&NODE);
        assert_eq!(size_of::<Atomic<Range<NonNull<Node>>>>(), 16, "a double word");
        assert_eq!(Range { start: node, end: node }.to_repr().first, node.as_ptr(), "the start");
    }

    #[test]
    fn pending_is_an_options_none() {
        let none = <Option<NonZero<u8>> as Atom>::to_repr(None);
        assert_eq!(Poll::<NonZero<u8>>::Pending.to_repr(), none, "zero, a nonzero's niche");
    }

    #[test]
    fn an_alignment_is_its_logarithm() {
        let alignment = Alignment::of::<u64>();
        assert_eq!(alignment.to_repr(), 3, "eight bytes, two to the third");
        assert_eq!(Alignment::from_repr(64), None, "past a word's bits");
    }

    #[test]
    fn an_ascii_character_is_its_code() {
        let letter = AsciiChar::from_u8(b'A').expect("an ASCII letter");
        assert_eq!(letter.to_repr(), b'A', "its code");
        assert_eq!(AsciiChar::from_repr(0x80), None, "past ASCII");
    }

    #[cfg(wide)]
    #[test]
    fn a_layout_holds_its_alignment_above_its_size() {
        let layout = Layout::new::<[u64; 3]>();
        assert_eq!(layout.to_repr(), 8 << 64 | 0x18, "aligned to 8, 24 bytes");
        assert_eq!(Layout::from_repr(3 << 64), None, "no alignment of three");
    }

    #[test]
    fn a_float_category_is_its_index() {
        assert_eq!(FpCategory::Normal.to_repr(), 4, "the last");
        assert_eq!(FpCategory::from_repr(5), None, "past it");
    }

    #[test]
    fn an_option_of_infallible_is_only_none() {
        assert_eq!(<Option<Infallible> as Atom>::from_repr(0), None, "zero is no value either");
        assert_eq!(Atomic::new(None::<Infallible>).load(Acquire), None, "`None` alone");
    }

    #[test]
    fn manually_drop_keeps_its_values_add() {
        let count = Atomic::new(ManuallyDrop::new(1_u32));
        count.fetch_add(2, AcqRel);
        assert_eq!(*count.load(Acquire), 3, "one and two");
    }
}
