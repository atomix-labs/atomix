//! An address is stored as its octets in core's order, so a load swaps no byte, and its repr, a
//! little-endian integer, compares its last octet first, where the address compares its first: an
//! address has no `AtomOrd`.

use core::net::Ipv4Addr;

use atomix_core::AtomOrd;

/// Compiles only where `T`'s reprs order as its values.
const fn orders_as_its_reprs<T: AtomOrd>() {}

fn main() {
    orders_as_its_reprs::<Ipv4Addr>();
}
