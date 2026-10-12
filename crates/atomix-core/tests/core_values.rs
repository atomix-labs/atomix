//! Core's small values as atomic values: an IP address stored as its octets, a socket address's
//! port above its address, a duration's seconds above its nanoseconds, an ordering as its
//! discriminant, a nonzero `char` leaving zero for `None`, and `Reverse` as the value it wraps, its
//! tags too.

#![cfg(not(loom))]

#[cfg(test)]
mod tests {
    use core::cmp::{Ordering, Reverse};
    #[cfg(wide)]
    use core::net::Ipv6Addr;
    use core::net::{Ipv4Addr, SocketAddrV4};
    use core::num::NonZero;
    use core::ptr::NonNull;
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
}
