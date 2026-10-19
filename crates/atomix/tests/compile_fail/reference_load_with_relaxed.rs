//! A `Relaxed` load of a reference would hand out a reference to what another thread wrote, before
//! those writes are seen: reading through it would race with them.

use atomix::Atomic;
use atomix::ordering::Relaxed;

/// What a trading service reads on every order.
struct Config {
    depth: u32,
}

/// The configuration before any is published.
static DEFAULT: Config = Config { depth: 10 };

/// The configuration in force.
static CONFIG: Atomic<&'static Config> = Atomic::new(&DEFAULT);

fn main() {
    let _ = CONFIG.load(Relaxed).depth;
}
