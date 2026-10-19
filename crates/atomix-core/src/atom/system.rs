//! `Atom` for std's values of the system, with the `std` feature: a `SystemTime`, its signed whole
//! seconds since the Unix epoch in the upper word and its nanoseconds in the lower; an
//! `ExitStatus`, its raw status; and a `BorrowedFd`, its descriptor sealed in a [`ReprOf`], which
//! no bits forge.
//!
//! None is `const`, as std's conversions are not, so an atomic of one is made with `Atomic::from`,
//! not `Atomic::new`.

#[cfg(wide)]
use core::time::Duration;
#[cfg(unix)]
use std::os::fd::{AsRawFd, BorrowedFd};
#[cfg(wait_status)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
#[cfg(exit_status)]
use std::process::ExitStatus;
#[cfg(wide)]
use std::time::{SystemTime, UNIX_EPOCH};

use super::Atom;
#[cfg(wide)]
use super::AtomOrd;
#[cfg(unix)]
use crate::ordering::Relaxed;
#[cfg(unix)]
use crate::primitive::{ReprOf, SealedValue};
#[cfg(unix)]
use crate::range::PointeeAlignment;
use crate::range::ReprRange;
#[cfg(exit_status)]
use crate::validity::Total;
#[cfg(unix)]
use crate::validity::TotalZeroNiche;
#[cfg(wide)]
use crate::validity::ZeroValid;

/// Where a time's whole seconds lie in its repr: the upper word.
#[cfg(wide)]
const SECONDS_OFFSET: u32 = u64::BITS;

/// Nanoseconds in a second, above the most a time's nanoseconds reach.
#[cfg(wide)]
const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// The whole seconds and nanoseconds of `time` since the Unix epoch, the seconds rounded down, so
/// a time before it has negative seconds and nanoseconds forward from them.
#[cfg(wide)]
fn seconds_and_nanos(time: SystemTime) -> (i64, u32) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => (after.as_secs().cast_signed(), after.subsec_nanos()),
        Err(before) => {
            let before = before.duration();
            let seconds = 0_i64.wrapping_sub_unsigned(before.as_secs());
            match before.subsec_nanos() {
                0 => (seconds, 0),
                nanos => (seconds.wrapping_sub(1), NANOS_PER_SECOND.wrapping_sub(nanos)),
            }
        },
    }
}

// Its badge writes `wide` out: a time is 16 bytes.
#[doc(cfg(all(
    feature = "std",
    any(unix, windows),
    any(
        target_arch = "aarch64",
        target_arch = "arm64ec",
        all(target_arch = "x86_64", target_feature = "cmpxchg16b")
    )
)))]
// SAFETY: the repr is the time's whole seconds since the epoch, rounded down, in the upper word,
// which holds every one, as Unix's `tv_sec` and Windows' count of hundreds of nanoseconds are each
// an `i64`, and its nanoseconds, below a billion, in the lower, so it lies in `REPRS`; `from_repr`
// decodes exactly the reprs whose nanoseconds lie below a billion and which make a time the
// platform holds, by the repr alone, each as that time, zero among them, as the epoch; on Windows,
// which counts time in hundreds of nanoseconds, it decodes only those whose nanoseconds a count of
// them makes, so each repr that decodes is its time's own; the default unchecked decode unwraps it;
// a time holds no pointer, so it may cross threads.
#[cfg(wide)]
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
unsafe impl Atom for SystemTime {
    type Repr = i128;
    type Validity = ZeroValid;
    const REPRS: ReprRange<i128> = ReprRange::from_signed(
        i128::from(i64::MIN) << SECONDS_OFFSET,
        i128::from(i64::MAX) << SECONDS_OFFSET | i128::from(NANOS_PER_SECOND - 1),
    );
    #[inline]
    fn to_repr(self) -> i128 {
        let (seconds, nanos) = seconds_and_nanos(self);
        i128::from(seconds) << SECONDS_OFFSET | i128::from(nanos)
    }
    #[inline]
    fn from_repr(repr: i128) -> Option<Self> {
        let nanos = repr.cast_unsigned().wrapping_cast::<u64>();
        if nanos >= u64::from(NANOS_PER_SECOND) || cfg!(windows) && nanos % 100 != 0 {
            return None;
        }
        let (seconds, nanos) =
            ((repr >> SECONDS_OFFSET).wrapping_cast::<i64>(), nanos.wrapping_cast());
        if seconds >= 0 {
            UNIX_EPOCH.checked_add(Duration::new(seconds.cast_unsigned(), nanos))
        } else {
            // Before the epoch by the whole seconds, less the nanoseconds forward from them.
            let before =
                Duration::new(seconds.unsigned_abs(), 0).checked_sub(Duration::new(0, nanos))?;
            UNIX_EPOCH.checked_sub(before)
        }
    }
}

/// Seconds, then nanoseconds, as the repr's words order, signed.
#[cfg(wide)]
impl AtomOrd for SystemTime {}

// SAFETY: the repr is the raw wait status, and every one is a status, so every repr decodes, by the
// repr alone, as `from_raw` makes it, which `into_raw` gives back; a status holds no pointer, so
// it may cross threads.
#[cfg(wait_status)]
#[doc(cfg(all(feature = "std", unix, not(target_os = "fuchsia"))))]
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
unsafe impl Atom for ExitStatus {
    type Repr = i32;
    type Validity = Total;
    const REPRS: ReprRange<i32> = ReprRange::FULL;
    #[inline]
    fn to_repr(self) -> i32 {
        self.into_raw()
    }
    #[inline]
    fn from_repr(repr: i32) -> Option<Self> {
        Some(Self::from_raw(repr))
    }
}

// SAFETY: the repr is the exit code's bits, and every one is a code, so every repr decodes, by the
// repr alone, as `from_raw` makes it, which `code` gives back, always some on Windows; a status
// holds no pointer, so it may cross threads.
#[cfg(windows)]
#[doc(cfg(all(feature = "std", windows)))]
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
unsafe impl Atom for ExitStatus {
    type Repr = u32;
    type Validity = Total;
    const REPRS: ReprRange<u32> = ReprRange::FULL;
    #[inline]
    fn to_repr(self) -> u32 {
        self.code().map_or(0, i32::cast_unsigned)
    }
    #[inline]
    fn from_repr(repr: u32) -> Option<Self> {
        Some(Self::from_raw(repr))
    }
}

// A descriptor is no pointer: its repr holds no tags, and its operations take any ordering, as it
// names a resource of the kernel's, which no write of this process makes.
#[cfg(unix)]
impl SealedValue for BorrowedFd<'_> {
    type Raw = u32;
    type OrderingFloor = Relaxed;
    const POINTEE_ALIGNMENT: PointeeAlignment = PointeeAlignment::NONE;
}

/// Stored as its descriptor's complement, inside a [`ReprOf`], which only a `BorrowedFd` of the
/// same lifetime gives, so a decode hands out no descriptor that bits made, nor one that outlives
/// the borrow it came from. The complement of -1, no descriptor's, is zero, so an `Option` of one
/// is free.
// SAFETY: the repr is the complement of the descriptor, never -1, so within every repr but zero;
// it is sealed, so every repr the cell holds is zero or one a `BorrowedFd` of its lifetime gave, by
// `ReprOf`'s INVARIANT, and `from_repr` decodes each but zero as that descriptor, by the repr
// alone, as `TotalZeroNiche` says, and the default unchecked decode unwraps it; a descriptor may
// cross threads, as `BorrowedFd` is `Send` and `Sync`.
#[cfg(unix)]
#[doc(cfg(all(feature = "std", unix)))]
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
unsafe impl Atom for BorrowedFd<'_> {
    type Repr = ReprOf<Self>;
    type Validity = TotalZeroNiche;
    const REPRS: ReprRange<Self::Repr> = ReprRange::NON_ZERO;
    #[inline]
    fn to_repr(self) -> Self::Repr {
        // SAFETY: the raw repr `self` gives: its descriptor's complement.
        unsafe { ReprOf::new(!self.as_raw_fd().cast_unsigned()) }
    }
    #[inline]
    fn from_repr(repr: Self::Repr) -> Option<Self> {
        match repr.repr() {
            0 => None,
            // SAFETY: by `ReprOf`'s INVARIANT, a repr that is not zero is the complement of a
            // descriptor a `BorrowedFd` of this lifetime gave, which stays open for all of it.
            complement => Some(unsafe { Self::borrow_raw((!complement).cast_signed()) }),
        }
    }
}
