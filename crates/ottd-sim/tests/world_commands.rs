//! Saved-world command behavior, including the native outer pipeline.
#![cfg(test)]
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World},
};
use ottd_sim::{Command, CommandMode, CommandRequest, execute_command};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn populated() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn request(command: Command) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command,
    }
}
fn field<'a>(world: &'a World, chunk: &[u8], id: u32, name: &str) -> Result<&'a WireValue> {
    let table = world.tables().get(chunk).ok_or("table")?;
    table
        .schema()
        .fields()
        .iter()
        .zip(table.records().get(&id).ok_or("row")?.values())
        .find_map(|(f, v)| (f.name() == name).then_some(v))
        .ok_or_else(|| "field".into())
}
fn set(world: &mut World, chunk: [u8; 4], name: &str, value: WireValue) -> Result {
    world.edit_field(chunk, 0, &[PathElement::Field(name.into())], value)?;
    Ok(())
}
#[test]
fn loan_interval_changes_balance_and_loan_without_expense() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(&mut world, *b"PLYR", "current_loan", WireValue::Signed(0))?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(1_000))?;
    let expense = field(&world, b"PLYR", 0, "yearly_expenses")?.clone();
    let receipt = execute_command(
        &mut world,
        &request(Command::IncreaseLoan {
            method: 0,
            amount: 0,
        }),
    )?;
    assert!(receipt.posted);
    assert_eq!(
        field(&world, b"PLYR", 0, "money")?,
        &WireValue::Signed(11_000)
    );
    assert_eq!(
        field(&world, b"PLYR", 0, "current_loan")?,
        &WireValue::Signed(10_000)
    );
    assert_eq!(field(&world, b"PLYR", 0, "yearly_expenses")?, &expense);
    Ok(())
}
#[test]
fn estimate_loan_preserves_complete_saved_world() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"PLYR", "current_loan", WireValue::Signed(0))?;
    let before = world.saved_json()?;
    let mut command = request(Command::IncreaseLoan {
        method: 0,
        amount: 0,
    });
    command.mode = CommandMode::Estimate;
    let receipt = execute_command(&mut world, &command)?;
    assert!(receipt.posted);
    assert!(receipt.exec.is_none());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn repayment_reports_native_required_currency_without_mutation() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(
        &mut world,
        *b"PLYR",
        "current_loan",
        WireValue::Signed(10_000),
    )?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(1))?;
    let before = world.saved_json()?;
    let receipt = execute_command(
        &mut world,
        &request(Command::DecreaseLoan {
            method: 0,
            amount: 0,
        }),
    )?;
    assert!(!receipt.posted);
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_CURRENCY_REQUIRED")
    );
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn duplicate_company_name_including_self_fails() -> Result {
    let mut world = populated()?;
    set(
        &mut world,
        *b"PLYR",
        "name",
        WireValue::Bytes(b"Existing".to_vec()),
    )?;
    let before = world.saved_json()?;
    let receipt = execute_command(
        &mut world,
        &request(Command::RenameCompany {
            text: "Existing".into(),
        }),
    )?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_NAME_MUST_BE_UNIQUE")
    );
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn company_name_limit_counts_unicode_characters() -> Result {
    let mut world = populated()?;
    let name = "é".repeat(31);
    let receipt = execute_command(
        &mut world,
        &request(Command::RenameCompany { text: name.clone() }),
    )?;
    assert!(receipt.posted);
    assert_eq!(
        field(&world, b"PLYR", 0, "name")?,
        &WireValue::Bytes(name.into_bytes())
    );
    Ok(())
}
#[test]
fn normal_unpause_clears_command_during_pause_marker() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(129))?;
    let receipt = execute_command(
        &mut world,
        &request(Command::Pause {
            mode: 0,
            paused: false,
        }),
    )?;
    assert!(receipt.posted);
    assert_eq!(
        field(&world, b"DATE", 0, "pause_mode")?,
        &WireValue::Unsigned(0)
    );
    Ok(())
}
#[test]
fn pause_no_estimate_trait_executes_even_in_estimate_mode() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    let mut command = request(Command::Pause {
        mode: 0,
        paused: true,
    });
    command.mode = CommandMode::Estimate;
    let receipt = execute_command(&mut world, &command)?;
    assert!(receipt.exec.is_some());
    assert_eq!(
        field(&world, b"DATE", 0, "pause_mode")?,
        &WireValue::Unsigned(1)
    );
    Ok(())
}
#[test]
fn invalid_tile_is_early_gate_without_native_cost() -> Result {
    let mut world = populated()?;
    let before = world.saved_json()?;
    let receipt = execute_command(
        &mut world,
        &request(Command::LandscapeClear { tile: u32::MAX }),
    )?;
    assert!(!receipt.posted);
    assert!(receipt.gate.is_some());
    assert!(receipt.test.is_none() && receipt.exec.is_none() && receipt.result.is_none());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
fn flat_clear(world: &World) -> Result<u32> {
    let width =
        std::num::NonZeroUsize::new(usize::try_from(world.map().width())?).ok_or("width")?;
    let last_column = width.get().checked_sub(1).ok_or("width")?;
    world
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find_map(|(i, t)| {
            if t.tile_type() >> 4 != 0 || i % width == last_column {
                return None;
            }
            [
                i.checked_add(1)?,
                i.checked_add(width.get())?,
                i.checked_add(width.get())?.checked_add(1)?,
            ]
            .iter()
            .all(|n| {
                world
                    .map()
                    .tiles()
                    .get(*n)
                    .is_some_and(|other| other.height() == t.height())
            })
            .then_some(i)
        })
        .ok_or_else(|| "flat clear tile".into())
        .and_then(|i| Ok(u32::try_from(i)?))
}
#[test]
fn builds_road_with_native_cost_and_saved_bookkeeping() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(1_000_000))?;
    set(
        &mut world,
        *b"PATS",
        "difficulty.construction_cost",
        WireValue::Unsigned(1),
    )?;
    set(
        &mut world,
        *b"ECMY",
        "inflation_prices",
        WireValue::Unsigned(65_536),
    )?;
    let tile = flat_clear(&world)?;
    let source = world
        .map()
        .tiles()
        .get(usize::try_from(tile)?)
        .ok_or("tile")?;
    let source_height = source.height();
    world.edit_tile(
        tile,
        &ottd_save::TileState::from(ottd_save::TileRawParts {
            tile_type: 0,
            height: source_height,
            m1: 16,
            m2: 0,
            m3: 0,
            m4: 0,
            m5: 0,
            m6: 0,
            m7: 0,
            m8: 0,
        }),
    )?;
    let before_limit = field(&world, b"PLYR", 0, "clear_limit")?.clone();
    let receipt = execute_command(
        &mut world,
        &request(Command::BuildRoad {
            tile,
            pieces: 5,
            road_type: 0,
            toggle_disallowed: 0,
            town_id: u16::MAX,
        }),
    )?;
    assert_eq!(receipt.result.ok_or("result")?.cost, 190);
    assert_eq!(
        field(&world, b"PLYR", 0, "money")?,
        &WireValue::Signed(999_810)
    );
    assert_eq!(
        field(&world, b"PLYR", 0, "last_build_coordinate")?,
        &WireValue::Unsigned(u64::from(tile))
    );
    assert_eq!(field(&world, b"PLYR", 0, "clear_limit")?, &before_limit);
    let built = world
        .map()
        .tiles()
        .get(usize::try_from(tile)?)
        .ok_or("tile")?;
    assert_eq!(
        (built.tile_type() >> 4, built.m5() & 15, built.m1() & 31),
        (2, 5, 0)
    );
    Ok(())
}
#[test]
fn clear_rough_charges_price_and_consumes_clear_limit() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(1_000))?;
    set(
        &mut world,
        *b"PLYR",
        "clear_limit",
        WireValue::Unsigned(131_072),
    )?;
    set(
        &mut world,
        *b"PATS",
        "difficulty.construction_cost",
        WireValue::Unsigned(1),
    )?;
    set(
        &mut world,
        *b"ECMY",
        "inflation_prices",
        WireValue::Unsigned(65_536),
    )?;
    let tile = flat_clear(&world)?;
    let source = world
        .map()
        .tiles()
        .get(usize::try_from(tile)?)
        .ok_or("tile")?;
    let mut parts = ottd_save::TileRawParts::from(source);
    parts.m5 = 7;
    world.edit_tile(tile, &parts.into())?;
    let receipt = execute_command(&mut world, &request(Command::LandscapeClear { tile }))?;
    assert_eq!(receipt.result.ok_or("result")?.cost, 40);
    assert_eq!(field(&world, b"PLYR", 0, "money")?, &WireValue::Signed(960));
    assert_eq!(
        field(&world, b"PLYR", 0, "clear_limit")?,
        &WireValue::Unsigned(65_536)
    );
    assert_eq!(
        world
            .map()
            .tiles()
            .get(usize::try_from(tile)?)
            .ok_or("tile")?
            .m5(),
        0
    );
    Ok(())
}
#[test]
fn extends_existing_road_despite_unrelated_vehicle_pool() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    let tile = flat_clear(&world)?;
    let height = world
        .map()
        .tiles()
        .get(usize::try_from(tile)?)
        .ok_or("tile")?
        .height();
    world.edit_tile(
        tile,
        &ottd_save::TileRawParts {
            tile_type: 32,
            height,
            m1: 0,
            m2: 0,
            m3: 0,
            m4: 0,
            m5: 1,
            m6: 0,
            m7: 0,
            m8: 4032,
        }
        .into(),
    )?;
    let receipt = execute_command(
        &mut world,
        &request(Command::BuildRoad {
            tile,
            pieces: 4,
            road_type: 0,
            toggle_disallowed: 0,
            town_id: u16::MAX,
        }),
    )?;
    assert!(receipt.posted);
    assert_eq!(
        world
            .map()
            .tiles()
            .get(usize::try_from(tile)?)
            .ok_or("tile")?
            .m5()
            & 15,
        5
    );
    Ok(())
}

#[test]
fn naming_rejects_active_script_events_transactionally() -> Result {
    let mut world = populated()?;
    set(
        &mut world,
        *b"GSDT",
        "name",
        WireValue::Bytes(b"Script".to_vec()),
    )?;
    let before = world.saved_json()?;
    let receipt = execute_command(
        &mut world,
        &request(Command::RenameCompany {
            text: "Changed".into(),
        }),
    );
    assert!(matches!(
        receipt,
        Err(ottd_sim::CommandError::Unsupported(_))
    ));
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn explicit_loan_rejects_invalid_amounts_without_changing_world() -> Result {
    for amount in [-10_000, 0, 9_999, 10_001, i64::MAX] {
        let mut world = populated()?;
        set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
        set(&mut world, *b"PLYR", "current_loan", WireValue::Signed(0))?;
        let before = world.saved_json()?;
        let receipt = execute_command(
            &mut world,
            &request(Command::IncreaseLoan { method: 2, amount }),
        )?;
        assert_eq!(
            receipt.result.ok_or("result")?.error.as_deref(),
            Some("CMD_ERROR")
        );
        assert_eq!(world.saved_json()?, before);
    }
    Ok(())
}

#[test]
fn president_rename_keeps_success_when_nested_company_name_is_too_long() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"PLYR", "name", WireValue::Bytes(Vec::new()))?;
    set(&mut world, *b"PLYR", "name_1", WireValue::Unsigned(0x6001))?;
    let name = "A".repeat(31);
    let receipt = execute_command(
        &mut world,
        &request(Command::RenamePresident { text: name.clone() }),
    )?;
    assert!(receipt.posted);
    assert_eq!(
        field(&world, b"PLYR", 0, "president_name")?,
        &WireValue::Bytes(name.into_bytes())
    );
    assert_eq!(
        field(&world, b"PLYR", 0, "name")?,
        &WireValue::Bytes(Vec::new())
    );
    Ok(())
}

#[test]
fn successful_name_command_marks_paused_world() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(1))?;
    let receipt = execute_command(
        &mut world,
        &request(Command::RenameCompany {
            text: "Pause name".into(),
        }),
    )?;
    assert!(receipt.posted);
    assert_eq!(
        field(&world, b"DATE", 0, "pause_mode")?,
        &WireValue::Unsigned(129)
    );
    Ok(())
}

#[test]
fn clearing_estimate_bypasses_pause_and_affordability_without_mutation() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(1))?;
    set(
        &mut world,
        *b"PATS",
        "construction.command_pause_level",
        WireValue::Unsigned(0),
    )?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(0))?;
    let tile = flat_clear(&world)?;
    let before = world.saved_json()?;
    let mut command = request(Command::LandscapeClear { tile });
    command.mode = CommandMode::Estimate;
    let receipt = execute_command(&mut world, &command)?;
    assert!(receipt.posted && receipt.exec.is_none());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn clearing_post_obeys_pause_gate_without_body_cost() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(1))?;
    set(
        &mut world,
        *b"PATS",
        "construction.command_pause_level",
        WireValue::Unsigned(0),
    )?;
    let tile = flat_clear(&world)?;
    let before = world.saved_json()?;
    let receipt = execute_command(&mut world, &request(Command::LandscapeClear { tile }))?;
    assert_eq!(receipt.gate, Some(ottd_sim::CommandGate::Pause));
    assert!(receipt.result.is_none() && receipt.test.is_none() && receipt.exec.is_none());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn foreign_company_cannot_toggle_town_owned_oneway_road() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    let tile = flat_clear(&world)?;
    let height = world
        .map()
        .tiles()
        .get(usize::try_from(tile)?)
        .ok_or("tile")?
        .height();
    world.edit_tile(
        tile,
        &ottd_save::TileRawParts {
            tile_type: 32,
            height,
            m1: 15,
            m2: 0,
            m3: 0,
            m4: 0,
            m5: 5,
            m6: 0,
            m7: 0,
            m8: 4032,
        }
        .into(),
    )?;
    let before = world.saved_json()?;
    let receipt = execute_command(
        &mut world,
        &request(Command::BuildRoad {
            tile,
            pieces: 5,
            road_type: 0,
            toggle_disallowed: 1,
            town_id: u16::MAX,
        }),
    )?;
    let result = receipt.result.ok_or("result")?;
    assert_eq!(result.error.as_deref(), Some("STR_ERROR_OWNED_BY"));
    assert_eq!(result.error_params, vec![0x8827, 0]);
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn deity_construction_is_explicitly_unsupported_not_native_failure() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    let tile = flat_clear(&world)?;
    let before = world.saved_json()?;
    let mut command = request(Command::LandscapeClear { tile });
    command.company = 18;
    let result = execute_command(&mut world, &command);
    assert!(matches!(
        result,
        Err(ottd_sim::CommandError::Unsupported(_))
    ));
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn explicit_loan_saturates_balance_at_native_money_maximum() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(1_000_000))?;
    set(
        &mut world,
        *b"PLYR",
        "current_loan",
        WireValue::Signed(9_223_372_036_854_774_807),
    )?;
    set(
        &mut world,
        *b"PLYR",
        "max_loan",
        WireValue::Signed(i64::MAX),
    )?;
    let receipt = execute_command(
        &mut world,
        &request(Command::IncreaseLoan {
            method: 2,
            amount: 10_000,
        }),
    )?;
    let success = ottd_sim::CommandCost {
        success: true,
        cost: 0,
        expenses: 12,
        error: None,
        error_params: Vec::new(),
    };
    assert_eq!(
        receipt,
        ottd_sim::CommandReceipt {
            returns: None,
            posted: true,
            gate: None,
            test: Some(success.clone()),
            exec: Some(success.clone()),
            result: Some(success)
        }
    );
    assert_eq!(
        field(&world, b"PLYR", 0, "money")?,
        &WireValue::Signed(1_010_000)
    );
    assert_eq!(
        field(&world, b"PLYR", 0, "current_loan")?,
        &WireValue::Signed(i64::MAX)
    );
    Ok(())
}

#[test]
fn loan_rejects_cash_overflow_even_when_loan_balance_can_increase() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(
        &mut world,
        *b"PLYR",
        "money",
        WireValue::Signed(9_223_372_036_854_770_807),
    )?;
    set(&mut world, *b"PLYR", "current_loan", WireValue::Signed(0))?;
    set(
        &mut world,
        *b"PLYR",
        "max_loan",
        WireValue::Signed(i64::MAX),
    )?;
    let before = world.saved_json()?;
    let receipt = execute_command(
        &mut world,
        &request(Command::IncreaseLoan {
            method: 2,
            amount: 10_000,
        }),
    )?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("CMD_ERROR")
    );
    assert!(receipt.exec.is_none());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn infinite_money_repayment_saturates_cash_at_native_minimum() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(
        &mut world,
        *b"PATS",
        "difficulty.infinite_money",
        WireValue::Signed(1),
    )?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(i64::MIN))?;
    set(
        &mut world,
        *b"PLYR",
        "current_loan",
        WireValue::Signed(10_000),
    )?;
    let receipt = execute_command(
        &mut world,
        &request(Command::DecreaseLoan {
            method: 2,
            amount: 10_000,
        }),
    )?;
    assert!(receipt.posted);
    assert_eq!(
        field(&world, b"PLYR", 0, "money")?,
        &WireValue::Signed(i64::MIN)
    );
    assert_eq!(
        field(&world, b"PLYR", 0, "current_loan")?,
        &WireValue::Signed(0)
    );
    Ok(())
}

#[test]
fn construction_saturates_native_money_debit_and_expense_total() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(
        &mut world,
        *b"PATS",
        "difficulty.infinite_money",
        WireValue::Signed(1),
    )?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(i64::MIN))?;
    world.edit_field(
        *b"PLYR",
        0,
        &[
            PathElement::Field("yearly_expenses".into()),
            PathElement::Index(0),
        ],
        WireValue::Signed(9_223_372_036_854_775_707),
    )?;
    let tile = flat_clear(&world)?;
    let receipt = execute_command(
        &mut world,
        &request(Command::BuildRoad {
            tile,
            pieces: 5,
            road_type: 0,
            toggle_disallowed: 0,
            town_id: u16::MAX,
        }),
    )?;
    assert!(receipt.posted);
    assert_eq!(
        field(&world, b"PLYR", 0, "money")?,
        &WireValue::Signed(i64::MIN)
    );
    let WireValue::Array(expenses) = field(&world, b"PLYR", 0, "yearly_expenses")? else {
        return Err("expenses".into());
    };
    assert_eq!(expenses.first(), Some(&WireValue::Signed(i64::MAX)));
    Ok(())
}

#[test]
fn construction_affordability_failure_keeps_boundary_balances_unchanged() -> Result {
    let mut world = populated()?;
    set(&mut world, *b"DATE", "pause_mode", WireValue::Unsigned(0))?;
    set(
        &mut world,
        *b"PATS",
        "difficulty.infinite_money",
        WireValue::Signed(0),
    )?;
    set(&mut world, *b"PLYR", "money", WireValue::Signed(0))?;
    world.edit_field(
        *b"PLYR",
        0,
        &[
            PathElement::Field("yearly_expenses".into()),
            PathElement::Index(0),
        ],
        WireValue::Signed(9_223_372_036_854_775_707),
    )?;
    let tile = flat_clear(&world)?;
    let before = world.saved_json()?;
    let receipt = execute_command(
        &mut world,
        &request(Command::BuildRoad {
            tile,
            pieces: 5,
            road_type: 0,
            toggle_disallowed: 0,
            town_id: u16::MAX,
        }),
    )?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY")
    );
    assert!(receipt.exec.is_none());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
