//! Public production regression: foreach must compile and execute real scalar arrays.
use ottd_script::{Execution, Runner, Value, compile};

#[test]
fn allocation_iteration_and_index_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile("local a=[4,7]; foreach(i,v in a) { a[i]=v*2; }\nreturn a;")?;
    let mut vm = ottd_script::Vm::new(&program)?;
    let Execution::Returned(Value::Array(array)) = vm.resume(10000)? else {
        return Err("expected returned array".into());
    };
    assert_eq!(array.get(0), Some(Value::Integer(8)));
    assert_eq!(array.get(1), Some(Value::Integer(14)));
    Ok(())
}

#[test]
fn shared_root_mutation_at_foreach_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let realm = ottd_script::Realm::new();
    let root = realm.empty_root();
    let array = realm.array(vec![
        Value::Integer(1),
        Value::Integer(2),
        Value::Integer(3),
    ])?;
    root.new_slot(b"pending", Value::Array(array.clone()))?;
    let mut runner = Runner::new(root);
    let program = runner.compile("local n=0; foreach(v in pending) { n+=v; }\nreturn n;")?;
    let mut vm = runner.start(&program)?;
    assert_eq!(vm.resume(0)?, Execution::Suspended);
    for _ in 0..5 {
        assert_eq!(vm.resume(2)?, Execution::Suspended);
    }
    assert_eq!(vm.instruction_pointer(), 5);
    array.set(1, Value::Integer(9))?;
    assert_eq!(vm.resume(10000)?, Execution::Returned(Value::Integer(13)));
    Ok(())
}
