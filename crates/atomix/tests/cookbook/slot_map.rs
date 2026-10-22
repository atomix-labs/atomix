//! A slot map: values in the slots of an arena, each found by a key of its slot and the slot's
//! generation, so a key may outlive its value: once the value is removed, the generation moves on,
//! and the key finds nothing.
//!
//! A slot is one word, its value beside its generation, so a lookup reads both at once, and a
//! removal checks the key and frees the slot in one compare-exchange. Free slots are kept on the
//! free list of the first recipe.

use atomix::ordering::{Acquire, Relaxed, Release};
use atomix::{Atom, Atomic};

use super::free_list::{FreeList, Index};

// ANCHOR: slot
/// A slot: its value, then its generation, odd while it holds the value; 32 bits each, in a
/// `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
struct Slot {
    /// The value it holds, or last held.
    value: u32,
    /// How many times a value went in or came out, wrapping.
    generation: u32,
}

/// A value's key: its slot, and the slot's generation as the value went in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Key {
    /// The slot.
    slot: Index,
    /// The slot's generation while it holds the value.
    generation: u32,
}
// ANCHOR_END: slot

/// Values of `u32` in up to 65,535 slots.
#[derive(Debug)]
pub(crate) struct SlotMap {
    /// The slots.
    slots: Box<[Atomic<Slot>]>,
    /// The slots that hold no value.
    free: FreeList,
}

impl SlotMap {
    /// A map of `len` slots, each free.
    pub(crate) fn new(len: u16) -> Self {
        let empty = Slot { value: 0, generation: 0 };
        let slots = (0..len).map(|_| Atomic::new(empty)).collect();
        Self { slots, free: FreeList::new(len) }
    }

    // ANCHOR: insert
    /// Puts `value` in a free slot, and returns its key, or `None` where every slot holds one.
    pub(crate) fn insert(&self, value: u32) -> Option<Key> {
        let slot = self.free.pop()?;
        let place = self.slot(slot);
        // ORDERING: Relaxed: a free slot is this thread's alone, its last removal published by the
        // free list's push.
        let generation = place.load(Relaxed).generation.wrapping_add(1);
        // ORDERING: Release, publishing whatever this thread wrote before, to the lookup that finds
        // the value.
        place.store(Slot { value, generation }, Release);
        Some(Key { slot, generation })
    }
    // ANCHOR_END: insert

    // ANCHOR: get
    /// The value `key` finds, or `None` where it was removed.
    pub(crate) fn get(&self, key: Key) -> Option<u32> {
        // ORDERING: Acquire, pairing with the insert's Release.
        let slot = self.slot(key.slot).load(Acquire);
        (slot.generation == key.generation).then_some(slot.value)
    }

    /// Removes the value `key` finds and returns it, or `None` where it was removed already.
    pub(crate) fn remove(&self, key: Key) -> Option<u32> {
        let vacate = |slot: Slot| {
            let generation = slot.generation.wrapping_add(1);
            (slot.generation == key.generation).then_some(Slot { generation, ..slot })
        };
        // ORDERING: Acquire on every read, pairing with the insert's Release; the free list's push
        // publishes the freed slot to its next insert.
        let removed = self.slot(key.slot).try_update(Acquire, Acquire, vacate).ok()?;
        self.free.push(key.slot);
        Some(removed.value)
    }
    // ANCHOR_END: get

    /// The slot of `slot`.
    fn slot(&self, slot: Index) -> &Atomic<Slot> {
        &self.slots[usize::from(slot.get())]
    }
}
