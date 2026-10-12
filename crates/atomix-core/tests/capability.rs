//! The read-modify-writes a capability allows, on the values that have it, at run time and in a
//! constant; and a raw pointer's bitwise forms, on a `usize` mask that keeps its provenance.

#![feature(const_trait_impl)]
// Loom's cells exist only inside a model; `model.rs` holds the loom tests.
#![cfg(not(loom))]

#[cfg(test)]
mod tests {
    use core::num::Wrapping;
    use core::ptr;

    use atomix_core::ordering::{AcqRel, Acquire, Relaxed, Release};
    use atomix_core::{
        AtomBitwise, Atomic, AtomicBool, AtomicI8, AtomicI16, AtomicI64, AtomicPtr, AtomicU8,
        AtomicU16, AtomicU32, AtomicU64,
    };

    /// Turns on `bits` in `atomic`, through the capability alone.
    fn set_bits<T: AtomBitwise>(atomic: &Atomic<T>, bits: T) {
        atomic.or(bits, Release);
    }

    #[test]
    fn add_and_sub_wrap() {
        let count = AtomicU8::new(255);
        assert_eq!(count.fetch_add(1, AcqRel), 255, "the value before");
        assert_eq!(count.load(Relaxed), 0, "wrapped");
        assert_eq!(count.fetch_sub(1, AcqRel), 0, "the value before");
        assert_eq!(count.load(Relaxed), 255, "wrapped back");
        count.fetch_add(2, Relaxed);
        assert_eq!(count.load(Relaxed), 1, "up two, wrapping");
        count.fetch_sub(3, Relaxed);
        assert_eq!(count.load(Relaxed), 254, "down three, wrapping");
    }

    #[test]
    fn signed_add_and_sub_wrap_at_the_signed_bounds() {
        let level = AtomicI8::new(i8::MAX);
        assert_eq!(level.fetch_add(1, AcqRel), i8::MAX, "the value before");
        assert_eq!(level.load(Relaxed), i8::MIN, "past the greatest, the least");
        level.fetch_sub(1, Relaxed);
        assert_eq!(level.load(Relaxed), i8::MAX, "below the least, the greatest");
        assert_eq!(level.fetch_sub(-2, AcqRel), i8::MAX, "the value before");
        assert_eq!(level.load(Relaxed), -127, "less -2 is 2 more, wrapping");
    }

    #[test]
    fn bitwise_operations_combine_bits() {
        let bits = AtomicU32::new(0b1100);
        bits.or(0b1010, AcqRel);
        assert_eq!(bits.load(Relaxed), 0b1110, "or keeps the bits either has");
        bits.and(0b0111, AcqRel);
        assert_eq!(bits.load(Relaxed), 0b0110, "and keeps the bits both have");
        bits.xor(0b0101, AcqRel);
        assert_eq!(bits.load(Relaxed), 0b0011, "xor keeps the bits one has");
        bits.not(AcqRel);
        assert_eq!(bits.load(Relaxed), !0b0011, "every bit inverted");
    }

    #[test]
    fn a_literal_operand_takes_the_atomics_own_type() {
        let flags = AtomicU64::new(0);
        flags.or(0b10, Release);
        assert_eq!(flags.load(Acquire), 0b10, "a `u64`");
        let byte = AtomicU8::new(0b01);
        byte.xor(0b11, Release);
        assert_eq!(byte.load(Acquire), 0b10, "a `u8`");
        let signed = AtomicI64::new(-1);
        signed.and(6, Release);
        assert_eq!(signed.load(Acquire), 6, "an `i64`");
        let flag = AtomicBool::new(false);
        flag.or(true, Release);
        assert!(flag.load(Acquire), "and a `bool`");
    }

    #[test]
    fn generic_code_over_the_capability_calls_or_with_no_other_bound() {
        let bits = AtomicU16::new(0b01);
        set_bits(&bits, 0b10);
        assert_eq!(bits.load(Acquire), 0b11, "the bits set");
    }

    #[test]
    fn a_raw_pointer_takes_a_mask_and_keeps_its_provenance() {
        let mut node = 7_u64;
        let pointer = ptr::from_mut(&mut node);
        let next = AtomicPtr::new(pointer);
        next.or(0b1, AcqRel);
        assert_eq!(next.load(Acquire).addr(), pointer.addr() | 1, "its low bit set");
        next.xor(0b11, AcqRel);
        assert_eq!(next.load(Acquire).addr(), pointer.addr() | 0b10, "flipped");
        next.and(!0b11, AcqRel);
        let cleared = next.load(Acquire);
        assert_eq!(cleared, pointer, "and cleared");
        // SAFETY: `cleared` is `pointer`, its address changed and changed back by masks that keep
        // its provenance over `node`, which is live.
        #[expect(unsafe_code, reason = "reads through it, so Miri checks its provenance")]
        let read = unsafe { cleared.read() };
        assert_eq!(read, 7, "it reads the node");
    }

    #[test]
    fn a_const_raw_pointer_takes_a_mask_too() {
        let node = 7_u64;
        let pointer = ptr::from_ref(&node);
        let next = Atomic::new(pointer);
        next.or(0b1, AcqRel);
        next.and(!0b1, AcqRel);
        let cleared = next.load(Acquire);
        // SAFETY: as above, over `node`, which is live and only read.
        #[expect(unsafe_code, reason = "reads through it, so Miri checks its provenance")]
        let read = unsafe { cleared.read() };
        assert_eq!(read, 7, "marked, unmarked, and it reads the node");
    }

    #[cfg(aarch64_code)]
    #[test]
    fn a_raw_pointers_fetch_forms_return_the_pointer_before() {
        let mut node = 7_u64;
        let pointer = ptr::from_mut(&mut node);
        let next = AtomicPtr::new(pointer);
        assert_eq!(next.fetch_or(0b1, AcqRel), pointer, "the pointer before the or");
        assert_eq!(next.fetch_xor(0b11, AcqRel).addr(), pointer.addr() | 1, "before the xor");
        let marked = next.fetch_and(!0b11, AcqRel);
        assert_eq!(marked.addr(), pointer.addr() | 0b10, "and before the and");
        // SAFETY: `marked` is `pointer` with bit 1 set by a mask that keeps its provenance; masked
        // off again, it reaches `node`, which is live.
        #[expect(unsafe_code, reason = "reads through it, so Miri checks its provenance")]
        let read = unsafe { marked.map_addr(|address| address & !0b11).read() };
        assert_eq!(read, 7, "whose provenance the masks kept");
    }

    #[test]
    fn a_bool_flips_and_combines() {
        let flag = AtomicBool::new(false);
        flag.not(AcqRel);
        assert!(flag.load(Relaxed), "not flips false");
        flag.or(false, AcqRel);
        assert!(flag.load(Relaxed), "or with false keeps true");
        flag.xor(true, AcqRel);
        assert!(!flag.load(Relaxed), "xor with true flips true");
        flag.and(true, AcqRel);
        assert!(!flag.load(Relaxed), "and with true keeps false");
    }

    #[cfg(aarch64_code)]
    #[test]
    fn max_and_min_follow_the_values_sign() {
        let signed = AtomicI64::new(-5);
        assert_eq!(signed.fetch_max(3, AcqRel), -5, "the value before");
        assert_eq!(signed.load(Relaxed), 3, "3 is the larger signed value");
        signed.fetch_min(-7, Relaxed);
        assert_eq!(signed.load(Relaxed), -7, "-7 is the smaller signed value");
        let unsigned = AtomicU64::new(7);
        assert_eq!(unsigned.fetch_min(2, AcqRel), 7, "the value before");
        assert_eq!(unsigned.load(Relaxed), 2, "2 is the smaller");
        unsigned.fetch_max(u64::MAX, Relaxed);
        assert_eq!(unsigned.load(Relaxed), u64::MAX, "the top bit set is the larger, unsigned");
        let narrow = AtomicI8::new(-1);
        narrow.fetch_max(1, Relaxed);
        assert_eq!(narrow.load(Relaxed), 1, "1 is the larger signed value; unsigned, -1 is 0xFF");
        assert_eq!(narrow.fetch_min(-1, AcqRel), 1, "the value before");
        assert_eq!(narrow.load(Relaxed), -1, "-1 is the smaller signed value");
    }

    #[cfg(aarch64_code)]
    #[test]
    fn the_fetch_forms_return_the_value_before() {
        let bits = AtomicU32::new(0b1100);
        assert_eq!(bits.fetch_or(0b1010, AcqRel), 0b1100, "before or");
        assert_eq!(bits.fetch_and(0b0111, AcqRel), 0b1110, "before and");
        assert_eq!(bits.fetch_xor(0b0101, AcqRel), 0b0110, "before xor");
        assert_eq!(bits.fetch_not(AcqRel), 0b0011, "before not");
        assert_eq!(bits.load(Relaxed), !0b0011, "after not");
        let flag = AtomicBool::new(false);
        assert!(!flag.fetch_or(true, AcqRel), "before or");
        assert!(flag.load(Relaxed), "after or");
    }

    #[test]
    fn a_bit_chosen_at_run_time_is_set_cleared_and_inverted_alone() {
        for bit in 0..u64::BITS {
            let mask = 1_u64 << bit;
            let bits = AtomicU64::new(0xA5A5_A5A5_A5A5_A5A5 & !mask);
            let others = bits.load(Relaxed);
            assert!(!bits.bit_set(bit, AcqRel), "{bit}: clear before the set");
            assert!(bits.bit_set(bit, AcqRel), "{bit}: set before the second set");
            assert_eq!(bits.load(Relaxed), others | mask, "{bit}: set, every other bit kept");
            assert!(bits.bit_toggle(bit, AcqRel), "{bit}: set before the toggle");
            assert!(!bits.bit_toggle(bit, AcqRel), "{bit}: clear before the toggle back");
            assert!(bits.bit_clear(bit, AcqRel), "{bit}: set before the clear");
            assert!(!bits.bit_clear(bit, AcqRel), "{bit}: clear before the second clear");
            assert_eq!(bits.load(Relaxed), others, "{bit}: clear again, every other bit kept");
        }
    }

    #[test]
    fn a_bit_counts_modulo_the_reprs_width() {
        let bits = AtomicU16::new(0);
        assert!(!bits.bit_set(16 + 3, AcqRel), "bit 19 of 16 is bit 3, clear");
        assert_eq!(bits.load(Relaxed), 0b1000, "bit 3 set");
        assert!(bits.bit_clear(u32::MAX - 12, AcqRel), "bit 2^32 - 13 of 16 is bit 3, set");
        assert_eq!(bits.load(Relaxed), 0, "bit 3 clear");
        assert!(!bits.bit_toggle(16, AcqRel), "bit 16 of 16, one past the top, is bit 0, clear");
        assert_eq!(bits.load(Relaxed), 0b1, "bit 0 set");
    }

    #[test]
    fn a_signed_values_top_bit_is_its_sign() {
        let level = AtomicI16::new(1);
        assert!(!level.bit_set(15, AcqRel), "a positive value's sign bit is clear");
        assert_eq!(level.load(Relaxed), i16::MIN + 1, "negative once set");
    }

    #[test]
    fn a_wrapping_value_takes_its_integers_bits() {
        let bits = Atomic::new(Wrapping(0_u32));
        assert!(!bits.bit_set(31, AcqRel), "clear before the set");
        assert_eq!(bits.load(Relaxed), Wrapping(1 << 31), "the top bit set");
    }

    // Add and subtract run in a constant.
    const _: () = {
        let count = AtomicU8::new(255);
        assert!(count.fetch_add(1, AcqRel) == 255, "the value before, in const");
        assert!(count.fetch_sub(2, AcqRel) == 0, "after a wrap to 0");
        assert!(count.load(Acquire) == 254, "and back past it");
    };

    // The bitwise operations that discard the value before run in a constant.
    const _: () = {
        let bits = AtomicU16::new(0b1100);
        bits.or(0b0011, Release);
        bits.and(0b0110, Release);
        bits.xor(0b0101, Release);
        bits.not(Release);
        assert!(bits.load(Acquire) == !0b0011, "each combined, in const");
    };

    // A bit's set, toggle and clear run in a constant.
    const _: () = {
        let busy = AtomicU64::new(0);
        assert!(!busy.bit_set(5, AcqRel), "worker 5 was idle, in const");
        assert!(busy.bit_toggle(5, AcqRel), "busy before the toggle");
        assert!(!busy.bit_clear(5, AcqRel), "and idle before the clear");
    };

    // On `aarch64` and `arm64ec`, the `fetch_` forms, maximum and minimum run in a constant.
    #[cfg(aarch64_code)]
    const _: () = {
        let low = AtomicI64::new(5);
        assert!(low.fetch_min(-3, AcqRel) == 5, "the value before, in const");
        assert!(low.fetch_max(1, AcqRel) == -3, "the minimum");
        let flags = AtomicU8::new(0b01);
        assert!(flags.fetch_or(0b10, AcqRel) == 0b01, "the flags before");
        assert!(flags.fetch_and(0b10, AcqRel) == 0b11, "both set");
        assert!(flags.fetch_xor(0b11, AcqRel) == 0b10, "one left");
        assert!(flags.fetch_not(AcqRel) == 0b01, "flipped");
        assert!(flags.load(Acquire) == !0b01, "and flipped again");
    };

    #[cfg(aarch64_code)]
    #[test]
    fn an_8_bit_value_has_its_bits_on_aarch64() {
        let bits = AtomicU8::new(0);
        assert!(!bits.bit_set(7, AcqRel), "clear before the set");
        assert_eq!(bits.load(Relaxed), 0x80, "the top bit set");
    }

    #[cfg(aarch64_code)]
    #[test]
    fn a_bools_one_bit_is_every_position_on_aarch64() {
        let flag = AtomicBool::new(false);
        assert!(!flag.bit_set(9, AcqRel), "clear before the set, at any position");
        assert!(flag.load(Relaxed), "and set after");
    }
}
