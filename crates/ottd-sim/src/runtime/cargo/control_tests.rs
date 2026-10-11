//! Labelled synthetic controls invoking unchanged original cargo consumers.
use crate::{
    WorldTickState,
    cargo::{CargoSource, Production, TileArea},
    content::ContentCatalog,
    runtime::{pools::PoolAllocator, transport::TransportRuntime},
};
use ottd_save::{Savegame, world::World};
use std::path::{Path, PathBuf};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn json(path: &Path) -> Result<serde_json::Value> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
#[test]
#[ignore = "requires SYNTHETIC_CARGO_NATIVE and SYNTHETIC_CARGO_OUTPUT; not genuine gameplay fixtures"]
fn original_distribution_and_rating_edge_matrix() -> Result {
    let input = PathBuf::from(std::env::var("SYNTHETIC_CARGO_NATIVE")?);
    let output = PathBuf::from(std::env::var("SYNTHETIC_CARGO_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let cases: Vec<String> = serde_json::from_slice(&std::fs::read(input.join("cases.json"))?)?;
    let mut results = Vec::new();
    for name in cases {
        let native = input.join(&name).join("invoke");
        let mut world = World::decode(&Savegame::decode(
            &std::fs::read(native.join("initial.sav"))?,
            ottd_save::DEFAULT_MAX_BYTES,
        )?)?;
        let content = ContentCatalog::from_world(&world)?;
        let mut transport = TransportRuntime::restore(&world, &content)?;
        let before = world.saved_json()?;
        let protocol = json(&native.join("actions.json"))?;
        let control = protocol
            .pointer("/actions/0/control")
            .ok_or("native control")?;
        let original = json(&native.join("results.json"))?;
        let industries = crate::runtime::IndustryCaches::restore(&world)?;
        let mut state = WorldTickState::load(&mut world, None, industries);
        let outcome = invoke(
            &mut state,
            &mut transport,
            &content,
            control,
            &original,
            &name,
        )?;
        transport.validate(state.view())?;
        assert_eq!(
            Some(&transport.snapshot()),
            original.pointer("/actions/0/movement/transport"),
            "{name} complete transport graph/payment/job pools"
        );
        std::fs::write(
            output.join(format!("{name}-transport.json")),
            serde_json::to_vec(&transport.snapshot())?,
        )?;
        let (candidate, _) = state.finish()?;
        candidate.commit();
        let actual = world.saved_json()?;
        let expected = json(&native.join("final.world.json"))?;
        std::fs::write(
            output.join(format!("{name}-actual-world.json")),
            serde_json::to_vec(&actual)?,
        )?;
        assert_eq!(actual, expected, "{name} complete World");
        assert_eq!(
            serde_json::to_value(world.derived())?,
            json(&native.join("final.derived.json"))?,
            "{name} derived World"
        );
        let live = json(&native.join("final.cargo.json"))?;
        std::fs::write(
            output.join(format!("{name}-cargo.json")),
            serde_json::to_vec(&transport.cargo.snapshot())?,
        )?;
        assert_eq!(
            transport.cargo.snapshot(),
            live,
            "{name} cargo lists/pool/cache"
        );
        assert_eq!(
            before != actual,
            control["expected_change"]
                .as_bool()
                .ok_or("expected change")?,
            "{name} expected mutation"
        );
        results.push(serde_json::json!({"case":name,"synthetic":true,"actual_original_moved":outcome.moved,"capacity_predicate":outcome.capacity,"full_world_derived_cargo_equal":true}));
        std::fs::write(
            output.join("PASS.json"),
            serde_json::to_vec_pretty(&results)?,
        )?;
    }
    Ok(())
}

struct Invocation {
    moved: Option<u32>,
    capacity: Option<serde_json::Value>,
}

fn invoke(
    state: &mut WorldTickState<'_>,
    transport: &mut TransportRuntime,
    content: &ContentCatalog,
    control: &serde_json::Value,
    original: &serde_json::Value,
    name: &str,
) -> Result<Invocation> {
    Ok(match control["kind"].as_str().ok_or("control kind")? {
        "rating" => {
            transport.cargo.rate_station(
                state,
                u32::try_from(control["station"].as_u64().ok_or("station")?)?,
                content,
            )?;
            Invocation {
                moved: None,
                capacity: None,
            }
        }
        "distribution" => {
            let stations: Vec<u32> = serde_json::from_value(control["stations"].clone())?;
            let old_pool = transport.cargo.pool.clone();
            if control["exhausted"].as_bool() == Some(true) {
                // Synthetic capacity predicate only; restore real metadata before comparison.
                transport.cargo.pool = PoolAllocator::restore(1, 1024, [0])?;
            }
            let predicate_before = transport.cargo.pool.snapshot();
            let can_allocate = transport.cargo.pool.can_allocate(1);
            let amount = transport.distribute(
                state,
                Production {
                    source: CargoSource::Town(0),
                    area: TileArea {
                        tile: 1958,
                        width: 1,
                        height: 1,
                    },
                    cargo: u8::try_from(control["cargo"].as_u64().unwrap_or(0))?,
                    amount: u32::try_from(control["amount"].as_u64().ok_or("amount")?)?,
                },
                &stations,
                u8::try_from(control["exclusive"].as_u64().unwrap_or(255))?,
            )?;
            let capacity = if control["exhausted"].as_bool() == Some(true) {
                let predicate_after = transport.cargo.pool.snapshot();
                assert!(!can_allocate);
                assert_eq!(predicate_before, predicate_after);
                let real_before = old_pool.snapshot();
                transport.cargo.pool = old_pool;
                Some(serde_json::json!({"synthetic_capacity_predicate_only":true,
                    "temporary_limit":1,"growth":1024,"can_allocate":can_allocate,
                    "real_before":real_before,"predicate_before":predicate_before,
                    "predicate_after":predicate_after,"real_restored":transport.cargo.pool.snapshot()}))
            } else {
                None
            };
            assert_eq!(
                Some(u64::from(amount)),
                original
                    .pointer("/actions/0/control/moved")
                    .and_then(serde_json::Value::as_u64),
                "{name}"
            );
            Invocation {
                moved: Some(amount),
                capacity,
            }
        }
        _ => return Err("unknown native synthetic operation".into()),
    })
}
