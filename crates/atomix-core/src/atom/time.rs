//! `Atom` for core's `Duration`, its whole seconds in a word above its nanoseconds: each field a
//! word of the repr's own, so neither moves through a shift, and the repr orders as the duration.

use core::hint::assert_unchecked;
use core::time::Duration;

use super::{Atom, AtomOrd};
use crate::range::ReprRange;
use crate::validity::ZeroValid;

/// Where a duration's whole seconds lie in its repr: the upper word.
const SECONDS_OFFSET: u32 = u64::BITS;

/// Nanoseconds in a second, above the most a duration's nanoseconds reach.
const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// The greatest repr: `Duration::MAX`'s, whose nanoseconds are a second's but one.
const MAX_REPR: u128 = u128::MAX << SECONDS_OFFSET | u128::from(NANOS_PER_SECOND - 1);

// SAFETY: the repr is the whole seconds in the upper word and the nanoseconds, below a billion, in
// the lower, the bits between clear, so it lies in `REPRS`, which ends at `Duration::MAX`'s;
// `from_repr` decodes exactly those reprs, by the repr alone, zero among them, each as its seconds
// and nanoseconds, as `from_repr_unchecked` does, which `new`'s carry, never taken below a
// billion, leaves as they are; a duration holds no pointer, so it may cross threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for Duration {
    type Repr = u128;
    type Validity = ZeroValid;
    const REPRS: ReprRange<u128> = ReprRange::new(0, MAX_REPR);
    #[inline]
    fn to_repr(self) -> u128 {
        u128::from(self.as_secs()) << SECONDS_OFFSET | u128::from(self.subsec_nanos())
    }
    #[inline]
    fn from_repr(repr: u128) -> Option<Self> {
        let nanos = repr.wrapping_cast::<u64>();
        if nanos < NANOS_PER_SECOND {
            Some(Self::new((repr >> SECONDS_OFFSET).wrapping_cast(), nanos.wrapping_cast()))
        } else {
            None
        }
    }
    // The default, `from_repr`'s unwrap, leaves a compare and a select of `None`'s nanoseconds on
    // every load, which `duration_load`'s pin refuses.
    #[inline]
    unsafe fn from_repr_unchecked(repr: u128) -> Self {
        let nanos = repr.wrapping_cast::<u64>();
        // SAFETY: the caller's repr decodes, so its lower word, the nanoseconds, lies below a
        // billion, which spares `new` its carry.
        unsafe {
            assert_unchecked(nanos < NANOS_PER_SECOND);
        }
        Self::new((repr >> SECONDS_OFFSET).wrapping_cast(), nanos.wrapping_cast())
    }
}

/// Seconds, then nanoseconds, as the repr's words order.
impl AtomOrd for Duration {}
