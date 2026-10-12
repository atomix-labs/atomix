//! A test-and-set returns one bit, so an element has it only where it is a `bool`.

use atomix_core::Atomic;
use atomix_core::ordering::AcqRel;

fn main() {
    let levels = Atomic::new([0_u8; 4]);
    let _ = levels.element(0).test_and_set(AcqRel);
}
