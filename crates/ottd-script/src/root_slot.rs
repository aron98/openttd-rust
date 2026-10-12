//! Persistent runner payloads keep one owner behind an explicit assignment boundary.
use crate::RootSlot;
use std::fmt::{Debug, Formatter, Result};

impl<T> RootSlot<T> {
    pub(super) const fn new(value: T) -> Self {
        Self { value }
    }
    pub(super) const fn as_ref(&self) -> &T {
        &self.value
    }
    pub(super) fn set(&mut self, value: T) {
        self.value = value;
    }
}
impl<T: Debug> Debug for RootSlot<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        self.value.fmt(formatter)
    }
}
