//! A reference's and a function pointer's operations, each a raw pointer's: a reference's load
//! acquires and its store releases, as its floor requires, and a function pointer takes any
//! ordering.

use atomix_core::ordering::{AcqRel, Acquire, Relaxed, Release};
use atomix_core::{Atomic, AtomicPtr};

/// A configuration a reference points to.
pub struct Config {
    /// How many price levels a quote shows.
    pub depth: u32,
}

/// A hook: a function pointer whose signature is higher-ranked.
pub type Hook = fn(&str) -> usize;

#[unsafe(no_mangle)]
pub fn pointer_load(atomic: &AtomicPtr<Config>) -> *mut Config {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn pointer_load_relaxed(atomic: &AtomicPtr<Config>) -> *mut Config {
    atomic.load(Relaxed)
}

#[unsafe(no_mangle)]
pub fn pointer_store(atomic: &AtomicPtr<Config>, pointer: *mut Config) {
    atomic.store(pointer, Release);
}

#[unsafe(no_mangle)]
pub fn pointer_swap(atomic: &AtomicPtr<Config>, pointer: *mut Config) -> *mut Config {
    atomic.swap(pointer, AcqRel)
}

#[unsafe(no_mangle)]
pub fn pointer_compare_exchange(
    atomic: &AtomicPtr<Config>, current: *mut Config, new: *mut Config,
) -> Result<*mut Config, *mut Config> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

#[unsafe(no_mangle)]
pub fn config_load(atomic: &Atomic<&'static Config>) -> &'static Config {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn config_depth(atomic: &Atomic<&'static Config>) -> u32 {
    atomic.load(Acquire).depth
}

#[unsafe(no_mangle)]
pub fn config_store(atomic: &Atomic<&'static Config>, config: &'static Config) {
    atomic.store(config, Release);
}

#[unsafe(no_mangle)]
pub fn config_swap(atomic: &Atomic<&'static Config>, config: &'static Config) -> &'static Config {
    atomic.swap(config, AcqRel)
}

#[unsafe(no_mangle)]
pub fn config_compare_exchange(
    atomic: &Atomic<&'static Config>, current: &'static Config, new: &'static Config,
) -> Result<&'static Config, &'static Config> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

#[unsafe(no_mangle)]
pub fn option_config_load(atomic: &Atomic<Option<&'static Config>>) -> Option<&'static Config> {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn hook_load(atomic: &Atomic<Option<Hook>>) -> Option<Hook> {
    atomic.load(Relaxed)
}

#[unsafe(no_mangle)]
pub fn hook_load_acquire(atomic: &Atomic<Option<Hook>>) -> Option<Hook> {
    atomic.load(Acquire)
}

#[unsafe(no_mangle)]
pub fn hook_store(atomic: &Atomic<Option<Hook>>, hook: Hook) {
    atomic.store(Some(hook), Release);
}

#[unsafe(no_mangle)]
pub fn hook_compare_exchange(
    atomic: &Atomic<Option<Hook>>, current: Option<Hook>, new: Option<Hook>,
) -> Result<Option<Hook>, Option<Hook>> {
    atomic.compare_exchange(current, new, AcqRel, Acquire)
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
#[unsafe(no_mangle)]
pub fn name_load_rmw(atomic: &Atomic<&'static str>) -> &'static str {
    atomic.load_rmw(Acquire)
}

#[cfg(any(target_feature = "lse2", target_feature = "avx"))]
#[unsafe(no_mangle)]
pub fn name_load(atomic: &Atomic<&'static str>) -> &'static str {
    atomic.load(Acquire)
}
