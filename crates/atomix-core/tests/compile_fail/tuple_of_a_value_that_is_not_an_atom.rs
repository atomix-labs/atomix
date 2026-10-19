//! A tuple is an atom where each of its fields is: a `String` is not.

use atomix_core::Atomic;

fn main() {
    let _ = Atomic::new((1_u8, String::new()));
}
