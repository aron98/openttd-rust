use super::{
    Action, Domain, Manifest, Native, Protocol, Result, Value, corpus, equal, io,
    model::NativeAction, replay,
};
use crate::{
    Command, CommandError, CommandMode, CommandRequest, execute_command, runtime::SimulationRuntime,
};
use ottd_save::{WireValue, world::PathElement};
use serde_json::json;

#[test]
#[ignore = "requires frozen native24 corpus and a fresh TREE_CORPUS_OUTPUT directory"]
fn native_corpus_explicit_admission_boundaries() -> Result {
    let directory = corpus()?;
    let manifest: Manifest = io::json(&directory.join("draft/native-cases.json"))?;
    let mut count = 0_usize;
    for case in manifest.cases {
        let reason = match case.domain {
            Domain::UnsupportedDeity => "deity construction",
            Domain::UnsupportedBridge => "terraform beneath bridge",
            Domain::UnsupportedWater => "clearing non-clear terrain",
            Domain::TerrainParity
            | Domain::LoaderClampBoundary
            | Domain::EarlyCompanyGate
            | Domain::ObserverOffControl => continue,
        };
        let protocol: Protocol = io::json(&case.directory.join("actions.json"))?;
        let mut world = io::load(&case.directory.join("initial.sav"))?;
        let before = world.saved_json()?;
        let derived = serde_json::to_value(world.derived())?;
        for action in protocol.actions {
            let Action::Command { ordinal, request } = action else {
                return Err("unexpected boundary checkpoint".into());
            };
            let error = execute_command(&mut world, &request)
                .err()
                .ok_or("unsupported boundary was admitted")?;
            io::record(
                &format!("boundary-{}-{ordinal}", case.id),
                &json!({"error":error.to_string(), "native_parity":false}),
            )?;
            assert!(matches!(error, CommandError::Unsupported(actual) if actual == reason));
            equal(
                &world.saved_json()?,
                &before,
                &case.id,
                "unsupported saved world",
            )?;
            equal(
                &serde_json::to_value(world.derived())?,
                &derived,
                &case.id,
                "unsupported derived state",
            )?;
        }
        count = count.saturating_add(1);
    }
    assert_eq!(count, 3);
    Ok(())
}

#[test]
#[ignore = "requires frozen native24 corpus and a fresh TREE_CORPUS_OUTPUT directory"]
fn native_corpus_company_gate_receipt_without_claiming_trace_parity() -> Result {
    let run = corpus()?.join("runs/company-15");
    let protocol: Protocol = io::json(&run.join("actions.json"))?;
    let native: Native = io::json(&run.join("results.json"))?;
    assert_eq!(protocol.actions.len(), native.actions.len());
    let mut world = io::load(&run.join("initial.sav"))?;
    for (action, expected) in protocol.actions.iter().zip(&native.actions) {
        let (
            Action::Command { ordinal, request },
            NativeAction::Command {
                receipt,
                native_metadata,
                ..
            },
        ) = (action, expected)
        else {
            return Err("unexpected company-gate action".into());
        };
        let actual = execute_command(&mut world, request)?;
        equal(
            &serde_json::to_value(&actual)?,
            receipt,
            "company-15",
            "gate receipt",
        )?;
        let trace = native_metadata
            .tree_rating
            .as_ref()
            .ok_or("native gate trace missing")?;
        assert_eq!(
            trace.events,
            json!([{"kind":"phase_complete", "ordinal":0, "phase":"result"}])
        );
        io::record(
            &format!("company-gate-{ordinal}"),
            &json!({"receipt":actual,"trace_parity":false,"reason":"actual public result-transition observer hook pending root review"}),
        )?;
    }
    io::checkpoint(&world, &run, "final", "company-15")
}

#[test]
#[ignore = "requires frozen native24 corpus and a fresh TREE_CORPUS_OUTPUT directory"]
fn native_corpus_loader_and_observer_boundaries() -> Result {
    let directory = corpus()?;
    for (fixture, command, requested) in [
        ("fixture-threshold-zero", "threshold-zero", 0),
        ("fixture-contiguous-tie-v2", "contiguous-tie", 2),
    ] {
        let raw: Value = io::json(&directory.join(format!("runs/{fixture}/final.world.json")))?;
        let canonical: Value =
            io::json(&directory.join(format!("runs/{command}/initial.world.json")))?;
        let field = "/chunks/PATS/records/0/economy.dist_local_authority";
        assert_eq!(raw.pointer(field), Some(&json!(requested)));
        assert_eq!(canonical.pointer(field), Some(&json!(5)));
        let mut normalized = raw;
        *normalized
            .pointer_mut(field)
            .ok_or("setting field missing")? = json!(5);
        assert_eq!(
            normalized, canonical,
            "only original load clamp changes input"
        );
    }
    let result = replay::run(
        &directory.join("runs/threshold-zero"),
        true,
        "canonical-loader-five",
    )?;
    io::record("canonical-loader-five", &result)?;
    let result = replay::run(
        &directory.join("runs/coastal-clear-off"),
        false,
        "observer-off",
    )?;
    io::record("observer-off", &result)
}

fn raw_distance_requires_admission(distance: u8) -> Result {
    let directory = corpus()?;
    let input = directory.join("runs/fixture-threshold-zero/final.sav");
    let native_after: Value = io::json(&directory.join("runs/threshold-zero/final.world.json"))?;
    let native_rating = native_after
        .pointer("/chunks/CITY/records/0/ratings/0")
        .ok_or("native rating missing")?;
    assert_eq!(native_rating, &json!(465));
    assert_eq!(
        native_after.pointer("/chunks/PATS/records/0/economy.dist_local_authority"),
        Some(&json!(5))
    );
    let mut failures = Vec::new();
    for mode in [CommandMode::Estimate, CommandMode::Post] {
        let mut world = io::load(&input)?;
        world.edit_field(
            *b"PATS",
            0,
            &[PathElement::Field("economy.dist_local_authority".into())],
            WireValue::Unsigned(u64::from(distance)),
        )?;
        let before = world.saved_json()?;
        let before_derived = serde_json::to_value(world.derived())?;
        // Use exactly the same typed edited world for both public command paths.
        let mut runtime_world = io::load(&input)?;
        runtime_world.edit_field(
            *b"PATS",
            0,
            &[PathElement::Field("economy.dist_local_authority".into())],
            WireValue::Unsigned(u64::from(distance)),
        )?;
        let mut runtime = SimulationRuntime::restore_vanilla(runtime_world)?;
        equal(
            &runtime.world().saved_json()?,
            &before,
            "raw setting",
            "restore preserves noncanonical setting",
        )?;
        let request = CommandRequest {
            company: 0,
            mode,
            command: Command::LandscapeClear { tile: 650 },
        };
        let free_result = execute_command(&mut world, &request);
        let runtime_result = runtime.execute_command(&request);
        let admitted = |result: &std::result::Result<crate::CommandReceipt, CommandError>| {
            matches!(
                result,
                Err(CommandError::Unsupported(
                    "noncanonical town authority distance"
                ))
            )
        };
        let unchanged = world.saved_json()? == before
            && runtime.world().saved_json()? == before
            && serde_json::to_value(world.derived())? == before_derived
            && serde_json::to_value(runtime.world().derived())? == before_derived;
        let free_observation = match &free_result {
            Ok(receipt) => json!({"receipt":receipt}),
            Err(error) => json!({"error":error.to_string()}),
        };
        let runtime_observation = match &runtime_result {
            Ok(receipt) => json!({"receipt":receipt}),
            Err(error) => json!({"error":error.to_string()}),
        };
        let mode_name = match mode {
            CommandMode::Estimate => "estimate",
            CommandMode::Post => "post",
        };
        io::record(
            &format!("raw-{distance}-{mode_name}"),
            &json!({
                "raw_distance":distance, "mode":mode_name, "free":free_observation, "runtime":runtime_observation,
                "unchanged":unchanged, "saved_rating":world.saved_json()?.pointer("/chunks/CITY/records/0/ratings/0"),
                "native_loader_distance":5, "retained_threshold_zero_native_post_rating":native_rating,
            }),
        )?;
        if !admitted(&free_result) || !admitted(&runtime_result) || !unchanged {
            failures.push(mode_name);
        }
    }
    assert!(
        failures.is_empty(),
        "raw distance{distance}: public paths must reject before mutation; failed modes={failures:?}"
    );
    Ok(())
}

#[test]
#[ignore = "expected red until explicitly allocated admission fix; requires frozen corpus"]
fn raw_zero_authority_distance_requires_typed_admission() -> Result {
    raw_distance_requires_admission(0)
}
#[test]
#[ignore = "expected red until explicitly allocated admission fix; requires frozen corpus"]
fn raw_two_authority_distance_requires_typed_admission() -> Result {
    raw_distance_requires_admission(2)
}

#[test]
#[ignore = "requires frozen corpus; protects the preexisting non-tree command domain"]
fn noncanonical_authority_distance_does_not_extend_old_non_tree_refusals() -> Result {
    let input = corpus()?.join("runs/threshold-zero/initial.sav");
    let commands = [
        Command::LandscapeClear { tile: 1000 },
        Command::TerraformLand {
            tile: 1000,
            slope: 8,
            dir_up: true,
        },
        Command::LevelLand {
            tile: 1000,
            start_tile: 1000,
            diagonal: false,
            level_mode: 2,
        },
    ];
    for (ordinal, command) in commands.into_iter().enumerate() {
        for distance in [0_u8, 2] {
            let mut canonical = io::load(&input)?;
            let mut raw = io::load(&input)?;
            raw.edit_field(
                *b"PATS",
                0,
                &[PathElement::Field("economy.dist_local_authority".into())],
                WireValue::Unsigned(u64::from(distance)),
            )?;
            let request = CommandRequest {
                company: 0,
                mode: CommandMode::Post,
                command: command.clone(),
            };
            let expected = execute_command(&mut canonical, &request)?;
            let actual = execute_command(&mut raw, &request)?;
            assert_eq!(actual, expected);
            raw.edit_field(
                *b"PATS",
                0,
                &[PathElement::Field("economy.dist_local_authority".into())],
                WireValue::Unsigned(5),
            )?;
            equal(
                &raw.saved_json()?,
                &canonical.saved_json()?,
                "non-tree",
                "setting-isolated saved world",
            )?;
            equal(
                &serde_json::to_value(raw.derived())?,
                &serde_json::to_value(canonical.derived())?,
                "non-tree",
                "setting-isolated derived state",
            )?;
            io::record(
                &format!("non-tree-{ordinal}-{distance}"),
                &json!({"receipt":actual,"raw_setting":distance,"setting_isolated_state_equal":true,"native_raw_load_parity":false}),
            )?;
        }
    }
    Ok(())
}
