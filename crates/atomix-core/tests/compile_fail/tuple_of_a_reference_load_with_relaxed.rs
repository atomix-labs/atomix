//! A tuple that holds a reference keeps the reference's floor, as a derived struct does: a
//! `Relaxed` load would hand out a reference to writes not yet seen, though its flag takes any
//! ordering.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

/// What a load hands out a reference to.
static DEFAULT: u64 = 7;

fn main() {
    let published = Atomic::new((&DEFAULT, false));
    published.fields().1.set(Relaxed);
    let _ = published.load(Relaxed);
}
