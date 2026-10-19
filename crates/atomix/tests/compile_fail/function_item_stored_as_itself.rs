//! A function's name is a value of a type of its own, of no bits, which no atomic holds: the
//! function pointer is a type the caller names.

use atomix::Atomic;

fn main() {
    let _ = Atomic::new(str::len);
}
