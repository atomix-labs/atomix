//! An array's operations, each its repr's, beside an integer's of the same width: four bytes are a
//! `u32`, sixty-four `bool`s a `u64`; and its elements', at an index chosen at run time, checked,
//! unchecked or a constant, each the instruction a field's is, a `Packed` one's too; and, in
//! `by_hand`, a byte's bitwise operations written over core's atomics, each its probe's twin.

use atomix_core::ordering::{AcqRel, Acquire, Relaxed, Release};
use atomix_core::{Atomic, AtomicU32, Packed, RangedU8};

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
pub fn byte_not(atomic: &Atomic<[u8; 4]>, index: usize) {
    atomic.element(index).not(Release);
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

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", feature = "aarch64-only"))]
#[unsafe(no_mangle)]
pub fn byte_fetch_and(atomic: &Atomic<[u8; 4]>, index: usize, value: u8) -> [u8; 4] {
    atomic.element(index).fetch_and(value, AcqRel)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", feature = "aarch64-only"))]
#[unsafe(no_mangle)]
pub fn byte_fetch_not(atomic: &Atomic<[u8; 4]>, index: usize) -> [u8; 4] {
    atomic.element(index).fetch_not(AcqRel)
}

/// Sixteen quarters, two bits each, in a `u32`.
type Fill = Packed<[RangedU8<0, 3>; 16], u32>;

#[unsafe(no_mangle)]
pub fn quarter_load(atomic: &Atomic<Fill>, index: usize) -> RangedU8<0, 3> {
    atomic.element(index).load(Acquire)
}

#[unsafe(no_mangle)]
pub fn quarter_place_load(atomic: &Atomic<Fill>) -> RangedU8<0, 3> {
    atomic.place::<5>().load(Acquire)
}

#[unsafe(no_mangle)]
pub fn bools_load(atomic: &Atomic<[bool; 64]>) -> [bool; 64] {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn bools_store(atomic: &Atomic<[bool; 64]>, flags: [bool; 64]) {
    atomic.store(flags, Release);
}

#[unsafe(no_mangle)]
pub fn eight_bools_load(atomic: &Atomic<[bool; 8]>) -> [bool; 8] {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn eight_bools_store(atomic: &Atomic<[bool; 8]>, flags: [bool; 8]) {
    atomic.store(flags, Release);
}

#[unsafe(no_mangle)]
pub fn sixteen_bools_load(atomic: &Atomic<[bool; 16]>) -> [bool; 16] {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn sixteen_bools_store(atomic: &Atomic<[bool; 16]>, flags: [bool; 16]) {
    atomic.store(flags, Release);
}

pub mod by_hand {
    //! A byte's `and`, `not`, `fetch_and` and `fetch_not` by hand over core's `AtomicU32`, as code
    //! over core's atomics writes them, after the check of the index a slice's index makes: each
    //! probe's twin, named for it with `core_` before, which `tests/codegen.rs` holds it to
    //! instruction for instruction.

    use core::sync::atomic::AtomicU32;
    use core::sync::atomic::Ordering::{AcqRel, Release};

    /// Panics as a slice's index does past its end, with std's message: the twins call it off their
    /// fast path, as an element calls its own refusal. It is public, as atomix's is to the crate
    /// that inlines an element, so LLVM keeps its length a parameter, passed as atomix's is.
    #[cold]
    #[inline(never)]
    #[track_caller]
    pub fn refuse_index(index: usize, length: usize) -> ! {
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
}
