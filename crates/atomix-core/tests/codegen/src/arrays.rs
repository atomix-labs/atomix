//! An array's operations, each its repr's, beside an integer's of the same width: four bytes are a
//! `u32`, sixty-four `bool`s a `u64`; and its elements', at an index chosen at run time, checked,
//! unchecked or a constant, each the instruction a field's is.

use atomix_core::ordering::{AcqRel, Acquire, Relaxed, Release};
use atomix_core::{Atomic, AtomicU32};

#[unsafe(no_mangle)]
pub fn u32_load(atomic: &AtomicU32) -> u32 {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn u32_store(atomic: &AtomicU32, value: u32) {
    atomic.store(value, Release);
}

#[unsafe(no_mangle)]
pub fn u32_swap(atomic: &AtomicU32, value: u32) -> u32 {
    atomic.swap(value, AcqRel)
}

#[unsafe(no_mangle)]
pub fn u32_compare_exchange(atomic: &AtomicU32, current: u32, new: u32) -> Result<u32, u32> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

#[unsafe(no_mangle)]
pub fn u32_or(atomic: &AtomicU32, value: u32) {
    atomic.or(value, Release);
}

#[unsafe(no_mangle)]
pub fn bytes_load(atomic: &Atomic<[u8; 4]>) -> [u8; 4] {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn bytes_store(atomic: &Atomic<[u8; 4]>, value: [u8; 4]) {
    atomic.store(value, Release);
}

#[unsafe(no_mangle)]
pub fn bytes_swap(atomic: &Atomic<[u8; 4]>, value: [u8; 4]) -> [u8; 4] {
    atomic.swap(value, AcqRel)
}

#[unsafe(no_mangle)]
pub fn bytes_compare_exchange(
    atomic: &Atomic<[u8; 4]>, current: [u8; 4], new: [u8; 4],
) -> Result<[u8; 4], [u8; 4]> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

#[unsafe(no_mangle)]
pub fn bytes_or(atomic: &Atomic<[u8; 4]>, value: [u8; 4]) {
    atomic.or(value, Release);
}

#[unsafe(no_mangle)]
pub fn byte_load(atomic: &Atomic<[u8; 4]>, index: usize) -> u8 {
    atomic.element(index).load(Acquire)
}

/// Loads the byte at `index`, unchecked.
///
/// # Safety
/// `index` lies below 4.
#[unsafe(no_mangle)]
pub unsafe fn byte_load_unchecked(atomic: &Atomic<[u8; 4]>, index: usize) -> u8 {
    // SAFETY: the caller's index lies below 4, as this probe's contract says.
    unsafe { atomic.element_unchecked(index) }.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn byte_load_literal(atomic: &Atomic<[u8; 4]>) -> u8 {
    atomic.element(2).load(Acquire)
}

#[unsafe(no_mangle)]
pub fn byte_place_load(atomic: &Atomic<[u8; 4]>) -> u8 {
    atomic.place::<2>().load(Acquire)
}

#[unsafe(no_mangle)]
pub fn byte_or(atomic: &Atomic<[u8; 4]>, index: usize, value: u8) {
    atomic.element(index).or(value, Release);
}

#[unsafe(no_mangle)]
pub fn byte_and(atomic: &Atomic<[u8; 4]>, index: usize, value: u8) {
    atomic.element(index).and(value, Release);
}

#[unsafe(no_mangle)]
pub fn byte_update(atomic: &Atomic<[u8; 4]>, index: usize) -> [u8; 4] {
    atomic.element(index).update(AcqRel, Acquire, |byte| byte.wrapping_add(1))
}

#[unsafe(no_mangle)]
pub fn flag_test_and_set(atomic: &Atomic<[bool; 64]>, index: usize) -> bool {
    atomic.element(index).test_and_set(AcqRel)
}

/// Turns the flag at `index` on, unchecked, and returns it before.
///
/// # Safety
/// `index` lies below 64.
#[unsafe(no_mangle)]
pub unsafe fn flag_test_and_set_unchecked(atomic: &Atomic<[bool; 64]>, index: usize) -> bool {
    // SAFETY: the caller's index lies below 64, as this probe's contract says.
    unsafe { atomic.element_unchecked(index) }.test_and_set(AcqRel)
}

#[unsafe(no_mangle)]
pub fn flag_test_and_set_checked(atomic: &Atomic<[bool; 64]>, index: usize) -> bool {
    atomic.element_checked(index).is_some_and(|flag| flag.test_and_set(AcqRel))
}

#[unsafe(no_mangle)]
pub fn flag_place_test_and_set(atomic: &Atomic<[bool; 64]>) -> bool {
    atomic.place::<5>().test_and_set(AcqRel)
}

#[unsafe(no_mangle)]
pub fn flag_set(atomic: &Atomic<[bool; 64]>, index: usize) {
    atomic.element(index).set(Release);
}

#[unsafe(no_mangle)]
pub fn flag_clear(atomic: &Atomic<[bool; 64]>, index: usize) {
    atomic.element(index).clear(Release);
}

#[unsafe(no_mangle)]
pub fn flag_load(atomic: &Atomic<[bool; 64]>, index: usize) -> bool {
    atomic.element(index).load(Relaxed)
}

#[unsafe(no_mangle)]
pub fn grid_test_and_set(atomic: &Atomic<[[bool; 8]; 8]>, row: usize, column: usize) -> bool {
    atomic.element(row).element(column).test_and_set(AcqRel)
}

#[unsafe(no_mangle)]
pub fn grid_place_test_and_set(atomic: &Atomic<[[bool; 8]; 8]>, row: usize) -> bool {
    atomic.element(row).place::<3>().test_and_set(AcqRel)
}

#[unsafe(no_mangle)]
pub fn halves_last_fetch_add(atomic: &Atomic<[u16; 2]>) -> [u16; 2] {
    atomic.last().fetch_add(1, AcqRel)
}

#[unsafe(no_mangle)]
pub fn halves_last_fetch_add_discarded(atomic: &Atomic<[u16; 2]>) {
    let _ = atomic.last().fetch_add(1, AcqRel);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", feature = "aarch64-only"))]
#[unsafe(no_mangle)]
pub fn byte_fetch_or(atomic: &Atomic<[u8; 4]>, index: usize, value: u8) -> [u8; 4] {
    atomic.element(index).fetch_or(value, AcqRel)
}
