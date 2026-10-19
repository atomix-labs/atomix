//! A tuple's operations, each its repr's, beside an integer's of the same width, and its fields',
//! each the instruction a derived struct's field's is, an array element's at a run-time index after
//! the index's check and a shift by its slot: beside a reference, a flag's `Relaxed` load
//! and set and a count's `Relaxed` read are its twin's beside a `NonNull`, raised by no floor; and
//! a hook beside a flag takes two words, its flag the second.

use core::ptr::NonNull;

use atomix_core::Atomic;
use atomix_core::ordering::{AcqRel, Acquire, Relaxed, Release};

#[unsafe(no_mangle)]
pub fn halves_or(atomic: &Atomic<(u32, u32)>, value: (u32, u32)) {
    atomic.or(value, Release);
}

#[unsafe(no_mangle)]
pub fn halves_store(atomic: &Atomic<(u32, u32)>, value: (u32, u32)) {
    atomic.store(value, Release);
}

#[unsafe(no_mangle)]
pub fn halves_load_second(atomic: &Atomic<(u32, u32)>) -> u32 {
    atomic.fields().1.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn counted_bytes_fetch_add(atomic: &Atomic<(u8, u8, u16)>) -> (u8, u8, u16) {
    atomic.fields().2.fetch_add(1, AcqRel)
}

#[unsafe(no_mangle)]
pub fn live_quote_test_and_set(atomic: &Atomic<(u32, bool)>) -> bool {
    atomic.fields().1.test_and_set(AcqRel)
}

#[unsafe(no_mangle)]
pub fn slot_test_and_set_written(atomic: &Atomic<[(bool, u8); 4]>, index: usize) -> bool {
    atomic.element(index).fields().0.test_and_set(AcqRel)
}

/// A node aligned to eight bytes, whose pointer leaves three low bits clear.
#[repr(align(8))]
pub struct Node {
    /// What it holds.
    pub value: u64,
}

#[unsafe(no_mangle)]
pub fn marked_reference_load_flag(atomic: &Atomic<(&'static Node, bool)>) -> bool {
    atomic.fields().1.load(Relaxed)
}

#[unsafe(no_mangle)]
pub fn marked_pointer_load_flag(atomic: &Atomic<(NonNull<Node>, bool)>) -> bool {
    atomic.fields().1.load(Relaxed)
}

#[unsafe(no_mangle)]
pub fn marked_reference_set_flag(atomic: &Atomic<(&'static Node, bool)>) {
    atomic.fields().1.set(Relaxed);
}

#[unsafe(no_mangle)]
pub fn marked_pointer_set_flag(atomic: &Atomic<(NonNull<Node>, bool)>) {
    atomic.fields().1.set(Relaxed);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
#[unsafe(no_mangle)]
pub fn counted_reference_load_count(atomic: &Atomic<(&'static Node, u64)>) -> u64 {
    atomic.fields().1.load_rmw(Relaxed)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
#[unsafe(no_mangle)]
pub fn counted_pointer_load_count(atomic: &Atomic<(NonNull<Node>, u64)>) -> u64 {
    atomic.fields().1.load_rmw(Relaxed)
}

/// A hook beside whether it is armed: two words, since a function's code leaves no low bit clear.
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
pub type Armed = (Option<fn(u64) -> u64>, bool);

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
#[unsafe(no_mangle)]
pub fn armed_load_flag(atomic: &Atomic<Armed>) -> bool {
    atomic.fields().1.load_rmw(Relaxed)
}
