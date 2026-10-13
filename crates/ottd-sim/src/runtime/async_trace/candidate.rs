//! Candidate observer scope is declared before candidate owners and drops after them.
use super::{Host, Kind, observe::token, with};
use crate::runtime::transport::TransportRuntime;

pub(in crate::runtime) struct Candidate {
    finished: bool,
}
impl Candidate {
    pub(in crate::runtime) fn begin(transport: &TransportRuntime) -> Self {
        with(|recorder| {
            for (id, worker) in &transport.workers {
                recorder.attach(token(worker), *id, Kind::OwnerFirstSeen);
            }
            recorder.begin_candidate();
        });
        Self { finished: false }
    }
    pub(in crate::runtime) fn prepared(&self) {
        if self.finished {
            with(|recorder| recorder.faults.push(super::model::Fault::MissingCandidate));
        } else {
            with(|recorder| recorder.emit(Kind::CandidatePrepared, None));
        }
    }
    pub(in crate::runtime) fn commit(&mut self, transport: &TransportRuntime) {
        with(|recorder| {
            let current: Vec<_> = transport.workers.values().map(token).collect();
            recorder.end_candidate(true, &current);
        });
        self.finished = true;
    }
}
impl Drop for Candidate {
    fn drop(&mut self) {
        if !self.finished {
            with(|recorder| recorder.end_candidate(false, &[]));
        }
    }
}

/// Emits refusal on any early-return path inside an actual planned host call.
impl Host {
    pub(crate) const fn begin() -> Self {
        Self { finished: false }
    }
    pub(crate) const fn finish(&mut self) {
        self.finished = true;
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        if !self.finished {
            with(|recorder| recorder.emit(Kind::InnerRefused, None));
        }
    }
}
