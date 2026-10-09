//! A newtype over a reference takes the reference's repr, and with it its orderings' floor: a
//! `Relaxed` load of it would hand out a reference to what another thread wrote, before those
//! writes are seen.

#![feature(const_trait_impl)]

use atomix::ordering::Relaxed;
use atomix::{Atom, Atomic};

/// A configuration a service reads on every order.
struct Config;

/// The configuration in force, a newtype over a reference.
#[derive(Clone, Copy, Atom)]
struct Current<'a>(&'a Config);

fn relaxed<'a>(current: &Atomic<Current<'a>>) -> Current<'a> {
    current.load(Relaxed)
}

fn main() {}
