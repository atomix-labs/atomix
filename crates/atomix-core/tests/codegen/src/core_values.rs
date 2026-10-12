//! Core's small values' operations: an IPv4 address's and a nonzero `char`'s load, the integer's,
//! with no byte swap, and an IPv6 address's, the `u128`'s; a netmask's `and`; an ordering's and a
//! socket address's load, and an ordering's maximum, one instruction; and a duration's load, its
//! two words moved as they are.

use core::cmp::Ordering;
#[cfg(any(target_feature = "avx", target_feature = "lse2"))]
use core::net::Ipv6Addr;
use core::net::{Ipv4Addr, SocketAddrV4};
use core::num::NonZero;
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
use core::time::Duration;

use atomix_core::Atomic;
use atomix_core::ordering::{AcqRel, Acquire};

#[unsafe(no_mangle)]
pub fn ipv4_load(atomic: &Atomic<Ipv4Addr>) -> Ipv4Addr {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn ipv4_and(atomic: &Atomic<Ipv4Addr>, mask: Ipv4Addr) {
    atomic.and(mask, AcqRel);
}

#[unsafe(no_mangle)]
pub fn socket_address_load(atomic: &Atomic<SocketAddrV4>) -> SocketAddrV4 {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn nonzero_char_load(atomic: &Atomic<NonZero<char>>) -> NonZero<char> {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn cmp_ordering_load(atomic: &Atomic<Ordering>) -> Ordering {
    atomic.load(Acquire)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", feature = "aarch64-only"))]
#[unsafe(no_mangle)]
pub fn cmp_ordering_fetch_max(atomic: &Atomic<Ordering>, value: Ordering) -> Ordering {
    atomic.fetch_max(value, AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
#[unsafe(no_mangle)]
pub fn duration_load_rmw(atomic: &Atomic<Duration>) -> Duration {
    atomic.load_rmw(Acquire)
}

#[cfg(any(target_feature = "avx", target_feature = "lse2"))]
#[unsafe(no_mangle)]
pub fn duration_load(atomic: &Atomic<Duration>) -> Duration {
    atomic.load(Acquire)
}

#[cfg(any(target_feature = "avx", target_feature = "lse2"))]
#[unsafe(no_mangle)]
pub fn ipv6_load(atomic: &Atomic<Ipv6Addr>) -> Ipv6Addr {
    atomic.load(Acquire)
}
