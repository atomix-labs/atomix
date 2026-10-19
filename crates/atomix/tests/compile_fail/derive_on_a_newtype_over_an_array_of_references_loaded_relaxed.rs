//! A newtype over an array of references takes the array's repr, and with it a reference's floor:
//! a `Relaxed` load of it would hand out references to what another thread wrote, before those
//! writes are seen.

use atomix::ordering::Relaxed;
use atomix::{Atom, Atomic};

/// A configuration a service reads on every order.
struct Config;

/// The configurations in force, one per venue.
#[derive(Clone, Copy, Atom)]
struct Venues([&'static Config; 1]);

fn relaxed(venues: &Atomic<Venues>) -> Venues {
    venues.load(Relaxed)
}

fn main() {}
