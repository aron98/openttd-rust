#![expect(
    clippy::redundant_pub_crate,
    reason = "integration-test helper types are shared only with their parent test crate"
)]
use super::Result;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub(super) struct Event {
    pub sequence: u64,
    pub phase: String,
    pub edge: String,
    pub tick: u64,
    pub random: [u32; 2],
    pub trees_tick_ctr: u64,
    pub cursor: u64,
    pub tile: Option<usize>,
    pub raw: Option<[u64; 10]>,
}
#[derive(Default, serde::Serialize)]
pub(super) struct Statistics {
    pub calls: u64,
    pub events: u64,
    pub callbacks: u64,
    pub mutations: u64,
    pub plants: u64,
    pub post_vehicle_plants: u64,
    pub clears: u64,
    pub grown_branches: [u64; 8],
    pub saturated_branch_one: u64,
    pub count_additions: u64,
    pub count_removals: u64,
}
pub(super) struct Trace {
    pub stats: Statistics,
    pub expected: BTreeMap<usize, [u64; 10]>,
    stack: Vec<Event>,
    tick: u64,
    phase: u8,
}
fn add(value: &mut u64, amount: u64) -> Result {
    *value = value.checked_add(amount).ok_or("trace counter overflow")?;
    Ok(())
}
impl Trace {
    pub(super) fn new(tick: u64) -> Self {
        Self {
            stats: Statistics::default(),
            expected: BTreeMap::new(),
            stack: Vec::new(),
            tick,
            phase: 0,
        }
    }
    fn enter(&mut self, event: Event) -> Result {
        match event.phase.as_str() {
            "tile_tree" => assert_eq!(self.phase, 0),
            "vehicle_ticks" => {
                assert_eq!(self.phase, 0);
                self.phase = 1;
            }
            "on_tick_trees" => {
                assert_eq!(self.phase, 2);
                self.phase = 3;
            }
            "plant_tree" | "clear_tree" => assert!(!self.stack.is_empty()),
            _ => return Err("unrecognized native phase".into()),
        }
        if event.phase == "plant_tree" && self.phase == 3 {
            add(&mut self.stats.post_vehicle_plants, 1)?;
        }
        self.stack.push(event);
        Ok(())
    }
    pub(super) fn observe(&mut self, event: Event, spread: bool) -> Result<Option<Event>> {
        add(&mut self.stats.events, 1)?;
        assert_eq!(event.sequence, self.stats.events);
        assert_eq!(
            event.tick,
            self.tick.checked_add(1).ok_or("trace tick overflow")?
        );
        if event.edge == "enter" {
            self.enter(event)?;
            return Ok(None);
        }
        assert_eq!(event.edge, "leave");
        let before = self.stack.pop().ok_or("unpaired native scope")?;
        assert_eq!((&before.phase, before.tile), (&event.phase, event.tile));
        self.growth(&before, &event, spread)?;
        match event.phase.as_str() {
            "tile_tree" => add(&mut self.stats.callbacks, 1)?,
            "plant_tree" => add(&mut self.stats.plants, 1)?,
            "clear_tree" => add(&mut self.stats.clears, 1)?,
            "vehicle_ticks" => {
                assert_eq!(self.phase, 1);
                self.phase = 2;
            }
            "on_tick_trees" => {
                assert_eq!(self.phase, 3);
                self.phase = 0;
            }
            _ => return Err("unrecognized native phase".into()),
        }
        if let Some(index) = event.tile {
            if before.raw != event.raw {
                add(&mut self.stats.mutations, 1)?;
            }
            self.expected
                .insert(index, event.raw.ok_or("missing raw tile")?);
        }
        if event.phase != "on_tick_trees" {
            return Ok(None);
        }
        assert!(self.stack.is_empty());
        add(&mut self.tick, 1)?;
        add(&mut self.stats.calls, 1)?;
        Ok(Some(event))
    }
    fn growth(&mut self, before: &Event, after: &Event, spread: bool) -> Result {
        if before.phase != "tile_tree" {
            return Ok(());
        }
        let start = before.raw.ok_or("tree entry")?;
        let end = after.raw.ok_or("tree exit")?;
        if end[0] >> 4 == 4 {
            add(
                &mut self.stats.count_additions,
                u64::from(end[6] >> 6 > start[6] >> 6),
            )?;
            add(
                &mut self.stats.count_removals,
                u64::from(end[6] >> 6 < start[6] >> 6),
            )?;
        }
        if before.random == after.random {
            return Ok(());
        }
        assert_eq!(start[6] & 7, 3);
        let mut rng = ottd_core::Randomizer::from_state(before.random);
        let branch = rng.next_u32() & 7;
        let saturated = branch == 1 && start[6] >> 6 == 3;
        if spread && (branch == 2 || saturated) {
            rng.next_u32();
        }
        assert_eq!(
            rng.state(),
            after.random,
            "exact native callback draw order"
        );
        let count = self
            .stats
            .grown_branches
            .get_mut(usize::try_from(branch)?)
            .ok_or("branch index")?;
        add(count, 1)?;
        add(
            &mut self.stats.saturated_branch_one,
            u64::from(spread && saturated),
        )?;
        Ok(())
    }
    pub(super) fn complete(&self) {
        assert!(self.stats.calls > 0 && self.stack.is_empty() && self.expected.is_empty());
        assert_eq!(self.phase, 0);
    }
}
