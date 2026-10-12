//! A byte leaves its pointer no low bit clear for the flag beside it, so every use of the tuple is
//! refused: its encode, and its decodes and projection too, so one an atomic holds without an
//! encode, through `from_ptr`, never sets a flag it has no bit for.

use atomix_core::ordering::Relaxed;
use atomix_core::{Atomic, Primitive, ReprOf};

fn main() {
    let mut null: ReprOf<&'static u8> = Primitive::from_bits(0);
    // SAFETY: the place holds a null repr, which decodes, and lives as long as the atomic.
    let atomic = unsafe { Atomic::<(Option<&'static u8>, bool)>::from_ptr(&raw mut null) };
    atomic.fields().1.set(Relaxed);
}
