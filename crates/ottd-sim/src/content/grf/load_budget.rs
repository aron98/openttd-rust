use super::load_types::{ControlLoadError, ControlOptions, LoadEvent, LoadLocation};

pub(super) struct Budget {
    options: ControlOptions,
    steps: usize,
    overrides: usize,
    labels: usize,
    events: usize,
    trace_bytes: usize,
}
impl Budget {
    pub(super) const fn new(options: ControlOptions) -> Self {
        Self {
            options,
            steps: 0,
            overrides: 0,
            labels: 0,
            events: 0,
            trace_bytes: 0,
        }
    }
    fn charge(
        used: &mut usize,
        amount: usize,
        limit: usize,
        location: LoadLocation,
        resource: &'static str,
    ) -> Result<(), ControlLoadError> {
        *used = used
            .checked_add(amount)
            .ok_or(ControlLoadError::ResourceLimit { location, resource })?;
        if *used > limit {
            return Err(ControlLoadError::ResourceLimit { location, resource });
        }
        Ok(())
    }
    pub(super) fn step(&mut self, location: LoadLocation) -> Result<(), ControlLoadError> {
        Self::charge(
            &mut self.steps,
            1,
            self.options.max_steps,
            location,
            "record visits",
        )
    }
    pub(super) fn label(&mut self, location: LoadLocation) -> Result<(), ControlLoadError> {
        Self::charge(
            &mut self.labels,
            1,
            self.options.max_labels,
            location,
            "labels",
        )
    }
    pub(super) fn overrides(
        &mut self,
        amount: usize,
        location: LoadLocation,
    ) -> Result<(), ControlLoadError> {
        Self::charge(
            &mut self.overrides,
            amount,
            self.options.max_override_bytes,
            location,
            "override work bytes",
        )
    }
    pub(super) fn trace(
        &mut self,
        bytes: usize,
        location: LoadLocation,
    ) -> Result<(), ControlLoadError> {
        Self::charge(
            &mut self.events,
            1,
            self.options.max_trace_events,
            location,
            "trace events",
        )?;
        self.payload(
            bytes.saturating_add(std::mem::size_of::<LoadEvent>()),
            location,
        )
    }
    pub(super) fn payload(
        &mut self,
        bytes: usize,
        location: LoadLocation,
    ) -> Result<(), ControlLoadError> {
        Self::charge(
            &mut self.trace_bytes,
            bytes,
            self.options.max_trace_bytes,
            location,
            "trace payload bytes",
        )
    }
}
