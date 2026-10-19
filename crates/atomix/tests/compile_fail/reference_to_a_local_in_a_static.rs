//! An atomic of a `'static` reference takes no reference to a local, which would dangle once the
//! local's frame returns.

use atomix::Atomic;
use atomix::ordering::Release;

/// What a trading service reads on every order.
struct Config {
    depth: u32,
}

/// The configuration before any is published.
static DEFAULT: Config = Config { depth: 10 };

/// The configuration in force.
static CONFIG: Atomic<&'static Config> = Atomic::new(&DEFAULT);

fn main() {
    let local = Config { depth: 20 };
    CONFIG.store(&local, Release);
}
