//! Observer state and semantic mutation controls; no worker scheduling controls.
use super::{Kind, recorder::Recorder};

#[test]
fn scalar_membership_lifetimes_survive_rollback_and_reuse_gets_new_id() {
    let mut recorder = Recorder::default();
    let first = recorder.attach(41, 0, Kind::OwnerFirstSeen);
    recorder.begin_candidate();
    recorder.job(41, 0, Kind::StagedRemoval);
    recorder.end_candidate(false, &[41]);
    assert_eq!(recorder.attach(41, 0, Kind::OwnerFirstSeen), first);
    recorder.begin_candidate();
    recorder.job(41, 0, Kind::StagedRemoval);
    recorder.end_candidate(true, &[]);
    assert_ne!(recorder.attach(41, 0, Kind::OwnerFirstSeen), first);
    assert!(recorder.faults.is_empty());
}

#[test]
fn extra_branch_sample_and_publication_before_wait_are_rejected() {
    let mut recorder = Recorder::default();
    recorder.attach(41, 0, Kind::OwnerFirstSeen);
    recorder.begin_candidate();
    recorder.host_enter();
    recorder.sample(Some((41, 0)), Some(true), Some(false));
    recorder.sample(Some((41, 0)), Some(true), Some(false));
    assert!(!recorder.faults.is_empty());
    let mut recorder = Recorder::default();
    recorder.attach(41, 0, Kind::OwnerFirstSeen);
    recorder.job(41, 0, Kind::PublicationBegin);
    assert!(!recorder.faults.is_empty());
}

#[test]
fn genuine_wait_order_is_accepted_and_spawn_discard_retires_only_new_membership() {
    let mut recorder = Recorder::default();
    let committed = recorder.attach(41, 0, Kind::OwnerFirstSeen);
    recorder.begin_candidate();
    let created = recorder.attach(42, 1, Kind::WorkerSpawned);
    recorder.job(42, 1, Kind::WaitBegin);
    recorder.job(42, 1, Kind::WaitReturned);
    recorder.job(42, 1, Kind::PublicationBegin);
    recorder.job(42, 1, Kind::PublicationEnd);
    recorder.end_candidate(false, &[41]);
    assert_eq!(recorder.attach(41, 0, Kind::OwnerFirstSeen), committed);
    assert_ne!(recorder.attach(42, 1, Kind::OwnerFirstSeen), created);
    assert!(recorder.faults.is_empty());
}

#[test]
fn changed_saved_identity_and_skipped_poll_with_completion_are_faults() {
    let mut recorder = Recorder::default();
    recorder.attach(41, 0, Kind::OwnerFirstSeen);
    recorder.attach(41, 1, Kind::OwnerFirstSeen);
    assert!(!recorder.faults.is_empty());
    let mut recorder = Recorder::default();
    recorder.attach(41, 0, Kind::OwnerFirstSeen);
    recorder.sample(Some((41, 0)), Some(false), Some(true));
    assert!(!recorder.faults.is_empty());
}

#[test]
fn diagnostic_finish_surfaces_faults_after_writing_artifact() -> Result<(), super::TraceError> {
    let session = super::Session::begin()?;
    assert!(matches!(
        super::Session::begin(),
        Err(super::TraceError::Active)
    ));
    super::with(|recorder| recorder.job(999, 0, Kind::WaitBegin));
    let path = std::env::temp_dir().join(format!("async-trace-fault-{}.json", std::process::id()));
    assert!(matches!(
        session.finish(&path),
        Err(super::TraceError::Faults(1))
    ));
    let bytes = std::fs::read(&path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        value.get("faults"),
        Some(&serde_json::json!(["unknown_owner"]))
    );
    std::fs::remove_file(path)?;
    Ok(())
}
