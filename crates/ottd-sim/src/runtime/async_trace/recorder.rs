//! Main-thread scalar membership tracking; never owns or dereferences a worker.
use super::{
    Kind,
    model::{Clock, Event, Fault, Identity},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct Membership {
    identity: Identity,
    phase: Phase,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Attached,
    Waiting,
    Waited,
    Publishing,
    Published,
}
#[derive(Debug, Default)]
pub(super) struct Recorder {
    pub events: Vec<Event>,
    pub faults: Vec<Fault>,
    owners: BTreeMap<usize, Membership>,
    original: BTreeSet<usize>,
    candidate: Option<u64>,
    next_candidate: u64,
    next_lifetime: u64,
    host: u64,
    sampled: bool,
}
impl Recorder {
    pub(super) fn emit(&mut self, kind: Kind, clock: Option<Clock>) {
        self.events.push(Event {
            sequence: self.events.len(),
            host: self.host,
            candidate: self.candidate,
            kind,
            clock,
            owner: None,
            due: None,
            completed: None,
        });
    }
    pub(super) fn attach(&mut self, token: usize, job: u32, kind: Kind) -> u64 {
        if let Some(owner) = self.owners.get(&token) {
            if owner.identity.saved_job != job {
                self.faults.push(Fault::IdentityChanged);
            }
            return owner.identity.lifetime;
        }
        self.next_lifetime = self.next_lifetime.saturating_add(1);
        let identity = Identity {
            lifetime: self.next_lifetime,
            saved_job: job,
        };
        self.owners.insert(
            token,
            Membership {
                identity,
                phase: Phase::Attached,
            },
        );
        self.job(token, job, kind);
        identity.lifetime
    }
    pub(super) fn begin_candidate(&mut self) {
        if self.candidate.is_some() {
            self.faults.push(Fault::NestedCandidate);
        }
        self.next_candidate = self.next_candidate.saturating_add(1);
        self.candidate = Some(self.next_candidate);
        self.original = self.owners.keys().copied().collect();
        for owner in self.owners.values_mut() {
            owner.phase = Phase::Attached;
        }
        self.emit(Kind::CandidateBegin, None);
    }
    pub(super) fn end_candidate(&mut self, committed: bool, current: &[usize]) {
        if self.candidate.is_none() {
            self.faults.push(Fault::MissingCandidate);
        }
        self.emit(
            if committed {
                Kind::CandidateCommitted
            } else {
                Kind::CandidateRefused
            },
            None,
        );
        let removed: Vec<_> = self
            .owners
            .keys()
            .copied()
            .filter(|token| {
                if committed {
                    !current.contains(token)
                } else {
                    !self.original.contains(token)
                }
            })
            .collect();
        for token in removed {
            if let Some(owner) = self.owners.get(&token) {
                self.job(
                    token,
                    owner.identity.saved_job,
                    if committed {
                        Kind::CommittedRelease
                    } else {
                        Kind::CandidateDiscard
                    },
                );
            }
            self.owners.remove(&token);
        }
        self.candidate = None;
        self.original.clear();
    }
    pub(super) const fn host_enter(&mut self) {
        self.host = self.host.saturating_add(1);
        self.sampled = false;
    }
    pub(super) fn sample(
        &mut self,
        owner: Option<(usize, u32)>,
        due: Option<bool>,
        completed: Option<bool>,
    ) {
        if self.sampled {
            self.faults.push(Fault::DuplicateSample);
        }
        self.sampled = true;
        if completed.is_some() && (due != Some(true) || owner.is_none()) {
            self.faults.push(Fault::SampleShape);
        }
        self.emit(Kind::BranchSample, None);
        let identity = owner.and_then(|(token, job)| {
            let member = self.owners.get(&token);
            if member.is_none() {
                self.faults.push(Fault::UnknownOwner);
            }
            member.map(|member| Identity {
                saved_job: job,
                ..member.identity
            })
        });
        if let Some(event) = self.events.last_mut() {
            event.owner = identity;
            event.due = due;
            event.completed = completed;
        }
    }
    pub(super) fn job(&mut self, token: usize, job: u32, kind: Kind) {
        let Some(owner) = self.owners.get_mut(&token) else {
            self.faults.push(Fault::UnknownOwner);
            return;
        };
        if owner.identity.saved_job != job {
            self.faults.push(Fault::IdentityChanged);
        }
        match kind {
            Kind::WaitBegin => owner.phase = Phase::Waiting,
            Kind::WaitReturned => {
                if owner.phase != Phase::Waiting {
                    self.faults.push(Fault::WaitOrder);
                }
                owner.phase = Phase::Waited;
            }
            Kind::PublicationBegin => {
                if owner.phase != Phase::Waited {
                    self.faults.push(Fault::PublicationBeforeWait);
                }
                owner.phase = Phase::Publishing;
            }
            Kind::PublicationEnd => {
                if owner.phase != Phase::Publishing {
                    self.faults.push(Fault::WaitOrder);
                }
                owner.phase = Phase::Published;
            }
            _ => {}
        }
        let identity = owner.identity;
        self.emit(kind, None);
        if let Some(event) = self.events.last_mut() {
            event.owner = Some(identity);
        }
        if kind == Kind::StagedRemoval && !self.original.contains(&token) {
            self.owners.remove(&token);
        }
    }
}
