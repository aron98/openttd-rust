//! Versioned operation boundary; every variant executes an implemented callback.
use crate::{VehicleCallbackError, VehicleCallbacks, VehicleOperation, run_vehicle_callback};
use serde::{Deserialize, Serialize};

/// Typed operation paired with the state it consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Callback {
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
