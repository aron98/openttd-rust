//! Native multiresolution history rotation, shared by the two industry records.
#![expect(
    clippy::redundant_pub_crate,
    reason = "history helpers remain internal to callback modules"
)]
use crate::{AcceptedHistory, IndustryCallbackError, ProducedHistory};

// Native monthly[1,25), quarterly[25,42), yearly[42,61) ranges.
const RANGES: [(usize, usize, usize, u8); 3] = [(1, 25, 1, 1), (25, 42, 3, 3), (42, 61, 4, 12)];

pub(crate) trait Record: Copy + Default {
    fn values(self) -> [u16; 2];
    fn from_values(values: [u16; 2]) -> Self;
}
impl Record for ProducedHistory {
    fn values(self) -> [u16; 2] {
        [self.production, self.transported]
    }
    fn from_values([production, transported]: [u16; 2]) -> Self {
        Self {
            production,
            transported,
        }
    }
}
impl Record for AcceptedHistory {
    fn values(self) -> [u16; 2] {
        [self.accepted, self.waiting]
    }
    fn from_values([accepted, waiting]: [u16; 2]) -> Self {
        Self { accepted, waiting }
    }
}
pub(crate) fn update_valid(mut mask: u64, month: u8) -> Result<u64, IndustryCallbackError> {
    for (first, last, division, total) in RANGES {
        if mask & (1_u64 << last.saturating_sub(1)) != 0
            || month
                .checked_rem(total)
                .ok_or(IndustryCallbackError::State)?
                != 0
            || division != 1 && mask & (1_u64 << first.saturating_sub(division)) == 0
        {
            continue;
        }
        let bits = (1_u64 << last.saturating_sub(first)).saturating_sub(1) << first;
        mask = (mask & !bits) | (((mask << 1) | (1_u64 << first)) & bits);
    }
    Ok(mask)
}
pub(crate) fn rotate<T: Record>(
    history: &mut [T],
    mask: u64,
    month: u8,
) -> Result<(), IndustryCallbackError> {
    for (first, last, division, total) in RANGES {
        if month
            .checked_rem(total)
            .ok_or(IndustryCallbackError::State)?
            != 0
        {
            continue;
        }
        let range = history
            .get_mut(first..last)
            .ok_or(IndustryCallbackError::State)?;
        range.copy_within(..range.len().saturating_sub(1), 1);
        if total == 1 {
            let current = history
                .first()
                .copied()
                .ok_or(IndustryCallbackError::State)?;
            *history.get_mut(first).ok_or(IndustryCallbackError::State)? = current;
            *history.first_mut().ok_or(IndustryCallbackError::State)? = T::default();
        } else if mask & (1_u64 << first.saturating_sub(division)) != 0 {
            let source = history
                .get(first.saturating_sub(division)..first)
                .ok_or(IndustryCallbackError::State)?;
            let [a, b] = source.iter().fold([0_u32; 2], |[a, b], item| {
                let [x, y] = item.values();
                [
                    a.saturating_add(u32::from(x)),
                    b.saturating_add(u32::from(y)),
                ]
            });
            let count = u32::try_from(division).map_err(|_| IndustryCallbackError::State)?;
            let a = u16::try_from(a.checked_div(count).ok_or(IndustryCallbackError::State)?)
                .map_err(|_| IndustryCallbackError::State)?;
            let b = u16::try_from(b.checked_div(count).ok_or(IndustryCallbackError::State)?)
                .map_err(|_| IndustryCallbackError::State)?;
            *history.get_mut(first).ok_or(IndustryCallbackError::State)? = T::from_values([a, b]);
        }
    }
    Ok(())
}
