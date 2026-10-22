//! Each probe's twin over core's atomics, named for it with `core_` before: core's operation of
//! the same name where core has one, and otherwise the bits of a field, a tag or an element masked
//! by hand, as code over core's atomics would, an element's after the check of its index a slice's
//! index makes. `tests/codegen.rs` holds each to its probe's code, instruction for instruction, or
//! pins where the two differ.
//!
//! A probe has no twin where core has no counterpart: a `char`, a ranged integer or a core value
//! decoded without a check, a value decoded from a word of several fields, an array of `bool`s
//! decoded by vectors, two pointers in 16 bytes, a reference to a slice or a `str`, a field's, a
//! tag's or an element's `update`, whose loop by hand has no one shape, or `fence(StoreStore)`; or
//! where atomix's code takes a step core's has no reason to: a tagged pointer's encode, which tests
//! its pointer's alignment.

// ORDERING: each twin's is its probe's, so the two compile alike; no thread runs a twin, and
// nothing pairs.

use core::hint::assert_unchecked;
#[cfg(all(target_has_atomic = "128", any(target_feature = "avx", target_feature = "lse2")))]
use core::net::Ipv6Addr;
use core::net::{Ipv4Addr, SocketAddrV4};
use core::num::NonZero;
use core::range::Range;
#[cfg(target_has_atomic = "128")]
use core::sync::atomic::AtomicU128;
use core::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release, SeqCst};
use core::sync::atomic::{
    AtomicBool, AtomicI32, AtomicI64, AtomicIsize, AtomicPtr, AtomicU16, AtomicU32, AtomicU64,
    compiler_fence, fence,
};
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
use core::sync::atomic::{AtomicI8, AtomicU8};

use arbitrary_int::u4;

use crate::pointer_words::{Block, Node};
use crate::references::Config;
use crate::tuples::Node as TupleNode;

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

// The values' twins: an array's, a reference's, a tuple's and core's small values'.

/// Panics as a slice's index does past its end, with std's message: the twins of a checked
/// element's operation call it off their fast path, as atomix's element calls its own.
#[cold]
#[inline(never)]
#[track_caller]
fn refuse_index(index: usize, length: usize) -> ! {
    panic!("index out of bounds: the len is {length} but the index is {index}")
}

/// `index`, once checked against `length` as a slice's index checks it.
#[track_caller]
fn checked(index: usize, length: usize) -> usize {
    if index >= length {
        refuse_index(index, length);
    }
    index
}

#[unsafe(no_mangle)]
pub fn core_u32_load(atomic: &AtomicU32) -> u32 {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn core_u32_store(atomic: &AtomicU32, value: u32) {
    atomic.store(value, Release);
}

#[unsafe(no_mangle)]
pub fn core_u32_swap(atomic: &AtomicU32, value: u32) -> u32 {
    atomic.swap(value, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_u32_compare_exchange(atomic: &AtomicU32, current: u32, new: u32) -> Result<u32, u32> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

#[unsafe(no_mangle)]
pub fn core_u32_or(atomic: &AtomicU32, value: u32) {
    atomic.fetch_or(value, Release);
}

#[unsafe(no_mangle)]
pub fn core_byte_load(atomic: &AtomicU32, index: usize) -> u8 {
    let shift = checked(index, 4).wrapping_mul(8);
    (u64::from(atomic.load(Acquire)) >> shift).truncate()
}

/// Loads the byte at `index`, unchecked.
///
/// # Safety
/// `index` lies below 4.
#[unsafe(no_mangle)]
pub unsafe fn core_byte_load_unchecked(atomic: &AtomicU32, index: usize) -> u8 {
    // SAFETY: the caller's index lies below 4, as this twin's contract says.
    unsafe { assert_unchecked(index < 4) };
    (u64::from(atomic.load(Acquire)) >> index.wrapping_mul(8)).truncate()
}

#[unsafe(no_mangle)]
pub fn core_byte_load_literal(atomic: &AtomicU32) -> u8 {
    (atomic.load(Acquire) >> 16).truncate()
}

#[unsafe(no_mangle)]
pub fn core_byte_or(atomic: &AtomicU32, index: usize, value: u8) {
    let shift = checked(index, 4).wrapping_mul(8);
    atomic.fetch_or((u64::from(value) << shift).truncate(), Release);
}

#[unsafe(no_mangle)]
pub fn core_byte_and(atomic: &AtomicU32, index: usize, value: u8) {
    let shift = checked(index, 4).wrapping_mul(8);
    atomic.fetch_and(!(u64::from(!value) << shift).truncate::<u32>(), Release);
}

#[unsafe(no_mangle)]
pub fn core_byte_not(atomic: &AtomicU32, index: usize) {
    let shift = checked(index, 4).wrapping_mul(8);
    atomic.fetch_xor((u64::from(u8::MAX) << shift).truncate(), Release);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_byte_fetch_and(atomic: &AtomicU32, index: usize, value: u8) -> u32 {
    let shift = checked(index, 4).wrapping_mul(8);
    atomic.fetch_and(!(u64::from(!value) << shift).truncate::<u32>(), AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_byte_fetch_not(atomic: &AtomicU32, index: usize) -> u32 {
    let shift = checked(index, 4).wrapping_mul(8);
    atomic.fetch_xor((u64::from(u8::MAX) << shift).truncate(), AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_byte_fetch_or(atomic: &AtomicU32, index: usize, value: u8) -> u32 {
    let shift = checked(index, 4).wrapping_mul(8);
    atomic.fetch_or((u64::from(value) << shift).truncate(), AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_flag_test_and_set(atomic: &AtomicU64, index: usize) -> bool {
    let bit = 1 << checked(index, 64);
    atomic.fetch_or(bit, AcqRel) & bit != 0
}

#[unsafe(no_mangle)]
pub fn core_flag_test_and_set_checked(atomic: &AtomicU64, index: usize) -> bool {
    index < 64 && {
        let bit = 1 << index;
        atomic.fetch_or(bit, AcqRel) & bit != 0
    }
}

#[unsafe(no_mangle)]
pub fn core_flag_place_test_and_set(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(1 << 5, AcqRel) & 1 << 5 != 0
}

#[unsafe(no_mangle)]
pub fn core_flag_set(atomic: &AtomicU64, index: usize) {
    atomic.fetch_or(1 << checked(index, 64), Release);
}

#[unsafe(no_mangle)]
pub fn core_flag_clear(atomic: &AtomicU64, index: usize) {
    atomic.fetch_and(!(1 << checked(index, 64)), Release);
}

#[unsafe(no_mangle)]
pub fn core_flag_load(atomic: &AtomicU64, index: usize) -> bool {
    let bit = 1 << checked(index, 64);
    atomic.load(Relaxed) & bit != 0
}

#[unsafe(no_mangle)]
pub fn core_grid_test_and_set(atomic: &AtomicU64, row: usize, column: usize) -> bool {
    let bit = 1 << (checked(row, 8).wrapping_mul(8) | checked(column, 8));
    atomic.fetch_or(bit, AcqRel) & bit != 0
}

#[unsafe(no_mangle)]
pub fn core_grid_place_test_and_set(atomic: &AtomicU64, row: usize) -> bool {
    let bit = 1 << (checked(row, 8).wrapping_mul(8) | 3);
    atomic.fetch_or(bit, AcqRel) & bit != 0
}

#[unsafe(no_mangle)]
pub fn core_halves_last_fetch_add(atomic: &AtomicU32) -> u32 {
    atomic.fetch_add(1 << 16, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_halves_last_fetch_add_discarded(atomic: &AtomicU32) {
    atomic.fetch_add(1 << 16, AcqRel);
}

#[unsafe(no_mangle)]
pub fn core_quarter_load(atomic: &AtomicU32, index: usize) -> u8 {
    let shift = checked(index, 16).wrapping_mul(2);
    (u64::from(atomic.load(Acquire)) >> shift & 0b11).truncate()
}

#[unsafe(no_mangle)]
pub fn core_quarter_place_load(atomic: &AtomicU32) -> u8 {
    (atomic.load(Acquire) >> 10 & 0b11).truncate()
}

#[unsafe(no_mangle)]
pub fn core_pointer_load(atomic: &AtomicPtr<Config>) -> *mut Config {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn core_pointer_load_relaxed(atomic: &AtomicPtr<Config>) -> *mut Config {
    atomic.load(Relaxed)
}

#[unsafe(no_mangle)]
pub fn core_pointer_store(atomic: &AtomicPtr<Config>, pointer: *mut Config) {
    atomic.store(pointer, Release);
}

/// `store_rmw` is a loop of compare-exchanges, whose reads feed the comparison alone.
#[unsafe(no_mangle)]
pub fn core_pointer_store_rmw(atomic: &AtomicPtr<Config>, pointer: *mut Config) {
    let mut current = atomic.load(Relaxed);
    while let Err(found) = atomic.compare_exchange_weak(current, pointer, Release, Relaxed) {
        current = found;
    }
}

#[unsafe(no_mangle)]
pub fn core_pointer_swap(atomic: &AtomicPtr<Config>, pointer: *mut Config) -> *mut Config {
    atomic.swap(pointer, AcqRel)
}

#[unsafe(no_mangle)]
pub fn core_pointer_compare_exchange(
    atomic: &AtomicPtr<Config>, current: *mut Config, new: *mut Config,
) -> Result<*mut Config, *mut Config> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

#[unsafe(no_mangle)]
pub fn core_halves_or(atomic: &AtomicU64, value: (u32, u32)) {
    atomic.fetch_or(u64::from(value.0) | u64::from(value.1) << 32, Release);
}

#[unsafe(no_mangle)]
pub fn core_halves_store(atomic: &AtomicU64, value: (u32, u32)) {
    atomic.store(u64::from(value.0) | u64::from(value.1) << 32, Release);
}

#[unsafe(no_mangle)]
pub fn core_halves_load_second(atomic: &AtomicU64) -> u32 {
    (atomic.load(Acquire) >> 32).truncate()
}

#[unsafe(no_mangle)]
pub fn core_counted_bytes_fetch_add(atomic: &AtomicU32) -> (u8, u8, u16) {
    let bits = atomic.fetch_add(1 << 16, AcqRel);
    (bits.truncate(), (bits >> 8).truncate(), (bits >> 16).truncate())
}

#[unsafe(no_mangle)]
pub fn core_live_quote_test_and_set(atomic: &AtomicU64) -> bool {
    atomic.fetch_or(1 << 32, AcqRel) & 1 << 32 != 0
}

#[unsafe(no_mangle)]
pub fn core_slot_test_and_set_written(atomic: &AtomicU64, index: usize) -> bool {
    let bit = 1 << checked(index, 4).wrapping_mul(16);
    atomic.fetch_or(bit, AcqRel) & bit != 0
}

#[unsafe(no_mangle)]
pub fn core_marked_pointer_load_flag(atomic: &AtomicPtr<TupleNode>) -> bool {
    atomic.load(Relaxed).addr() & 1 != 0
}

#[unsafe(no_mangle)]
pub fn core_marked_pointer_set_flag(atomic: &AtomicPtr<TupleNode>) {
    atomic.fetch_or(1, Relaxed);
}

/// `load_rmw` is a compare-exchange of zero for zero, which writes only what it found.
#[cfg(target_has_atomic = "128")]
#[unsafe(no_mangle)]
pub fn core_counted_pointer_load_count(atomic: &AtomicU128) -> u64 {
    (atomic.compare_exchange(0, 0, Relaxed, Relaxed).unwrap_or_else(|found| found) >> 64).truncate()
}

/// `load_rmw` is a compare-exchange of zero for zero, which writes only what it found.
#[cfg(target_has_atomic = "128")]
#[unsafe(no_mangle)]
pub fn core_armed_load_flag(atomic: &AtomicU128) -> bool {
    atomic.compare_exchange(0, 0, Relaxed, Relaxed).unwrap_or_else(|found| found) >> 64 & 1 != 0
}

#[unsafe(no_mangle)]
pub fn core_ipv4_load(atomic: &AtomicU32) -> Ipv4Addr {
    Ipv4Addr::from_octets(atomic.load(Acquire).to_ne_bytes())
}

#[unsafe(no_mangle)]
pub fn core_ipv4_and(atomic: &AtomicU32, mask: Ipv4Addr) {
    atomic.fetch_and(u32::from_ne_bytes(mask.octets()), AcqRel);
}

#[unsafe(no_mangle)]
pub fn core_socket_address_load(atomic: &AtomicU64) -> SocketAddrV4 {
    let bits = atomic.load(Acquire);
    let address: u32 = bits.truncate();
    SocketAddrV4::new(Ipv4Addr::from_octets(address.to_ne_bytes()), (bits >> 32).truncate())
}

/// An `Ordering`'s `i8`, as its documented `repr(i8)` lays it out.
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec"))]
#[unsafe(no_mangle)]
pub fn core_cmp_ordering_fetch_max(atomic: &AtomicI8, value: i8) -> i8 {
    atomic.fetch_max(value, AcqRel)
}

#[cfg(all(target_has_atomic = "128", any(target_feature = "avx", target_feature = "lse2")))]
#[unsafe(no_mangle)]
pub fn core_ipv6_load(atomic: &AtomicU128) -> Ipv6Addr {
    Ipv6Addr::from_octets(atomic.load(Acquire).to_ne_bytes())
}

#[unsafe(no_mangle)]
pub fn core_result_is_ok(atomic: &AtomicU64) -> bool {
    atomic.load(Acquire) >> 32 == 0
}

#[unsafe(no_mangle)]
pub fn core_range_load(atomic: &AtomicU64) -> Range<u32> {
    let bits = atomic.load(Acquire);
    Range { start: bits.truncate(), end: (bits >> 32).truncate() }
}
