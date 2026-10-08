//! Full isolated subsystem stepping and rejection scenarios.
use ottd_sim::{Clock, Context, Fixture, Map, State, Tile, simulate};

fn fixture() -> Fixture {
    let tile = Tile {
        tile_type: 0,
        height: 0,
        m1: 0,
        m2: 0,
        m3: 0,
        m4: 0,
        m5: 224,
        m6: 0,
        m7: 0,
        m8: 0,
    };
    Fixture {
        schema_version: 1,
        context: Context {
            landscape: "temperate".into(),
            mode: "normal".into(),
            ambient_callbacks: false,
            paused: false,
            timekeeping_units: 0,
            minutes_per_calendar_year: 12,
        },
        state: State {
            map: Map {
                width: 64,
                height: 64,
                tiles: vec![tile; 4096],
            },
            clock: Clock {
                date: 0,
                date_fract: 0,
                calendar_sub_date_fract: 0,
                economy_date: 0,
                economy_date_fract: 0,
                days_since_last_month: 0,
                tick_counter: 0,
                calendar_year: 0,
                calendar_month: 0,
                economy_year: 0,
                economy_month: 0,
            },
            cur_tileloop_tile: 1,
            random_state: [17, 31],
        },
        events: vec![],
    }
}

#[test]
fn clocks_then_terrain_and_resume_are_exact() -> Result<(), Box<dyn std::error::Error>> {
    let input = fixture();
    let output = simulate(input.clone(), 256)?;
    assert_eq!(output.state.clock.tick_counter, 256);
    assert!(output.state.map.tiles.iter().all(|tile| tile.m5 == 1));
    assert_eq!(output.state.random_state, [17, 31]);
    assert_eq!(output.events.len(), 256);
    let resumed = simulate(simulate(input.clone(), 255)?, 1)?;
    assert_eq!(resumed.state, output.state);
    assert_eq!(resumed.events.len(), 1);
    assert_eq!(simulate(input.clone(), 0)?, input);
    Ok(())
}

#[test]
fn paused_preserves_every_state_field() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = fixture();
    input.context.paused = true;
    let output = simulate(input.clone(), 300)?;
    assert_eq!(output.state, input.state);
    assert!(
        output
            .events
            .iter()
            .all(|record| !record.calendar_progressed && record.events.is_empty())
    );
    Ok(())
}

#[test]
fn invalid_contract_and_budget_fail() {
    let mut inputs = Vec::new();
    let mut input = fixture();
    input.schema_version = 2;
    inputs.push(input);
    let mut input = fixture();
    input.context.landscape = "arctic".into();
    inputs.push(input);
    let mut input = fixture();
    input.context.mode = "editor".into();
    inputs.push(input);
    let mut input = fixture();
    input.context.ambient_callbacks = true;
    inputs.push(input);
    let mut input = fixture();
    input.context.timekeeping_units = 2;
    inputs.push(input);
    let mut input = fixture();
    input.state.clock.date_fract = 74;
    inputs.push(input);
    let mut input = fixture();
    input.state.clock.calendar_year = 3;
    inputs.push(input);
    for input in inputs {
        assert!(simulate(input, 0).is_err());
    }
    assert!(simulate(fixture(), 1_000_001).is_err());
}
