//! Exact source bodies with original-success RED receipts in the approved delegate design.
use ottd_script::{Execution, Value, Vm, compile};

fn execute(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(source)?;
    for credits in [
        vec![10000],
        vec![0, 1, 2, 4, 0, 8, 16, 32, 10000],
        [vec![2; 40], vec![10000]].concat(),
    ] {
        let mut vm = Vm::new(&program)?;
        let mut result = Ok(Execution::Suspended);
        for credit in credits {
            result = vm.resume(credit);
            if result != Ok(Execution::Suspended) {
                break;
            }
        }
        assert_eq!(result, Ok(Execution::Returned(Value::Integer(1))));
    }
    Ok(())
}

#[test]
fn attach_own_and_chain() {
    execute(include_str!("delegation/attach-own-and-chain.nut"))
        .expect("defined native delegation");
}

#[test]
fn own_shadows_parent() {
    execute(include_str!("delegation/own-shadows-parent.nut")).expect("defined native delegation");
}

#[test]
fn chain_three() {
    execute(include_str!("delegation/chain-three.nut")).expect("defined native delegation");
}

#[test]
fn detach_alias() {
    execute(include_str!("delegation/detach-alias.nut")).expect("defined native delegation");
}

#[test]
fn set_existing_parent() {
    execute(include_str!("delegation/set-existing-parent.nut")).expect("defined native delegation");
}

#[test]
fn set_nearest_parent() {
    execute(include_str!("delegation/set-nearest-parent.nut")).expect("defined native delegation");
}

#[test]
fn newslot_own_shadow() {
    execute(include_str!("delegation/newslot-own-shadow.nut")).expect("defined native delegation");
}

#[test]
fn in_traverses_delegate() {
    execute(include_str!("delegation/in-traverses-delegate.nut"))
        .expect("defined native delegation");
}

#[test]
fn foreach_own_only() {
    execute(include_str!("delegation/foreach-own-only.nut")).expect("defined native delegation");
}

#[test]
fn delete_own_reveals_parent() {
    execute(include_str!("delegation/delete-own-reveals-parent.nut"))
        .expect("defined native delegation");
}

#[test]
fn delete_inherited_refuses() {
    execute(include_str!("delegation/delete-inherited-refuses.nut"))
        .expect("defined native delegation");
}

#[test]
fn clone_retains_delegate() {
    execute(include_str!("delegation/clone-retains-delegate.nut"))
        .expect("defined native delegation");
}

#[test]
fn cycle_direct_atomic() {
    execute(include_str!("delegation/cycle-direct-atomic.nut")).expect("defined native delegation");
}

#[test]
fn cycle_indirect_atomic() {
    execute(include_str!("delegation/cycle-indirect-atomic.nut"))
        .expect("defined native delegation");
}

#[test]
fn receiver_error_before_delegate_type() {
    execute(include_str!(
        "delegation/receiver-error-before-delegate-type.nut"
    ))
    .expect("defined native delegation");
}

#[test]
fn delegate_type_atomic() {
    execute(include_str!("delegation/delegate-type-atomic.nut"))
        .expect("defined native delegation");
}

#[test]
fn parent_bare_root() {
    execute(include_str!("delegation/parent-bare-root.nut")).expect("defined native delegation");
}

#[test]
fn parent_nontable_error() {
    execute(include_str!("delegation/parent-nontable-error.nut"))
        .expect("defined native delegation");
}

#[test]
fn root_delegate_read_write() {
    execute(include_str!("delegation/root-delegate-read-write.nut"))
        .expect("defined native delegation");
}

#[test]
fn closure_retained_delegate() {
    execute(include_str!("delegation/closure-retained-delegate.nut"))
        .expect("defined native delegation");
}

#[test]
fn ordinary_slot_cycle() {
    execute(include_str!("delegation/ordinary-slot-cycle.nut")).expect("defined native delegation");
}

#[test]
fn delegated_method_this() {
    execute(include_str!("delegation/delegated-method-this.nut"))
        .expect("defined native delegation");
}

#[test]
fn unicode_nul_key() {
    execute(include_str!("delegation/unicode-nul-key.nut")).expect("defined native delegation");
}

#[test]
fn delegate_evaluation_order() {
    execute(include_str!("delegation/delegate-evaluation-order.nut"))
        .expect("defined native delegation");
}

#[test]
fn raw_in_skips_get_hook() {
    execute(include_str!("delegation/raw-in-skips-get-hook.nut"))
        .expect("defined native delegation");
}

#[test]
fn own_get_dead_hook() {
    execute(include_str!("delegation/own-get-dead-hook.nut"))
        .expect("defined native delegation bypass");
}

#[test]
fn own_newslot_dead_hook() {
    execute(include_str!("delegation/own-newslot-dead-hook.nut"))
        .expect("defined native delegation bypass");
}

#[test]
fn foreach_dead_nexti() {
    execute(include_str!("delegation/foreach-dead-nexti.nut"))
        .expect("defined native delegation bypass");
}

#[test]
fn same_identity_dead_cmp() {
    execute(include_str!("delegation/same-identity-dead-cmp.nut"))
        .expect("defined native delegation bypass");
}

#[test]
fn builtin_type_dead_typeof() {
    execute(include_str!("delegation/builtin-type-dead-typeof.nut"))
        .expect("defined native delegation bypass");
}

#[test]
fn immediate_meta_only() {
    execute(include_str!("delegation/immediate-meta-only.nut"))
        .expect("defined native delegation bypass");
}
