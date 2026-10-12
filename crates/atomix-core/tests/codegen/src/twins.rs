//! Each probe's twin over core's atomics, named for it with `core_` before, but a byte's bitwise
//! operations', which `arrays::by_hand` holds beside their probes: core's operation of the same
//! name where core has one, and otherwise the bits of a field or a tag masked by hand, as code over
//! core's atomics would. `tests/codegen.rs` holds each to its probe's code, instruction for
//! instruction, or pins where the two differ.
//!
//! A probe has no twin where core has no counterpart: a `char` or a ranged integer decoded without
//! a check, a value decoded from a word of several fields, two pointers in 16 bytes, or
//! `fence(StoreStore)`; or where atomix's code takes a step core's has no reason to: a tagged
//! pointer's encode, which tests its pointer's alignment.

// ORDERING: each twin's is its probe's, so the two compile alike; no thread runs a twin, and
// nothing pairs.

use core::num::NonZero;
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
use core::sync::atomic::AtomicU8;
#[cfg(target_has_atomic = "128")]
use core::sync::atomic::AtomicU128;
use core::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release, SeqCst};
use core::sync::atomic::{
    AtomicBool, AtomicI32, AtomicI64, AtomicIsize, AtomicPtr, AtomicU16, AtomicU32, AtomicU64,
    compiler_fence, fence,
};

use arbitrary_int::u4;

use crate::pointer_words::{Block, Node};

/// `Order`'s `live`, bit 40: above 32 bits of quantity and 8 of owner.
const LIVE: u64 = 1 << 40;

/// Where `Order`'s flags start, above `live`.
const FLAGS: u32 = 41;

/// Where `Counted`'s count starts: the top half.
const COUNT: u32 = 32;

/// Where `Task`'s count of references starts: above three bits of state.
const REFERENCES: u32 = 3;

/// `Outer`'s `inner.ready`, bit 8: above a byte.
const INNER_READY: u32 = 1 << 8;

/// Where `Outer`'s `inner.flags` start, above `inner.ready`.
const INNER_FLAGS: u32 = 9;

#[unsafe(no_mangle)]
pub fn core_u64_load(atomic: &AtomicU64) -> u64 {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn core_u64_store(atomic: &AtomicU64, value: u64) {
    atomic.store(value, Release);
}

#[unsafe(no_mangle)]
pub fn core_u64_swap(atomic: &AtomicU64, value: u64) -> u64 {
    atomic.swap(value, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_u64_fetch_add(atomic: &AtomicU64, delta: u64) -> u64 {
    atomic.fetch_add(delta, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_u64_fetch_sub(atomic: &AtomicU64, delta: u64) -> u64 {
    atomic.fetch_sub(delta, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_u64_compare_exchange(atomic: &AtomicU64, current: u64, new: u64) -> Result<u64, u64> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

#[unsafe(no_mangle)]
pub fn core_u64_fetch_add_discarded(atomic: &AtomicU64, delta: u64) {
    atomic.fetch_add(delta, Relaxed);
}

#[unsafe(no_mangle)]
pub fn core_u64_fetch_sub_discarded(atomic: &AtomicU64, delta: u64) {
    atomic.fetch_sub(delta, Relaxed);
}

#[unsafe(no_mangle)]
pub fn core_u64_or(atomic: &AtomicU64, value: u64) {
    atomic.fetch_or(value, Release);
}

#[unsafe(no_mangle)]
pub fn core_u64_and(atomic: &AtomicU64, value: u64) {
    atomic.fetch_and(value, Release);
}

#[unsafe(no_mangle)]
pub fn core_u64_xor(atomic: &AtomicU64, value: u64) {
    atomic.fetch_xor(value, Release);
}

#[unsafe(no_mangle)]
pub fn core_u64_not(atomic: &AtomicU64) {
    atomic.fetch_xor(u64::MAX, Release);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_u64_fetch_and(atomic: &AtomicU64, value: u64) -> u64 {
    atomic.fetch_and(value, AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_u64_fetch_or(atomic: &AtomicU64, value: u64) -> u64 {
    atomic.fetch_or(value, AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_u64_fetch_xor(atomic: &AtomicU64, value: u64) -> u64 {
    atomic.fetch_xor(value, AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_u64_fetch_not(atomic: &AtomicU64) -> u64 {
    atomic.fetch_xor(u64::MAX, AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_u64_fetch_max_discarded(atomic: &AtomicU64, value: u64) {
    atomic.fetch_max(value, Relaxed);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_u64_fetch_min_discarded(atomic: &AtomicU64, value: u64) {
    atomic.fetch_min(value, Relaxed);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_u64_fetch_max(atomic: &AtomicU64, value: u64) -> u64 {
    atomic.fetch_max(value, AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_i64_fetch_max_discarded(atomic: &AtomicI64, value: i64) {
    atomic.fetch_max(value, Relaxed);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_i64_fetch_min(atomic: &AtomicI64, value: i64) -> i64 {
    atomic.fetch_min(value, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_bool_or(atomic: &AtomicBool, value: bool) {
    atomic.fetch_or(value, Release);
}

#[unsafe(no_mangle)]
pub fn core_pointer_fetch_byte_add_discarded(atomic: &AtomicPtr<u64>, bytes: usize) {
    atomic.fetch_byte_add(bytes, Relaxed);
}

#[unsafe(no_mangle)]
pub fn core_pointer_fetch_ptr_sub(atomic: &AtomicPtr<u64>, count: usize) -> *mut u64 {
    atomic.fetch_ptr_sub(count, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_pointer_or(atomic: &AtomicPtr<u64>, mask: usize) {
    atomic.fetch_or(mask, Release);
}

#[unsafe(no_mangle)]
pub fn core_pointer_and(atomic: &AtomicPtr<u64>, mask: usize) {
    atomic.fetch_and(mask, Release);
}

#[unsafe(no_mangle)]
pub fn core_pointer_xor(atomic: &AtomicPtr<u64>, mask: usize) {
    atomic.fetch_xor(mask, Release);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_pointer_fetch_or(atomic: &AtomicPtr<u64>, mask: usize) -> *mut u64 {
    atomic.fetch_or(mask, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_option_load(atomic: &AtomicU64) -> Option<NonZero<u64>> {
    NonZero::new(atomic.load(Acquire))
}

#[cfg(all(target_has_atomic = "128", any(target_feature = "avx", target_feature = "lse2")))]
#[unsafe(no_mangle)]
pub fn core_u128_load(atomic: &AtomicU128) -> u128 {
    atomic.load(Acquire)
}

#[cfg(all(target_has_atomic = "128", any(target_feature = "avx", target_feature = "lse2")))]
#[unsafe(no_mangle)]
pub fn core_u128_load_relaxed(atomic: &AtomicU128) -> u128 {
    atomic.load(Relaxed)
}

#[cfg(all(target_has_atomic = "128", any(target_feature = "avx", target_feature = "lse2")))]
#[unsafe(no_mangle)]
pub fn core_u128_load_seq_cst(atomic: &AtomicU128) -> u128 {
    atomic.load(SeqCst)
}

#[cfg(all(target_has_atomic = "128", any(target_feature = "avx", target_feature = "lse2")))]
#[unsafe(no_mangle)]
pub fn core_u128_store(atomic: &AtomicU128, value: u128) {
    atomic.store(value, Release);
}

#[cfg(all(target_has_atomic = "128", any(target_feature = "avx", target_feature = "lse2")))]
#[unsafe(no_mangle)]
pub fn core_u128_store_seq_cst(atomic: &AtomicU128, value: u128) {
    atomic.store(value, SeqCst);
}

#[cfg(target_has_atomic = "128")]
#[unsafe(no_mangle)]
pub fn core_u128_compare_exchange(
    atomic: &AtomicU128, current: u128, new: u128,
) -> Result<u128, u128> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

/// `load_rmw` is a compare-exchange of zero for zero, which writes only what it found.
#[cfg(target_has_atomic = "128")]
#[unsafe(no_mangle)]
pub fn core_u128_load_rmw(atomic: &AtomicU128) -> u128 {
    atomic.compare_exchange(0, 0, Acquire, Acquire).unwrap_or_else(|found| found)
}

#[cfg(target_has_atomic = "128")]
#[unsafe(no_mangle)]
pub fn core_u128_update(atomic: &AtomicU128) -> u128 {
    atomic.update(AcqRel, Acquire, |value| value.wrapping_add(1))
}

#[unsafe(no_mangle)]
pub fn core_seq_cst_compiler_fence() {
    compiler_fence(SeqCst);
}

#[unsafe(no_mangle)]
pub fn core_acquire_fence() {
    fence(Acquire);
}

#[unsafe(no_mangle)]
pub fn core_release_fence() {
    fence(Release);
}

#[unsafe(no_mangle)]
pub fn core_seq_cst_fence() {
    fence(SeqCst);
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set(atomic: &AtomicU64, bit: u32) -> bool {
    let mask = 1 << (bit & 63);
    atomic.fetch_or(mask, AcqRel) & mask != 0
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_clear(atomic: &AtomicU64, bit: u32) -> bool {
    let mask = 1 << (bit & 63);
    atomic.fetch_and(!mask, AcqRel) & mask != 0
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_toggle(atomic: &AtomicU64, bit: u32) -> bool {
    let mask = 1 << (bit & 63);
    atomic.fetch_xor(mask, AcqRel) & mask != 0
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set_0(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(1, AcqRel) & 1 != 0
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set_5(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(1 << 5, AcqRel) & 1 << 5 != 0
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set_63(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(1 << 63, AcqRel) & 1 << 63 != 0
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_clear_63(atomic: &AtomicU64) -> bool {
    atomic.fetch_and(!(1 << 63), AcqRel) & 1 << 63 != 0
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set_64(atomic: &AtomicU64) -> bool {
    // The bit is taken modulo the width, as `bit_set` takes it: 64 is bit 0.
    atomic.fetch_or(1, AcqRel) & 1 != 0
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set_discarded(atomic: &AtomicU64, bit: u32) {
    atomic.fetch_or(1 << (bit & 63), AcqRel);
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set_in_some(atomic: &AtomicU64, bit: u32) -> Option<bool> {
    let mask = 1 << (bit & 63);
    Some(atomic.fetch_or(mask, AcqRel) & mask != 0)
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_clear_in_some(atomic: &AtomicU64, bit: u32) -> Option<bool> {
    let mask = 1 << (bit & 63);
    Some(atomic.fetch_and(!mask, AcqRel) & mask != 0)
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_toggle_in_some(atomic: &AtomicU64, bit: u32) -> Option<bool> {
    let mask = 1 << (bit & 63);
    Some(atomic.fetch_xor(mask, AcqRel) & mask != 0)
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set_63_in_some(atomic: &AtomicU64) -> Option<bool> {
    Some(atomic.fetch_or(1 << 63, AcqRel) & 1 << 63 != 0)
}

#[unsafe(no_mangle)]
pub fn core_u64_bit_set_through_map(atomic: &AtomicU64, bit: Option<u32>) -> Option<bool> {
    bit.map(|bit| {
        let mask = 1 << (bit & 63);
        atomic.fetch_or(mask, AcqRel) & mask != 0
    })
}

#[unsafe(no_mangle)]
pub fn core_u32_bit_set(atomic: &AtomicU32, bit: u32) -> bool {
    let mask = 1 << (bit & 31);
    atomic.fetch_or(mask, AcqRel) & mask != 0
}

#[unsafe(no_mangle)]
pub fn core_u32_bit_set_31(atomic: &AtomicU32) -> bool {
    atomic.fetch_or(1 << 31, AcqRel) & 1 << 31 != 0
}

#[unsafe(no_mangle)]
pub fn core_u16_bit_set(atomic: &AtomicU16, bit: u32) -> bool {
    let mask = 1 << (bit & 15);
    atomic.fetch_or(mask, AcqRel) & mask != 0
}

#[unsafe(no_mangle)]
pub fn core_u16_bit_set_in_some(atomic: &AtomicU16, bit: u32) -> Option<bool> {
    let mask = 1 << (bit & 15);
    Some(atomic.fetch_or(mask, AcqRel) & mask != 0)
}

#[unsafe(no_mangle)]
pub fn core_i32_bit_set(atomic: &AtomicI32, bit: u32) -> bool {
    let mask: i32 = 1 << (bit & 31);
    atomic.fetch_or(mask, AcqRel) & mask != 0
}

#[unsafe(no_mangle)]
pub fn core_i64_bit_set(atomic: &AtomicI64, bit: u32) -> bool {
    let mask: i64 = 1 << (bit & 63);
    atomic.fetch_or(mask, AcqRel) & mask != 0
}

#[unsafe(no_mangle)]
pub fn core_isize_bit_set(atomic: &AtomicIsize, bit: u32) -> bool {
    let mask: isize = 1 << (bit & 63);
    atomic.fetch_or(mask, AcqRel) & mask != 0
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_u8_bit_set(atomic: &AtomicU8, bit: u32) -> bool {
    let mask = 1 << (bit & 7);
    atomic.fetch_or(mask, AcqRel) & mask != 0
}

#[unsafe(no_mangle)]
pub fn core_field_set(atomic: &AtomicU64) {
    atomic.fetch_or(LIVE, Release);
}

#[unsafe(no_mangle)]
pub fn core_field_clear(atomic: &AtomicU64) {
    atomic.fetch_and(!LIVE, Release);
}

#[unsafe(no_mangle)]
pub fn core_field_toggle(atomic: &AtomicU64) {
    atomic.fetch_xor(LIVE, Release);
}

#[unsafe(no_mangle)]
pub fn core_field_store(atomic: &AtomicU64, value: bool) {
    if value {
        atomic.fetch_or(LIVE, Release);
    } else {
        atomic.fetch_and(!LIVE, Release);
    }
}

#[unsafe(no_mangle)]
pub fn core_field_test_and_set(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(LIVE, AcqRel) & LIVE != 0
}

#[unsafe(no_mangle)]
pub fn core_field_test_and_set_in_some(atomic: &AtomicU64) -> Option<bool> {
    Some(atomic.fetch_or(LIVE, AcqRel) & LIVE != 0)
}

#[unsafe(no_mangle)]
pub fn core_field_test_and_set_through_map(atomic: Option<&AtomicU64>) -> Option<bool> {
    atomic.map(|atomic| atomic.fetch_or(LIVE, AcqRel) & LIVE != 0)
}

#[unsafe(no_mangle)]
pub fn core_field_load(atomic: &AtomicU64) -> bool {
    atomic.load(Acquire) & LIVE != 0
}

#[unsafe(no_mangle)]
pub fn core_flags_load(atomic: &AtomicU64) -> u8 {
    (atomic.load(Acquire) >> FLAGS).truncate()
}

#[unsafe(no_mangle)]
pub fn core_quantity_load(atomic: &AtomicU64) -> u32 {
    atomic.load(Acquire).truncate()
}

#[unsafe(no_mangle)]
pub fn core_flags_or(atomic: &AtomicU64, flags: u8) {
    atomic.fetch_or(u64::from(flags) << FLAGS, Release);
}

#[unsafe(no_mangle)]
pub fn core_flags_and(atomic: &AtomicU64, flags: u8) {
    atomic.fetch_and(u64::from(flags) << FLAGS | !(u64::from(u8::MAX) << FLAGS), Release);
}

#[unsafe(no_mangle)]
pub fn core_flags_xor(atomic: &AtomicU64, flags: u8) {
    atomic.fetch_xor(u64::from(flags) << FLAGS, Release);
}

#[unsafe(no_mangle)]
pub fn core_flags_not(atomic: &AtomicU64) {
    atomic.fetch_xor(u64::from(u8::MAX) << FLAGS, Release);
}

#[unsafe(no_mangle)]
pub fn core_flags_or_constant(atomic: &AtomicU64) {
    atomic.fetch_or(0b100 << FLAGS, Release);
}

#[unsafe(no_mangle)]
pub fn core_nested_clear(atomic: &AtomicU32) {
    atomic.fetch_and(!INNER_READY, Release);
}

#[unsafe(no_mangle)]
pub fn core_nested_flags_or(atomic: &AtomicU32, flags: u8) {
    atomic.fetch_or(u32::from(flags) << INNER_FLAGS, Release);
}

#[unsafe(no_mangle)]
pub fn core_top_fetch_add_discarded(atomic: &AtomicU64, delta: u32) {
    atomic.fetch_add(u64::from(delta) << COUNT, Relaxed);
}

#[unsafe(no_mangle)]
pub fn core_top_fetch_add_count(atomic: &AtomicU64) -> u32 {
    (atomic.fetch_add(1 << COUNT, AcqRel) >> COUNT).truncate()
}

#[unsafe(no_mangle)]
pub fn core_u61_top_fetch_add_references(atomic: &AtomicU64) -> u64 {
    atomic.fetch_add(1 << REFERENCES, AcqRel) >> REFERENCES
}

#[unsafe(no_mangle)]
pub fn core_u4_flags_or(atomic: &AtomicU16, flags: u4) {
    atomic.fetch_or(u16::from(flags.value()) & 0xF, Release);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_ends8_low_test_and_set(atomic: &AtomicU8) -> bool {
    atomic.fetch_or(1, AcqRel) & 1 != 0
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_ends8_top_test_and_set(atomic: &AtomicU8) -> bool {
    atomic.fetch_or(1 << 7, AcqRel) & 1 << 7 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends16_low_test_and_set(atomic: &AtomicU16) -> bool {
    atomic.fetch_or(1, AcqRel) & 1 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends16_middle_test_and_set(atomic: &AtomicU16) -> bool {
    atomic.fetch_or(1 << 8, AcqRel) & 1 << 8 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends16_top_test_and_set(atomic: &AtomicU16) -> bool {
    atomic.fetch_or(1 << 15, AcqRel) & 1 << 15 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends32_low_test_and_set(atomic: &AtomicU32) -> bool {
    atomic.fetch_or(1, AcqRel) & 1 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends32_middle_test_and_set(atomic: &AtomicU32) -> bool {
    atomic.fetch_or(1 << 16, AcqRel) & 1 << 16 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends32_top_test_and_set(atomic: &AtomicU32) -> bool {
    atomic.fetch_or(1 << 31, AcqRel) & 1 << 31 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends64_low_test_and_set(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(1, AcqRel) & 1 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends64_middle_test_and_set(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(1 << 32, AcqRel) & 1 << 32 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends64_middle_test_and_clear(atomic: &AtomicU64) -> bool {
    atomic.fetch_and(!(1 << 32), AcqRel) & 1 << 32 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends64_middle_test_and_toggle(atomic: &AtomicU64) -> bool {
    atomic.fetch_xor(1 << 32, AcqRel) & 1 << 32 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends64_top_test_and_set(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(1 << 63, AcqRel) & 1 << 63 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends64_top_test_and_clear(atomic: &AtomicU64) -> bool {
    atomic.fetch_and(!(1 << 63), AcqRel) & 1 << 63 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends64_top_test_and_toggle(atomic: &AtomicU64) -> bool {
    atomic.fetch_xor(1 << 63, AcqRel) & 1 << 63 != 0
}

#[unsafe(no_mangle)]
pub fn core_ends64_low_test_and_clear_in_some(atomic: &AtomicU64) -> Option<bool> {
    Some(atomic.fetch_and(!1, AcqRel) & 1 != 0)
}

#[unsafe(no_mangle)]
pub fn core_ends64_top_test_and_toggle_in_some(atomic: &AtomicU64) -> Option<bool> {
    Some(atomic.fetch_xor(1 << 63, AcqRel) & 1 << 63 != 0)
}

#[unsafe(no_mangle)]
pub fn core_word_load_top(atomic: &AtomicPtr<Node>) -> *mut Node {
    atomic.load(Acquire).map_addr(|address| address & !0b111)
}

#[unsafe(no_mangle)]
pub fn core_tag_set(atomic: &AtomicPtr<Node>) {
    atomic.fetch_or(1, Release);
}

#[unsafe(no_mangle)]
pub fn core_tag_clear(atomic: &AtomicPtr<Node>) {
    atomic.fetch_and(!1, Release);
}

#[unsafe(no_mangle)]
pub fn core_tag_toggle(atomic: &AtomicPtr<Node>) {
    atomic.fetch_xor(1, Release);
}

#[unsafe(no_mangle)]
pub fn core_tag_load(atomic: &AtomicPtr<Node>) -> u8 {
    (atomic.load(Acquire).addr() & 0b11).truncate()
}

#[unsafe(no_mangle)]
pub fn core_bit0_test_and_set(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_or(0b1, AcqRel).addr() & 0b1 != 0
}

#[unsafe(no_mangle)]
pub fn core_bit0_test_and_set_in_some(atomic: &AtomicPtr<Node>) -> Option<bool> {
    Some(atomic.fetch_or(0b1, AcqRel).addr() & 0b1 != 0)
}

#[unsafe(no_mangle)]
pub fn core_bit0_test_and_clear(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_and(!0b1, AcqRel).addr() & 0b1 != 0
}

#[unsafe(no_mangle)]
pub fn core_bit0_test_and_toggle(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_xor(0b1, AcqRel).addr() & 0b1 != 0
}

#[unsafe(no_mangle)]
pub fn core_bit1_test_and_set(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_or(0b10, AcqRel).addr() & 0b10 != 0
}

#[unsafe(no_mangle)]
pub fn core_bit1_test_and_clear(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_and(!0b10, AcqRel).addr() & 0b10 != 0
}

#[unsafe(no_mangle)]
pub fn core_bit1_test_and_toggle(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_xor(0b10, AcqRel).addr() & 0b10 != 0
}

#[unsafe(no_mangle)]
pub fn core_bit2_test_and_set(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_or(0b100, AcqRel).addr() & 0b100 != 0
}

#[unsafe(no_mangle)]
pub fn core_bit2_test_and_clear(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_and(!0b100, AcqRel).addr() & 0b100 != 0
}

#[unsafe(no_mangle)]
pub fn core_bit2_test_and_toggle(atomic: &AtomicPtr<Node>) -> bool {
    atomic.fetch_xor(0b100, AcqRel).addr() & 0b100 != 0
}

#[unsafe(no_mangle)]
pub fn core_tag_flags_or(atomic: &AtomicPtr<Block>, flags: u4) {
    atomic.fetch_or(usize::from(flags.value()) << 1 & 0b1_1110, Release);
}

#[unsafe(no_mangle)]
pub fn core_tag_flags_and(atomic: &AtomicPtr<Block>, flags: u4) {
    atomic.fetch_and(usize::from(flags.value()) << 1 | !0b1_1110, Release);
}

#[unsafe(no_mangle)]
pub fn core_tag_flags_xor(atomic: &AtomicPtr<Block>, flags: u4) {
    atomic.fetch_xor(usize::from(flags.value()) << 1 & 0b1_1110, Release);
}

#[unsafe(no_mangle)]
pub fn core_tag_flags_not(atomic: &AtomicPtr<Block>) {
    atomic.fetch_xor(0b1_1110, Release);
}

#[unsafe(no_mangle)]
pub fn core_nested_tag_set(atomic: &AtomicPtr<Node>) {
    atomic.fetch_or(1, Release);
}
