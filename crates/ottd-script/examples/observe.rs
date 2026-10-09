//! Test-only native differential observer for scalar programs.
use ottd_script::{Execution, Value, Vm, compile};
use std::{env, error::Error, fs};
fn value(value: Value) {
    match value {
        Value::Null => println!("null"),
        Value::Integer(n) => println!("integer {n}"),
        Value::Float(bits) => println!("float {bits}"),
        Value::Bool(b) => println!("bool {}", u8::from(b)),
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let filename = args.next().ok_or("source path required")?;
    let source = fs::read_to_string(filename)?;
    let program = match compile(&source) {
        Ok(program) => program,
        Err(error) => {
            println!("compile_error");
            eprintln!("{error}");
            return Ok(());
        }
    };
    println!("stack {}", program.stack_size);
    for literal in &program.literals {
        print!("literal ");
        value(*literal);
    }
    for i in &program.instructions {
        println!(
            "op {} {} {} {} {}",
            i.opcode, i.arg0, i.arg1, i.arg2, i.arg3
        );
    }
    let credits: Vec<u32> = args.map(|s| s.parse()).collect::<Result<_, _>>()?;
    let default = [10_000];
    let credits = if credits.is_empty() {
        default.as_slice()
    } else {
        credits.as_slice()
    };
    let mut vm = Vm::new(&program)?;
    for credit in credits {
        match vm.resume(*credit) {
            Ok(Execution::Suspended) => println!(
                "suspend {} {}",
                vm.remaining_ops(),
                vm.instruction_pointer()
            ),
            Ok(Execution::Returned(result)) => {
                print!("return {} ", vm.remaining_ops());
                value(result);
                break;
            }
            Err(error) => {
                println!("runtime_error {}", vm.remaining_ops());
                eprintln!("{error}");
                break;
            }
        }
    }
    Ok(())
}
