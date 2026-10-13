//! Labelled synthetic arithmetic/path controls; these are not native route fixtures.
use super::{
    dijkstra::Search,
    model::{Distribution, Edge, JobInput, Node, Settings},
    paths::{self, Metric},
};
use std::sync::atomic::AtomicBool;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;
fn input() -> JobInput {
    let edge = |destination| Edge {
        destination,
        capacity: 100,
        usage: 9,
        travel_time: 26,
    };
    JobInput {
        graph: 0,
        cargo: 0,
        width: 64,
        height: 64,
        express: true,
        settings: Settings {
            distribution: Distribution::Manual,
            accuracy: 16,
            demand_distance: 100,
            demand_size: 100,
            saturation: 80,
            recalc_time: 64,
        },
        join_date: 100,
        last_compression: 60,
        nodes: vec![
            Node {
                station: 5,
                xy: 0,
                supply: 73,
                demand: 5,
                edges: vec![edge(1), edge(2)],
            },
            Node {
                station: 4,
                xy: 1,
                supply: 0,
                demand: 1,
                edges: vec![edge(3)],
            },
            Node {
                station: 3,
                xy: 64,
                supply: 9,
                demand: 2,
                edges: vec![edge(3)],
            },
            Node {
                station: 2,
                xy: 65,
                supply: 0,
                demand: 1,
                edges: Vec::new(),
            },
        ],
    }
}
#[test]
fn real_distance_paths_keep_native_ties_and_cleanup() -> Result {
    let input = input();
    let result = input.run(&AtomicBool::new(false))?;
    let mut paths = Search {
        input: &input,
        result: &result,
        metric: Metric::Distance,
    }
    .run(0)?;
    let last = paths::get(&paths, 3)?;
    assert_eq!(
        (
            last.parent,
            last.distance,
            last.capacity,
            last.free_capacity
        ),
        (Some(1), 200, 80, 80)
    );
    assert_eq!(paths::get(&paths, 0)?.children, 2);
    assert_eq!(paths::get(&paths, 1)?.children, 1);
    paths::cleanup(&mut paths, 0)?;
    assert!(paths.is_empty());
    assert_eq!(
        result
            .annotations
            .iter()
            .map(|n| n.undelivered_supply)
            .collect::<Vec<_>>(),
        [73, 0, 9, 0]
    );
    assert!(
        result
            .annotations
            .iter()
            .all(|n| n.paths.is_empty() && n.flows.is_empty())
    );
    Ok(())
}
#[test]
fn capacity_pass_uses_actual_flow_edges_and_descending_node_ties() -> Result {
    let input = input();
    let result = input.run(&AtomicBool::new(false))?;
    let paths = Search {
        input: &input,
        result: &result,
        metric: Metric::Capacity,
    }
    .run(0)?;
    assert_eq!(paths::get(&paths, 3)?.distance, u32::MAX);
    assert!(
        paths::get(&paths, 2)?.rank(Metric::Capacity)
            < paths::get(&paths, 1)?.rank(Metric::Capacity)
    );
    assert_eq!(paths::get(&paths, 0)?.origin, 0);
    assert_eq!(paths::get(&paths, 3)?.origin, u16::MAX);
    Ok(())
}
#[test]
fn source_capacity_ratio_retains_cpp_unsigned_division() {
    assert_eq!(paths::ratio(-1, 100), 42_949_672);
    assert_eq!(paths::ratio(i32::MIN, 0), -2_147_483_632);
    assert_eq!(paths::ratio(i32::MAX, u32::MAX), 0);
    assert_eq!(paths::ratio(80, 100), 12);
}
#[test]
fn zero_time_fallback_and_saturation_minimum_are_computed() -> Result {
    let mut input = input();
    input.settings.saturation = 0;
    input
        .nodes
        .first_mut()
        .ok_or("source")?
        .edges
        .first_mut()
        .ok_or("edge")?
        .travel_time = 0;
    let result = input.run(&AtomicBool::new(false))?;
    let paths = Search {
        input: &input,
        result: &result,
        metric: Metric::Distance,
    }
    .run(0)?;
    assert_eq!(
        (
            paths::get(&paths, 1)?.distance,
            paths::get(&paths, 1)?.capacity
        ),
        (222, 1)
    );
    input.settings.saturation = 80;
    input
        .nodes
        .first_mut()
        .ok_or("source")?
        .edges
        .first_mut()
        .ok_or("edge")?
        .capacity = u32::MAX;
    let result = input.run(&AtomicBool::new(false))?;
    let paths = Search {
        input: &input,
        result: &result,
        metric: Metric::Distance,
    }
    .run(0)?;
    assert_eq!(paths::get(&paths, 1)?.capacity, 42_949_672);
    Ok(())
}
