use super::{ContentError, Price, price_data::BASES};
use serde::Serialize;

/// Native cost difficulty setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Difficulty {
    /// Three quarters of the medium cost.
    Low,
    /// Native base cost.
    Medium,
    /// Nine eighths of the medium cost.
    High,
}
impl Difficulty {
    const fn factor(self) -> i64 {
        match self {
            Self::Low => 6,
            Self::Medium => 8,
            Self::High => 9,
        }
    }
}

/// Inputs to native `RecomputePrices`; inflation has 16 fractional bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PriceSettings {
    /// Construction difficulty.
    pub construction: Difficulty,
    /// Vehicle running-cost difficulty.
    pub running: Difficulty,
    /// Accumulated price inflation, bounded by native `MAX_INFLATION`.
    pub inflation_prices: u64,
    /// Accumulated cargo-payment inflation.
    pub inflation_payment: u64,
    /// Difficulty maximum loan before inflation, in pounds.
    pub max_loan: u32,
}
impl Default for PriceSettings {
    fn default() -> Self {
        Self {
            construction: Difficulty::Medium,
            running: Difficulty::Medium,
            inflation_prices: 65_536,
            inflation_payment: 65_536,
            max_loan: 300_000,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub(super) enum PriceCategory {
    Fixed,
    Construction,
    Running,
}

/// Derived vanilla prices; mutable saved economy fields remain in `World`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prices {
    values: Vec<i64>,
    /// Inflation-adjusted loan rounded down to a 10,000-pound interval.
    pub max_loan: u64,
}
impl Prices {
    pub(super) fn new(settings: PriceSettings) -> Result<Self, ContentError> {
        if settings.inflation_prices > 2_147_483_647 || settings.inflation_payment > 2_147_483_647 {
            return Err(ContentError::Inflation);
        }
        let inflation =
            i64::try_from(settings.inflation_prices).map_err(|_| ContentError::Inflation)?;
        let values = BASES
            .iter()
            .map(|&(base, category)| {
                let difficulty = match category {
                    PriceCategory::Fixed => Difficulty::Medium,
                    PriceCategory::Construction => settings.construction,
                    PriceCategory::Running => settings.running,
                };
                let value = base
                    .checked_mul(difficulty.factor())
                    .and_then(|v| v.checked_mul(inflation))
                    .ok_or(ContentError::Arithmetic)?
                    >> 19;
                Ok(if value == 0 { base.signum() } else { value })
            })
            .collect::<Result<Vec<_>, ContentError>>()?;
        let max_loan = u64::from(settings.max_loan)
            .checked_mul(settings.inflation_prices)
            .ok_or(ContentError::Arithmetic)?
            >> 16;
        Ok(Self {
            values,
            max_loan: (max_loan / 10_000)
                .checked_mul(10_000)
                .ok_or(ContentError::Arithmetic)?,
        })
    }
    /// Look up a valid native price key.
    pub fn get(&self, price: Price) -> i64 {
        // Every Price discriminant indexes the equally sized immutable native table.
        #[expect(
            clippy::indexing_slicing,
            reason = "Price is exhaustive and values contains every native price"
        )]
        self.values[usize::from(price as u8)]
    }
}
