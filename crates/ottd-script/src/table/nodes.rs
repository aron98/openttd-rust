//! `SQTable`'s physical node array, descending free scan and collision relocation.
use super::scalar::Scalar;
use crate::{VmError, heap::Stored};
#[derive(Clone, Debug)]
struct Node {
    key: Scalar,
    value: Stored,
    hash: u64,
    next: Option<usize>,
}
#[derive(Debug)]
pub(crate) struct Nodes {
    slots: Vec<Node>,
    free: usize,
    used: usize,
}
// All indices originate in this module's power-of-two allocation and links.
#[expect(
    clippy::indexing_slicing,
    reason = "Node indices are masked to allocated capacity or retained valid links"
)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Power-of-two capacity >=4 and valid node indices bound all arithmetic"
)]
impl Nodes {
    pub(crate) fn new(capacity: usize) -> Result<Self, VmError> {
        let capacity = capacity
            .max(4)
            .checked_next_power_of_two()
            .ok_or(VmError::Allocation)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity)
            .map_err(|_| VmError::Allocation)?;
        slots.resize(
            capacity,
            Node {
                key: Scalar::Null,
                value: Stored::null(),
                hash: 0,
                next: None,
            },
        );
        Ok(Self {
            slots,
            free: capacity - 1,
            used: 0,
        })
    }
    fn main_position(&self, hash: u64) -> usize {
        // The mask is less than usize::MAX even on narrower hosts.
        usize::try_from(hash & ((self.slots.len() - 1) as u64)).unwrap_or_default()
    }
    fn find(&self, key: &Scalar, hash: u64) -> Option<usize> {
        let mut index = self.main_position(hash);
        loop {
            let node = &self.slots[index];
            if node.key == *key {
                return Some(index);
            }
            index = node.next?;
        }
    }
    pub(crate) fn get(&self, key: &Scalar, hash: u64) -> Option<Stored> {
        self.find(key, hash)
            .map(|index| self.slots[index].value.clone())
    }
    pub(crate) fn set(&mut self, key: &Scalar, hash: u64, value: Stored) -> Result<(), VmError> {
        let index = self.find(key, hash).ok_or(VmError::MissingIndex)?;
        self.slots[index].value = value;
        Ok(())
    }
    pub(crate) const fn len(&self) -> usize {
        self.used
    }
    pub(crate) fn next(&self, cursor: usize) -> Option<(usize, Scalar, Stored)> {
        self.slots
            .iter()
            .enumerate()
            .skip(cursor)
            .find_map(|(index, node)| {
                (node.key != Scalar::Null)
                    .then(|| (index + 1, node.key.clone(), node.value.clone()))
            })
    }
    pub(crate) fn insert(&mut self, key: Scalar, hash: u64, value: Stored) -> Result<(), VmError> {
        if let Some(index) = self.find(&key, hash) {
            self.slots[index].value = value;
            return Ok(());
        }
        let mut main = self.main_position(hash);
        if self.slots[main].key != Scalar::Null {
            let free = self.free;
            let mut other = self.main_position(self.slots[main].hash);
            if main > free && other != main {
                while self.slots[other].next != Some(main) {
                    other = self.slots[other].next.ok_or(VmError::InvalidBytecode)?;
                }
                self.slots[other].next = Some(free);
                self.slots[free] = self.slots[main].clone();
                self.slots[main] = Node {
                    key: Scalar::Null,
                    value: Stored::null(),
                    hash: 0,
                    next: None,
                };
            } else {
                self.slots[free].next = self.slots[main].next;
                self.slots[main].next = Some(free);
                main = free;
            }
        }
        self.slots[main].key = key.clone();
        self.slots[main].hash = hash;
        loop {
            if self.slots[self.free].key == Scalar::Null && self.slots[self.free].next.is_none() {
                self.slots[main].value = value;
                self.used += 1;
                return Ok(());
            }
            if self.free == 0 {
                break;
            }
            self.free -= 1;
        }
        self.rehash(true)?;
        self.insert(key, hash, value)
    }
    pub(crate) fn remove(&mut self, key: &Scalar, hash: u64) -> Result<Stored, VmError> {
        let index = self.find(key, hash).ok_or(VmError::MissingIndex)?;
        let value = std::mem::replace(&mut self.slots[index].value, Stored::null());
        self.slots[index].key = Scalar::Null;
        self.used -= 1;
        self.rehash(false)?;
        Ok(value)
    }
    pub(crate) fn values(&self) -> impl Iterator<Item = &Stored> {
        self.slots.iter().map(|node| &node.value)
    }
    pub(crate) fn duplicate(&self) -> Result<Self, VmError> {
        self.reinsert(self.slots.len())
    }
    fn reinsert(&self, capacity: usize) -> Result<Self, VmError> {
        let mut replacement = Self::new(capacity)?;
        for node in &self.slots {
            if node.key != Scalar::Null {
                replacement.insert(node.key.clone(), node.hash, node.value.clone())?;
            }
        }
        Ok(replacement)
    }
    fn rehash(&mut self, force: bool) -> Result<(), VmError> {
        let size = self.slots.len();
        let capacity = if self.used >= size - size / 4 {
            size.checked_mul(2).ok_or(VmError::Allocation)?
        } else if self.used <= size / 4 && size > 4 {
            size / 2
        } else if force {
            size
        } else {
            return Ok(());
        };
        *self = self.reinsert(capacity)?;
        Ok(())
    }
}
