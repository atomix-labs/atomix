//! Core's small values' operations: an IPv4 address's and a nonzero `char`'s load, the integer's,
//! with no byte swap, and an IPv6 address's, the `u128`'s; a netmask's `and`; an ordering's and a
//! socket address's load, and an ordering's maximum, one instruction; a duration's load and a
//! layout's, their two words moved as they are; a `Result`'s load and `is_ok`, and a `Bound`'s and
//! a range's load; and an alignment's load, its logarithm shifted back.

#[cfg(any(target_feature = "avx", target_feature = "lse2"))]
use core::alloc::Layout;
use core::cmp::Ordering;
use core::mem::Alignment;
#[cfg(any(target_feature = "avx", target_feature = "lse2"))]
use core::net::Ipv6Addr;
use core::net::{Ipv4Addr, SocketAddrV4};
use core::num::NonZero;
use core::ops::Bound;
use core::range::Range;
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

#[unsafe(no_mangle)]
pub fn result_load(atomic: &Atomic<Result<u32, u8>>) -> Result<u32, u8> {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn result_is_ok(atomic: &Atomic<Result<u32, u8>>) -> bool {
    atomic.load(Acquire).is_ok()
}

#[unsafe(no_mangle)]
pub fn bound_load(atomic: &Atomic<Bound<u16>>) -> Bound<u16> {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn range_load(atomic: &Atomic<Range<u32>>) -> Range<u32> {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn alignment_load(atomic: &Atomic<Alignment>) -> Alignment {
    atomic.load(Acquire)
}

#[cfg(any(target_feature = "avx", target_feature = "lse2"))]
#[unsafe(no_mangle)]
pub fn layout_load(atomic: &Atomic<Layout>) -> Layout {
    atomic.load(Acquire)
}
