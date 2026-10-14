//! A load hands out a borrowed descriptor for its own lifetime, `'a`, and no longer: the atomic is
//! invariant in it, as a reference's is.

use std::os::fd::BorrowedFd;

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn outlive<'a>(atomic: &Atomic<BorrowedFd<'a>>) -> BorrowedFd<'static> {
    atomic.load(Relaxed)
}

fn main() {}
