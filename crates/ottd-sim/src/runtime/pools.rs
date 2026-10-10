//! Pure allocation metadata from OpenTTD `core/pool_func.hpp` and vehicle.cpp.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Invalid allocation request, rejected without changing allocator state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PoolError {
    /// Limit must be nonzero and growth must be a power of two.
    #[error("invalid pool limit or growth step")]
    Configuration,
    /// Explicit identity exceeds the pool's exclusive maximum.
    #[error("pool ID {0} is out of range")]
    OutOfRange(u32),
    /// Explicit identity is already occupied.
    #[error("pool ID {0} is already occupied")]
    Occupied(u32),
    /// Identity to free is unoccupied.
    #[error("pool ID {0} is not occupied")]
    Missing(u32),
    /// Native `CanAllocate` would return false.
    #[error("pool exhausted")]
    Exhausted,
}

/// Logical pool state; slots describe native growth, not Rust heap capacity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolSnapshot {
    /// Native lowest-free search cursor.
    pub first_free: u32,
    /// Highest ever occupied ID plus one, retained after freeing.
    pub first_unused: u32,
    /// Number of occupied IDs.
    pub items: usize,
    /// Native addressable slot count after growth.
    pub slots: u32,
    /// Occupied IDs in ascending native iteration order.
    pub occupied: Vec<u32>,
}

/// One native pool's identity metadata, cloneable for tentative transactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolAllocator {
    limit: u32,
    growth: u32,
    first_free: u32,
    first_unused: u32,
    slots: u32,
    occupied: BTreeSet<u32>,
}
impl PoolAllocator {
    /// Create an empty pool with the native exclusive maximum and growth step.
    /// # Errors
    /// Rejects zero limits and non-power-of-two growth steps.
    pub const fn new(limit: u32, growth: u32) -> Result<Self, PoolError> {
        if limit == 0 || !growth.is_power_of_two() {
            return Err(PoolError::Configuration);
        }
        Ok(Self {
            limit,
            growth,
            first_free: 0,
            first_unused: 0,
            slots: 0,
            occupied: BTreeSet::new(),
        })
    }
    /// Reconstruct explicit saved IDs without advancing the native search cursor.
    /// # Errors
    /// Rejects invalid configuration, duplicate IDs and out-of-range IDs.
    pub fn restore(
        limit: u32,
        growth: u32,
        ids: impl IntoIterator<Item = u32>,
    ) -> Result<Self, PoolError> {
        let mut pool = Self::new(limit, growth)?;
        for id in ids {
            pool.insert(id)?;
        }
        Ok(pool)
    }
    /// Whether enough unoccupied IDs remain; this has no allocation effects.
    pub fn can_allocate(&self, count: u32) -> bool {
        u64::try_from(self.occupied.len())
            .is_ok_and(|used| used.saturating_add(u64::from(count)) <= u64::from(self.limit))
    }
    /// Mark one explicit ID occupied without runtime allocation side effects.
    /// # Errors
    /// Rejects duplicate and out-of-range IDs before mutation.
    pub fn insert(&mut self, id: u32) -> Result<(), PoolError> {
        if id >= self.limit {
            return Err(PoolError::OutOfRange(id));
        }
        if self.occupied.contains(&id) {
            return Err(PoolError::Occupied(id));
        }
        let slots = if id >= self.slots {
            let grown = u64::from(id)
                .saturating_add(1)
                .div_ceil(u64::from(self.growth))
                .saturating_mul(u64::from(self.growth));
            u32::try_from(grown.min(u64::from(self.limit))).map_err(|_| PoolError::Configuration)?
        } else {
            self.slots
        };
        self.occupied.insert(id);
        self.first_unused = self.first_unused.max(id.saturating_add(1));
        self.slots = slots;
        Ok(())
    }
    /// Allocate the lowest free ID, advancing the native search cursor.
    /// # Errors
    /// Returns Exhausted without mutation when no IDs remain.
    pub fn allocate(&mut self) -> Result<u32, PoolError> {
        if !self.can_allocate(1) {
            return Err(PoolError::Exhausted);
        }
        let mut id = self.first_free;
        for used in self.occupied.range(id..) {
            if *used != id {
                break;
            }
            id = id.saturating_add(1);
        }
        self.insert(id)?;
        self.first_free = id.saturating_add(1);
        Ok(id)
    }
    /// Free an ID, retaining native allocated slots and high-water mark.
    /// # Errors
    /// Rejects an unoccupied ID before mutation.
    pub fn free(&mut self, id: u32) -> Result<(), PoolError> {
        if !self.occupied.remove(&id) {
            return Err(PoolError::Missing(id));
        }
        self.first_free = self.first_free.min(id);
        Ok(())
    }
    /// Native `CleanPool` semantics, including clearing growth metadata.
    pub fn reset(&mut self) {
        self.occupied.clear();
        self.first_free = 0;
        self.first_unused = 0;
        self.slots = 0;
    }
    /// Observe all logical allocation state without exposing host heap capacity.
    pub fn snapshot(&self) -> PoolSnapshot {
        PoolSnapshot {
            first_free: self.first_free,
            first_unused: self.first_unused,
            items: self.occupied.len(),
            slots: self.slots,
            occupied: self.occupied.iter().copied().collect(),
        }
    }
}

/// Release attempted outside every bitmap block registered by `UseID`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unit number {0} has no registered bitmap block")]
pub struct UnitReleaseError(pub u16);

/// One company's one vehicle type's independent, one-based unit-number generator.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitNumberAllocator {
    used: Vec<u64>,
}
impl UnitNumberAllocator {
    /// Lowest unused unit number; 65535 is the native exhaustion sentinel.
    pub fn next_id(&self) -> u16 {
        for (slot, bits) in self.used.iter().enumerate() {
            if *bits != u64::MAX {
                return u16::try_from(
                    slot.saturating_mul(64)
                        .saturating_add((!bits).trailing_zeros() as usize)
                        .saturating_add(1),
                )
                .unwrap_or(u16::MAX);
            }
        }
        u16::try_from(self.used.len().saturating_mul(64).saturating_add(1)).unwrap_or(u16::MAX)
    }
    /// Native `UseID`: idempotently register a number; ignore reserved 0 and 65535.
    pub fn use_id(&mut self, id: u16) -> u16 {
        if id == 0 || id == u16::MAX {
            return id;
        }
        let bit = usize::from(id.saturating_sub(1));
        self.used
            .resize(self.used.len().max((bit / 64).saturating_add(1)), 0);
        if let Some(slot) = self.used.get_mut(bit / 64) {
            *slot |= 1 << (bit % 64);
        }
        id
    }
    /// Native `ReleaseID`: ignore reserved numbers and allow already-clear bits.
    /// # Errors
    /// Rejects an absent bitmap block, which would violate a native assertion.
    pub fn release_id(&mut self, id: u16) -> Result<(), UnitReleaseError> {
        if id == 0 || id == u16::MAX {
            return Ok(());
        }
        let bit = usize::from(id.saturating_sub(1));
        let slot = self.used.get_mut(bit / 64).ok_or(UnitReleaseError(id))?;
        *slot &= !(1 << (bit % 64));
        Ok(())
    }
}
