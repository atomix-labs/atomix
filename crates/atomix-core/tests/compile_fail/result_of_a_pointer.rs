//! A `Result` keeps its tag above its payloads' bits, which a pointer's provenance is not: an
//! enum of the same variants derives a pointer enum, its tag in the pointer's low bits.

use core::ptr::NonNull;

use atomix_core::Atomic;

fn main() {
    let _ = Atomic::new(Ok::<NonNull<u64>, u8>(NonNull::dangling()));
}
