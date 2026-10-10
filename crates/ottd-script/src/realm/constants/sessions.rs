//! Stateful native projections compare compiler effects and actual execution.
//! Native temp/refcount/drop-child records stay separate: Vm borrows Program.
use super::{
    Realm,
    corpus::CASES,
    tests::{dump, lookup, value},
};
use crate::{Execution, Program, Vm};
use std::fmt::Write;

fn run(program: &Program, output: &mut String) -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Vm::new(program)?;
    for credit in [0, 2, 2, 2, 2, 2, 100] {
        match vm.resume(credit)? {
            Execution::Suspended => writeln!(
                output,
                "suspend {} {}",
                vm.remaining_ops(),
                vm.instruction_pointer()
            )?,
            Execution::Returned(result) => {
                writeln!(output, "return {} {}", vm.remaining_ops(), value(&result)?)?;
                return Ok(());
            }
        }
    }
    Err("session did not terminate within captured schedule".into())
}
fn project(native: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut output = String::new();
    for line in native.lines() {
        let kind = line
            .split_whitespace()
            .next()
            .ok_or("empty native record")?;
        match kind {
            "compile_error" => output.push_str("compile_error\n"),
            "compiled" | "stack" | "literal" | "op" | "lookup" | "suspend" | "return" => {
                writeln!(output, "{line}")?;
            }
            "step" | "refs" | "dropped" => {}
            _ => return Err("unexpected native session record".into()),
        }
    }
    Ok(output)
}
fn observe(session: &str) -> Result<String, Box<dyn std::error::Error>> {
    let root = Realm::new();
    let realms = [root.clone(), root, Realm::new()];
    let mut slots: [Option<Program>; 16] = std::array::from_fn(|_| None);
    let mut output = String::new();
    for line in session.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        match fields.as_slice() {
            ["compile", realm, slot, "buffer", source] => {
                let realm = realms.get(realm.parse::<usize>()?).ok_or("missing realm")?;
                let slot = slots
                    .get_mut(slot.parse::<usize>()?)
                    .ok_or("missing slot")?;
                if slot.is_some() {
                    return Err("occupied slot".into());
                }
                let name = source
                    .strip_prefix("fixtures/")
                    .and_then(|path| path.strip_suffix(".nut"))
                    .ok_or("invalid source path")?;
                let (_, source, _, _) = CASES
                    .iter()
                    .find(|case| case.0 == name)
                    .ok_or("unlisted source")?;
                match realm.compile_bytes(source) {
                    Ok(program) => {
                        output.push_str("compiled\n");
                        output.push_str(&dump(&program)?);
                        *slot = Some(program);
                    }
                    Err(_) => output.push_str("compile_error\n"),
                }
            }
            ["lookup", realm, name, member] => {
                let realm = realms.get(realm.parse::<usize>()?).ok_or("missing realm")?;
                output.push_str(&lookup(realm, name, member)?);
            }
            ["drop", _, slot] => {
                let slot = slots
                    .get_mut(slot.parse::<usize>()?)
                    .ok_or("missing slot")?;
                if slot.take().is_none() {
                    return Err("missing closure".into());
                }
            }
            ["run", _, slot] => {
                let program = slots
                    .get(slot.parse::<usize>()?)
                    .and_then(Option::as_ref)
                    .ok_or("missing closure")?;
                run(program, &mut output)?;
            }
            ["refs", _, _] => {}
            _ => return Err("unexpected session command".into()),
        }
    }
    Ok(output)
}
#[test]
fn shared_session_effects_match_native_compile_lookup_and_execution()
-> Result<(), Box<dyn std::error::Error>> {
    for (session, native) in [
        (
            include_str!("../../../tests/constants/native/shared-lifetime.session"),
            include_str!("../../../tests/constants/native/shared-lifetime-session.txt"),
        ),
        (
            include_str!("../../../tests/constants/native/replacement-failure.session"),
            include_str!("../../../tests/constants/native/replacement-failure-session.txt"),
        ),
        (
            include_str!("../../../tests/constants/native/updates.session"),
            include_str!("../../../tests/constants/native/updates-session.txt"),
        ),
    ] {
        assert_eq!(observe(session)?, project(native)?);
    }
    Ok(())
}
