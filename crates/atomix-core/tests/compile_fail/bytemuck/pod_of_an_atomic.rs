//! A shared reference to an atomic writes, so no cell is `Copy`, and no atomic is bytemuck's `Pod`:
//! none lends its bytes, as a plain value of the same bits does.
//!
//! The refusal names the first eight types that are `Pod`, sorted. `Count` is among them, so they
//! end at `[T; N]` on every target, where on `x86_64` bytemuck's `__m128` would follow it.

use atomix_core::AtomicU64;
use bytemuck::{NoUninit, Pod, Zeroable};

/// A plain count, which lends its bytes.
#[derive(Clone, Copy)]
#[repr(transparent)]
struct Count(u64);

// SAFETY: `Count` is a `u64`, every bit pattern of which is a value, zero among them.
unsafe impl Zeroable for Count {}
// SAFETY: and it is `repr(transparent)` over the `u64`, with no padding and no interior mutability.
unsafe impl Pod for Count {}

/// Compiles only where `T` lends its bytes, as `bytes_of` needs.
const fn lends_its_bytes<T: NoUninit>() {}

fn main() {
    lends_its_bytes::<Count>();
    lends_its_bytes::<AtomicU64>();
}
