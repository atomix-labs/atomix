//! std's values of the system, with the `std` feature: a time stored as its signed seconds since
//! the Unix epoch above its nanoseconds, before the epoch too; an exit status as its raw status;
//! and a borrowed file descriptor sealed, so bits make none.

#![cfg(std_values)]
#![cfg(not(loom))]
#![cfg_attr(wide, feature(time_systemtime_limits))]

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests {
    #[cfg(wide)]
    use core::time::Duration;
    #[cfg(unix)]
    use std::os::fd::{AsRawFd, BorrowedFd};
    #[cfg(wait_status)]
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    #[cfg(wide)]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(any(unix, wide))]
    use atomix_core::ordering::AcqRel;
    #[cfg(wide)]
    use atomix_core::ordering::Acquire;
    #[cfg(unix)]
    use atomix_core::ordering::CoreRmwOrdering;
    use atomix_core::ordering::Relaxed;
    #[cfg(wait_status)]
    use atomix_core::validity::Total;
    #[cfg(unix)]
    use atomix_core::validity::TotalZeroNiche;
    use atomix_core::{Atom, Atomic};
    #[cfg(unix)]
    use atomix_core::{BitTest, Load, MaskBitwise, Primitive, ReprOf};
    #[cfg(wide)]
    use proptest::prelude::{Strategy, any};
    #[cfg(wide)]
    use proptest::proptest;

    use crate::testing::law::{assert_holds, decodes_as_promised};
    #[cfg(wide)]
    use crate::testing::law::{reprs_order_as_ord, round_trips};

    /// A time `seconds` and `nanos` from the epoch, before it where `seconds` is negative.
    #[cfg(wide)]
    fn time(seconds: i64, nanos: u32) -> SystemTime {
        let whole = Duration::from_secs(seconds.unsigned_abs());
        let moment =
            if seconds < 0 { UNIX_EPOCH.checked_sub(whole) } else { UNIX_EPOCH.checked_add(whole) };
        moment
            .and_then(|moment| moment.checked_add(Duration::from_nanos(nanos.into())))
            .expect("each test's time lies within a century of the epoch")
    }

    #[cfg(wide)]
    #[test]
    fn the_epoch_is_the_zero_repr() {
        assert_eq!(UNIX_EPOCH.to_repr(), 0, "no seconds, no nanoseconds");
    }

    #[cfg(wide)]
    #[test]
    fn a_time_holds_its_seconds_above_its_nanoseconds() {
        let repr = time(3, 700).to_repr();
        assert_eq!(repr, i128::from(3_i64) << 64 | i128::from(700_u32), "the seconds above");
    }

    #[cfg(wide)]
    #[test]
    fn a_time_before_the_epoch_rounds_its_seconds_down() {
        let repr = time(-2, 300).to_repr();
        assert_eq!(repr, i128::from(-2_i64) << 64 | i128::from(300_u32), "-2 seconds, 300 ns on");
        assert_eq!(SystemTime::from_repr(repr), Some(time(-2, 300)), "and back");
    }

    #[cfg(wide)]
    #[test]
    fn the_earliest_and_latest_times_round_trip() {
        assert_holds(round_trips(SystemTime::MIN));
        assert_holds(round_trips(SystemTime::MAX));
    }

    #[cfg(all(wide, windows))]
    #[test]
    fn a_time_refuses_nanoseconds_no_count_of_hundreds_makes() {
        assert_eq!(SystemTime::from_repr(150), None, "150 nanoseconds past the epoch");
    }

    #[cfg(wide)]
    #[test]
    fn a_time_refuses_a_second_of_nanoseconds() {
        assert_eq!(SystemTime::from_repr(1_000_000_000), None, "a billion nanoseconds");
    }

    #[cfg(wide)]
    #[test]
    fn an_atomic_time_moves_forward() {
        let deadline = Atomic::from(time(10, 0));
        let later = |moment: SystemTime| {
            moment
                .checked_add(Duration::from_secs(5))
                .expect("fifteen seconds past the epoch is a time the platform holds")
        };
        let before = deadline.update(AcqRel, Acquire, later);
        assert_eq!((before, deadline.load_rmw(Acquire)), (time(10, 0), time(15, 0)), "five on");
    }

    /// A time's repr: any seconds within a century either side of the epoch, and nanoseconds below
    /// a billion, in hundreds where the platform counts them so.
    #[cfg(wide)]
    fn time_reprs() -> impl Strategy<Value = i128> {
        const CENTURY: i64 = 3_155_760_000;
        (-CENTURY..CENTURY, 0..10_000_000_u32).prop_map(|(seconds, hundreds)| {
            let whole_hundreds = hundreds.strict_mul(100);
            let nanos = if cfg!(windows) {
                whole_hundreds
            } else {
                whole_hundreds.strict_add(hundreds % 100)
            };
            i128::from(seconds) << 64 | i128::from(nanos)
        })
    }

    #[cfg(wide)]
    proptest! {
        #[test]
        fn times_obey_the_laws(a in time_reprs(), b in time_reprs(), bits in any::<i128>()) {
            decodes_as_promised::<_, SystemTime>(bits)?;
            decodes_as_promised::<_, SystemTime>(a)?;
            let (a, b) = (SystemTime::from_repr(a), SystemTime::from_repr(b));
            if let (Some(a), Some(b)) = (a, b) {
                round_trips(a)?;
                round_trips(Some(a))?;
                reprs_order_as_ord(a, b)?;
            }
        }
    }

    #[cfg(wide)]
    #[test]
    fn none_is_no_time() {
        let none = <Option<SystemTime> as Atom>::to_repr(None);
        assert_eq!(SystemTime::from_repr(none), None, "a spare repr");
        assert_holds(round_trips(None::<SystemTime>));
    }

    #[cfg(wait_status)]
    #[test]
    fn every_raw_wait_status_decodes_and_round_trips() {
        for raw in [i32::MIN, -1, 0, 0x0100, i32::MAX] {
            assert_holds(decodes_as_promised::<Total, ExitStatus>(raw));
        }
    }

    #[cfg(wait_status)]
    #[test]
    fn an_exit_status_is_its_raw_status() {
        let status = ExitStatus::from_raw(0x0100);
        assert_eq!(status.to_repr(), 0x0100, "the raw wait status");
        let last = Atomic::from(status);
        last.store(ExitStatus::from_raw(0), Relaxed);
        assert!(last.load(Relaxed).success(), "exit code zero");
    }

    #[cfg(windows)]
    #[test]
    fn an_exit_status_is_its_code() {
        use std::os::windows::process::ExitStatusExt;

        let status = ExitStatus::from_raw(3);
        assert_eq!(status.to_repr(), 3, "the exit code");
        assert_eq!(Atomic::from(status).load(Relaxed).code(), Some(3), "loaded back");
    }

    /// Standard input's descriptor, borrowed for as long as the tests run.
    #[cfg(unix)]
    fn standard_input() -> BorrowedFd<'static> {
        // SAFETY: descriptor 0 stays open for the whole test process.
        #[expect(unsafe_code, reason = "borrows a descriptor the process keeps open")]
        unsafe {
            BorrowedFd::borrow_raw(0)
        }
    }

    /// Standard output's descriptor, borrowed as `standard_input` borrows its own.
    #[cfg(unix)]
    fn standard_output() -> BorrowedFd<'static> {
        // SAFETY: descriptor 1 stays open for the whole test process.
        #[expect(unsafe_code, reason = "borrows a descriptor the process keeps open")]
        unsafe {
            BorrowedFd::borrow_raw(1)
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_borrowed_descriptor_is_its_complement() {
        let repr = standard_output().to_repr();
        assert_eq!(repr.packed_bits(), u128::from(!1_u32), "descriptor 1's complement");
        assert_holds(decodes_as_promised::<TotalZeroNiche, BorrowedFd<'_>>(repr));
    }

    #[cfg(unix)]
    #[test]
    fn descriptor_zero_is_no_zero_repr() {
        let repr = standard_input().to_repr();
        assert!(!repr.is_bits(0), "zero's complement is all ones");
        assert_eq!(BorrowedFd::from_repr(repr).map(|fd| fd.as_raw_fd()), Some(0), "decodes back");
    }

    #[cfg(unix)]
    #[test]
    fn a_change_of_any_bit_makes_no_descriptor() {
        let repr = standard_output().to_repr();
        let changed = repr.with_packed_bits(repr.packed_bits() ^ 1 << 31);
        assert!(BorrowedFd::from_repr(changed).is_none(), "a bit changed makes null");
        assert!(
            BorrowedFd::from_repr(repr.clear_packed_bits(1 << 31)).is_none(),
            "and one cleared"
        );
    }

    /// The cell of `repr`, through the bound every repr has, as a caller of the hidden primitive
    /// operations reaches one.
    #[cfg(unix)]
    fn cell_of<R: Primitive>(repr: R) -> R::Cell {
        R::into_cell(repr)
    }

    #[cfg(unix)]
    #[test]
    fn a_mask_leaves_a_descriptor_as_it_was() {
        type DescriptorRepr = ReprOf<BorrowedFd<'static>>;
        let cell = cell_of(standard_output().to_repr());
        <DescriptorRepr as MaskBitwise>::fetch_or_mask(&cell, u32::MAX, AcqRel::CORE);
        <DescriptorRepr as MaskBitwise>::fetch_xor_mask(&cell, u32::MAX, AcqRel::CORE);
        let _ = <DescriptorRepr as BitTest>::test_and_toggle_bit(&cell, 0, AcqRel::CORE);
        let after = BorrowedFd::from_repr(<DescriptorRepr as Load>::load(&cell, Relaxed::CORE));
        assert_eq!(after.map(|fd| fd.as_raw_fd()), Some(1), "descriptor 1 still");
    }

    // A descriptor's repr crosses threads as the descriptor does.
    #[cfg(unix)]
    const _: () = {
        /// Compiles only where `T` may cross threads and be shared between them.
        const fn crosses_threads<T: Send + Sync>() {}
        crosses_threads::<ReprOf<BorrowedFd<'static>>>();
        crosses_threads::<Atomic<BorrowedFd<'static>>>();
    };

    #[cfg(unix)]
    #[test]
    fn bits_make_no_descriptor() {
        let forged: ReprOf<BorrowedFd<'static>> = Primitive::from_bits(!1);
        assert!(BorrowedFd::from_repr(forged).is_none(), "only a borrowed descriptor gives one");
    }

    #[cfg(unix)]
    #[test]
    fn an_atomic_descriptor_swaps() {
        let target = Atomic::from(standard_input());
        let before = target.swap(standard_output(), AcqRel);
        assert_eq!(before.as_raw_fd(), 0, "standard input before");
        assert_eq!(target.load(Relaxed).as_raw_fd(), 1, "standard output after");
    }

    #[cfg(unix)]
    #[test]
    fn none_is_no_descriptor() {
        assert!(<Option<BorrowedFd<'static>> as Atom>::to_repr(None).is_bits(0), "the zero repr");
    }
}
