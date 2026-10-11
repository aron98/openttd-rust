//! Offline native after-load and pre-state-loop link job pause control.
use super::{RuntimeError, transport::TransportRuntime};
use crate::{
    WorldTickError, WorldTickState,
    world_access::{field_edit, row_field},
};
use ottd_save::{
    WireValue,
    world::{CandidateView, World},
};

struct Context<'a> {
    view: CandidateView<'a>,
}
impl Context<'_> {
    fn value(&self, chunk: [u8; 4], id: u32, name: &str) -> Result<&WireValue, RuntimeError> {
        let table = self
            .view
            .table(chunk)
            .ok_or(RuntimeError::Invalid("job pause table"))?;
        let record = table
            .record(id)
            .ok_or(RuntimeError::Invalid("job pause record"))?;
        row_field(table.schema(), record, name)
            .map_err(|_| RuntimeError::Invalid("job pause field"))
    }
    fn number(&self, chunk: [u8; 4], id: u32, name: &str) -> Result<i64, RuntimeError> {
        match self.value(chunk, id, name)? {
            WireValue::Unsigned(value) => {
                i64::try_from(*value).map_err(|_| RuntimeError::Invalid("job pause number"))
            }
            WireValue::Signed(value) => Ok(*value),
            _ => Err(RuntimeError::Invalid("job pause numeric field")),
        }
    }
    fn unfinished_due(&self, transport: &TransportRuntime) -> Result<bool, RuntimeError> {
        let WireValue::Array(running) = self.value(*b"LGRS", 0, "running")? else {
            return Err(RuntimeError::Invalid("job running list"));
        };
        let Some(first) = running.first() else {
            return Ok(false);
        };
        let WireValue::Unsigned(pointer) = first else {
            return Err(RuntimeError::Invalid("job pointer"));
        };
        let id = u32::try_from(
            pointer
                .checked_sub(1)
                .ok_or(RuntimeError::Invalid("null job pointer"))?,
        )
        .map_err(|_| RuntimeError::Invalid("job pointer width"))?;
        if self.number(*b"LGRJ", id, "join_date")? > self.number(*b"DATE", 0, "economy_date")? {
            return Ok(false);
        }
        let worker = transport
            .workers
            .get(&id)
            .ok_or(RuntimeError::Invalid("missing due worker"))?;
        Ok(worker.poll()?.is_none())
    }
}

pub(super) fn after_load(
    world: &mut World,
    transport: &TransportRuntime,
) -> Result<(), RuntimeError> {
    // Command-only runtimes retain opaque populated jobs. Their simulation consumer is
    // rejected explicitly below; it must not remove unrelated sale/order capabilities.
    if transport.job_expectations.values().any(Option::is_none) {
        return Ok(());
    }
    let mut transaction = world.transaction();
    let context = Context {
        view: transaction.view(),
    };
    if context.unfinished_due(transport)? {
        let pause = context.number(*b"DATE", 0, "pause_mode")? | 64;
        transaction.apply(field_edit(
            *b"DATE",
            0,
            "pause_mode",
            WireValue::Unsigned(
                u64::try_from(pause).map_err(|_| RuntimeError::Invalid("pause bits"))?,
            ),
        ))?;
    }
    transaction.prepare()?.commit();
    Ok(())
}

pub(super) fn before_tick(
    state: &mut WorldTickState<'_>,
    transport: &TransportRuntime,
) -> Result<(), WorldTickError> {
    let context = Context { view: state.view() };
    let pause = context.number(*b"DATE", 0, "pause_mode")?;
    if pause != 0 && pause & 64 == 0 {
        return Ok(());
    }
    if transport.job_expectations.values().any(Option::is_none) {
        return Err(RuntimeError::Unsupported("populated link job execution").into());
    }
    let changed = if pause & 64 != 0 {
        if context.unfinished_due(transport)? {
            None
        } else {
            let cleared = pause & !64;
            Some(if cleared == 128 { 0 } else { cleared })
        }
    } else if pause == 0 && context.number(*b"DATE", 0, "economy_date_fract")? == 19 {
        let interval = context.number(*b"PATS", 0, "linkgraph.recalc_interval")? / 2;
        if interval <= 0 {
            return Err(RuntimeError::Invalid("job pause interval").into());
        }
        if context
            .number(*b"DATE", 0, "economy_date")?
            .checked_rem(interval)
            .ok_or(RuntimeError::Invalid("job pause remainder"))?
            == interval / 2
            && context.unfinished_due(transport)?
        {
            Some(64)
        } else {
            None
        }
    } else {
        None
    };
    if let Some(pause) = changed {
        state.set_number(b"DATE", 0, "pause_mode", pause)?;
    }
    Ok(())
}
#[cfg(test)]
mod prejoin_tests;
#[cfg(test)]
mod tests;
