//! `Atom` for core's IP addresses and IPv4 socket address, each stored as its octets in the order
//! core keeps them, so a load or a store is the integer's own, with no byte swap.
//!
//! The repr's order is then not the address's, which compares its first octet first: an address
//! has no `AtomOrd`, and a maximum of addresses goes through `update`.

#[cfg(wide)]
use core::net::Ipv6Addr;
use core::net::{Ipv4Addr, SocketAddrV4};

use super::{Atom, AtomBitwise};
use crate::range::{ReprRange, mask};
use crate::validity::{Total, ZeroValid};

// SAFETY: the repr is the address's four octets, in memory's order, and every four octets are an
// address, so every repr decodes, by the repr alone, as `from_repr_unchecked` decodes it too; an
// address holds no pointer, so it may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for Ipv4Addr {
    type Repr = u32;
    type Validity = Total;
    const REPRS: ReprRange<u32> = ReprRange::FULL;
    #[inline]
    fn to_repr(self) -> u32 {
        u32::from_ne_bytes(self.octets())
    }
    #[inline]
    fn from_repr(repr: u32) -> Option<Self> {
        Some(Self::from_octets(repr.to_ne_bytes()))
    }
    #[inline]
    unsafe fn from_repr_unchecked(repr: u32) -> Self {
        Self::from_octets(repr.to_ne_bytes())
    }
}

/// And, or, xor and not apply octet by octet, as `Ipv4Addr`'s own `&`, `|` and `!` do: a netmask
/// applies in place.
impl AtomBitwise for Ipv4Addr {}

// SAFETY: as `Ipv4Addr`'s, in sixteen octets.
#[cfg(wide)]
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for Ipv6Addr {
    type Repr = u128;
    type Validity = Total;
    const REPRS: ReprRange<u128> = ReprRange::FULL;
    #[inline]
    fn to_repr(self) -> u128 {
        u128::from_ne_bytes(self.octets())
    }
    #[inline]
    fn from_repr(repr: u128) -> Option<Self> {
        Some(Self::from_octets(repr.to_ne_bytes()))
    }
    #[inline]
    unsafe fn from_repr_unchecked(repr: u128) -> Self {
        Self::from_octets(repr.to_ne_bytes())
    }
}

/// Where a socket address's port lies in its repr: above the address's 32 bits.
const PORT_OFFSET: u32 = u32::BITS;

/// The bits a socket address's repr takes: the address's 32, then the port's 16.
const SOCKET_ADDRESS_BITS: u32 = PORT_OFFSET + u16::BITS;

/// The socket address `repr` holds, its bits above the port ignored.
#[inline]
const fn socket_address(repr: u64) -> SocketAddrV4 {
    let ip = Ipv4Addr::from_octets(repr.wrapping_cast::<u32>().to_ne_bytes());
    SocketAddrV4::new(ip, (repr >> PORT_OFFSET).wrapping_cast())
}

// SAFETY: the repr is the address's octets, as `Ipv4Addr`'s, below the port, the bits above clear,
// within `REPRS`; `from_repr` decodes exactly the reprs whose bits above are clear, by the repr
// alone, each as the address and port it holds, zero among them, as `from_repr_unchecked` does; a
// socket address holds no pointer, so it may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for SocketAddrV4 {
    type Repr = u64;
    type Validity = ZeroValid;
    const REPRS: ReprRange<u64> = ReprRange::new(0, mask(SOCKET_ADDRESS_BITS));
    #[inline]
    fn to_repr(self) -> u64 {
        u64::from(u32::from_ne_bytes(self.ip().octets())) | u64::from(self.port()) << PORT_OFFSET
    }
    #[inline]
    fn from_repr(repr: u64) -> Option<Self> {
        if repr >> SOCKET_ADDRESS_BITS == 0 { Some(socket_address(repr)) } else { None }
    }
    #[inline]
    unsafe fn from_repr_unchecked(repr: u64) -> Self {
        socket_address(repr)
    }
}
