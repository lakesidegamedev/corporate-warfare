//! Stable logical handles and dense row indirection.
//!
//! A handle is `slot | generation << 32`. Deleting an entity bumps the slot generation, so a
//! stale handle can never address the entity that later reuses the slot. Rows are stored
//! densely and swap-removed; the slot table is repaired in the same operation, so no
//! surviving handle ever points at the wrong row.

use serde::{Deserialize, Serialize};

/// Sentinel used for "no entity". Never allocated.
pub const INVALID_HANDLE: u64 = 0;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Handle(pub u64);

impl Handle {
    pub const INVALID: Handle = Handle(0);

    pub fn new(slot: u32, generation: u32) -> Handle {
        Handle((slot as u64) | ((generation as u64) << 32))
    }

    pub const fn slot(self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }

    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    pub const fn is_valid(self) -> bool {
        self.0 != INVALID_HANDLE
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Slot {
    row: u32,
    generation: u32,
    live: bool,
}

/// Dense table addressed by generation-checked handles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slab<T> {
    slots: Vec<Slot>,
    rows: Vec<T>,
    /// Slot index owning each dense row, used to repair the slot table after swap-remove.
    row_slot: Vec<u32>,
    free: Vec<u32>,
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Slab {
            slots: Vec::new(),
            rows: Vec::new(),
            row_slot: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<T> Slab<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn insert(&mut self, value: T) -> Handle {
        let row = self.rows.len() as u32;
        self.rows.push(value);
        if let Some(slot_index) = self.free.pop() {
            let generation = self.slots[slot_index as usize].generation;
            self.slots[slot_index as usize] = Slot {
                row,
                generation,
                live: true,
            };
            self.row_slot.push(slot_index);
            Handle::new(slot_index, generation)
        } else {
            let slot_index = self.slots.len() as u32;
            self.slots.push(Slot {
                row,
                generation: 1,
                live: true,
            });
            self.row_slot.push(slot_index);
            Handle::new(slot_index, 1)
        }
    }

    pub fn get(&self, handle: Handle) -> Option<&T> {
        self.row_of(handle).map(|row| &self.rows[row])
    }

    pub fn get_mut(&mut self, handle: Handle) -> Option<&mut T> {
        match self.row_of(handle) {
            Some(row) => Some(&mut self.rows[row]),
            None => None,
        }
    }

    pub fn contains(&self, handle: Handle) -> bool {
        self.row_of(handle).is_some()
    }

    pub fn remove(&mut self, handle: Handle) -> Option<T> {
        let row = self.row_of(handle)?;
        let slot_index = handle.slot();
        let removed = self.rows.swap_remove(row);
        self.row_slot.swap_remove(row);
        if row < self.rows.len() {
            let moved_slot = self.row_slot[row];
            self.slots[moved_slot as usize].row = row as u32;
        }
        let slot = &mut self.slots[slot_index as usize];
        slot.live = false;
        if slot.generation == u32::MAX {
            // Generation space exhausted: retire the slot permanently.
            slot.generation = u32::MAX;
        } else {
            slot.generation += 1;
            self.free.push(slot_index);
        }
        Some(removed)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle, &T)> + '_ {
        self.rows.iter().enumerate().map(|(row, value)| {
            let slot = self.row_slot[row];
            let generation = self.slots[slot as usize].generation;
            (Handle::new(slot, generation), value)
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle, &mut T)> + '_ {
        let slots = &self.slots;
        let row_slot = &self.row_slot;
        self.rows.iter_mut().enumerate().map(move |(row, value)| {
            let slot = row_slot[row];
            let generation = slots[slot as usize].generation;
            (Handle::new(slot, generation), value)
        })
    }

    /// Handles in stable ascending order. Deterministic iteration must use this or a
    /// declared schedule, never raw row order after removals.
    pub fn handles_sorted(&self) -> Vec<Handle> {
        let mut handles: Vec<Handle> = self.iter().map(|(handle, _)| handle).collect();
        handles.sort_unstable();
        handles
    }

    fn row_of(&self, handle: Handle) -> Option<usize> {
        if !handle.is_valid() {
            return None;
        }
        let slot = *self.slots.get(handle.slot() as usize)?;
        if !slot.live || slot.generation != handle.generation() {
            return None;
        }
        Some(slot.row as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_handle_is_rejected_after_reuse() {
        let mut slab = Slab::new();
        let first = slab.insert("a");
        let second = slab.insert("b");
        assert_eq!(slab.remove(first), Some("a"));
        let third = slab.insert("c");
        assert_eq!(third.slot(), first.slot());
        assert_ne!(third.generation(), first.generation());
        assert_eq!(slab.get(first), None);
        assert_eq!(slab.get(second), Some(&"b"));
        assert_eq!(slab.get(third), Some(&"c"));
    }

    #[test]
    fn swap_remove_repairs_indirection() {
        let mut slab = Slab::new();
        let a = slab.insert(1);
        let b = slab.insert(2);
        let c = slab.insert(3);
        slab.remove(a);
        // `c` was swapped into row 0; both survivors must still resolve.
        assert_eq!(slab.get(b), Some(&2));
        assert_eq!(slab.get(c), Some(&3));
        assert_eq!(slab.len(), 2);
    }

    #[test]
    fn handles_are_stable_sorted() {
        let mut slab = Slab::new();
        let a = slab.insert(1);
        let b = slab.insert(2);
        assert_eq!(slab.handles_sorted(), vec![a, b]);
    }

    #[test]
    fn invalid_handle_never_addresses_a_row() {
        let mut slab = Slab::new();
        slab.insert(1);
        assert_eq!(slab.get(Handle::INVALID), None);
    }
}
