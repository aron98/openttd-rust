use super::RuntimeError;
use crate::world_access::{signed, unsigned};
use ottd_save::world::World;

/// Borrowed dynamic ENGN state; static engine properties live in the catalog.
#[derive(Debug, Clone, Copy)]
pub struct SavedEngineView<'a> {
    world: &'a World,
    id: u16,
}
impl<'a> SavedEngineView<'a> {
    pub(crate) fn new(world: &'a World, id: u16) -> Result<Self, RuntimeError> {
        if !world
            .tables()
            .get(b"ENGN")
            .is_some_and(|t| t.records().contains_key(&u32::from(id)))
        {
            return Err(RuntimeError::Invalid("engine ID"));
        }
        Ok(Self { world, id })
    }
    /// Saved reliability used when building or servicing a vehicle.
    /// # Errors
    /// Rejects an invalid saved reliability field.
    pub fn reliability(self) -> Result<u16, RuntimeError> {
        self.word("reliability")
    }
    /// Saved rate of reliability decay.
    /// # Errors
    /// Rejects an invalid saved decay field.
    pub fn reliability_decay(self) -> Result<u16, RuntimeError> {
        self.word("reliability_spd_dec")
    }
    /// Saved company availability bitmask.
    /// # Errors
    /// Rejects an invalid saved availability field.
    pub fn company_availability(self) -> Result<u16, RuntimeError> {
        self.word("company_avail")
    }
    /// Saved availability and exclusive-preview flags used by live construction.
    /// # Errors
    /// Rejects malformed saved flag bits.
    pub fn flags(self) -> Result<u8, RuntimeError> {
        u8::try_from(self.word("flags")?).map_err(|_| RuntimeError::Invalid("ENGN/flags"))
    }
    /// Saved engine-model age in months.
    /// # Errors
    /// Rejects an invalid saved age field.
    pub fn age(self) -> Result<i32, RuntimeError> {
        let value = signed(self.world, b"ENGN", u32::from(self.id), "age")
            .map_err(|_| RuntimeError::Invalid("ENGN/age"))?;
        i32::try_from(value).map_err(|_| RuntimeError::Invalid("ENGN/age"))
    }
    fn word(self, name: &'static str) -> Result<u16, RuntimeError> {
        let value = unsigned(self.world, b"ENGN", u32::from(self.id), name)
            .map_err(|_| RuntimeError::Invalid(name))?;
        u16::try_from(value).map_err(|_| RuntimeError::Invalid(name))
    }
}
