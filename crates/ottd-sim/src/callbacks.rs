//! Versioned operation boundary; every variant executes an implemented callback.
use crate::{
    CompanyCallbacks, HouseCallbacks, PeriodicCallbackError, StationCallbacks, run_company_year,
    run_house_year, run_station_month,
};
use crate::{IndustryCallbackError, IndustryCallbacks, IndustryMonth, run_industry_month};
use crate::{VehicleCallbackError, VehicleCallbacks, VehicleOperation, run_vehicle_callback};
use serde::{Deserialize, Serialize};

/// Typed operation paired with the state it consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Callback {
    /// Original economy MONTH/INDUSTRY callback in original economy.
    IndustryMonth {
        /// Actual callback phase, before clock accumulator reset or year rewind.
        phase: IndustryMonth,
        /// Live vanilla industry statistics and builder state.
        state: IndustryCallbacks,
    },
    /// Original economy YEAR/TOWN house age scan.
    HouseYear {
        /// Entire raw map and random state.
        state: HouseCallbacks,
    },
    /// Original economy YEAR/COMPANY expense rollover.
    CompanyYear {
        /// Company expense tables and explicit UI context.
        state: CompanyCallbacks,
    },
    /// Original economy MONTH/STATION cargo status rollover.
    StationMonth {
        /// Station cargo status arrays and random state.
        state: StationCallbacks,
    },
    /// Supported vehicle calendar aging or yearly accounting.
    Vehicle {
        /// Original callback to invoke.
        operation: VehicleOperation,
        /// Explicit vehicle and group state.
        state: VehicleCallbacks,
    },
}

/// Isolated callback request and result, not a savegame or full-world tick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallbackFixture {
    /// Currently 1.
    pub schema_version: u32,
    /// Operation and corresponding typed state.
    pub callback: Callback,
}

/// Unsupported schema or invalid callback state.
#[derive(Debug, thiserror::Error)]
pub enum CallbackError {
    /// Only the current callback boundary version is accepted.
    #[error("expected callback schema version 1")]
    Schema,
    /// Industry callback context or state was rejected.
    #[error(transparent)]
    Industry(#[from] IndustryCallbackError),
    /// Periodic bookkeeping context or state was rejected.
    #[error(transparent)]
    Periodic(#[from] PeriodicCallbackError),
    /// Vehicle context or state was rejected.
    #[error(transparent)]
    Vehicle(#[from] VehicleCallbackError),
}

/// Executes one supported callback, retaining its operation in the result.
/// # Errors
/// Rejects unsupported schemas, context, or inconsistent object references.
pub fn simulate_callback(fixture: CallbackFixture) -> Result<CallbackFixture, CallbackError> {
    if fixture.schema_version != 1 {
        return Err(CallbackError::Schema);
    }
    let callback = match fixture.callback {
        Callback::IndustryMonth { phase, state } => Callback::IndustryMonth {
            phase,
            state: run_industry_month(state, phase)?,
        },
        Callback::HouseYear { state } => Callback::HouseYear {
            state: run_house_year(state)?,
        },
        Callback::CompanyYear { state } => Callback::CompanyYear {
            state: run_company_year(state)?,
        },
        Callback::StationMonth { state } => Callback::StationMonth {
            state: run_station_month(state)?,
        },
        Callback::Vehicle { operation, state } => Callback::Vehicle {
            operation,
            state: run_vehicle_callback(state, operation)?,
        },
    };
    Ok(CallbackFixture {
        schema_version: 1,
        callback,
    })
}
