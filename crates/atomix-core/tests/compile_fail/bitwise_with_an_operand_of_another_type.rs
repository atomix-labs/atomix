//! `and`, `or` and `xor` combine two values of one type: an operand of another,
//! a `u32` into an atomic `u64`, is a mismatched type, since rustc infers the operand's
//! type from the atomic's one impl.

use atomix_core::AtomicU64;
use atomix_core::ordering::Relaxed;

fn main() {
    let flags = AtomicU64::new(0);
    flags.or(0b10_u32, Relaxed);
}
