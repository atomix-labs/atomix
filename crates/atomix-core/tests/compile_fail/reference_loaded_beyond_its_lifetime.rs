//! A load hands out the reference for its own lifetime, `'a`, and no longer.

use atomix_core::Atomic;
use atomix_core::ordering::Acquire;

fn outlive<'a>(atomic: &Atomic<&'a u64>) -> &'static u64 {
    atomic.load(Acquire)
}

fn main() {
    let value = 7_u64;
    let _ = outlive(&Atomic::new(&value));
}
