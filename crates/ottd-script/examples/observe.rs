//! Test-only native differential observer for scalar programs.
use ottd_script::{Execution, Value, Vm, compile_bytes};
use std::{env, error::Error, fs};
fn value(value: &Value) {
    match value {
        Value::String(bytes) => {
            print!("string {} ", bytes.as_bytes().len());
            for byte in bytes.as_bytes() {
                print!("{byte:02x}");
            }
            println!();
        }
        Value::Array(_) => println!("unsupported 134217792"),
        Value::Null => println!("null"),
        Value::Integer(n) => println!("integer {n}"),
        Value::Float(bits) => println!("float {bits}"),
        Value::Bool(b) => println!("bool {}", u8::from(*b)),
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let filename = args.next().ok_or("source path required")?;
    if filename == "--format" {
        return formats(&args.next().ok_or("bit-pattern file required")?);
    }
    let source = fs::read(filename)?;
    let program = match compile_bytes(&source) {
        Ok(program) => program,
        Err(error) => {
            println!("compile_error");
            eprintln!("{error}");
            return Ok(());
        }
    };
    println!("stack {}", program.stack_size());
    for literal in program.literals() {
        print!("literal ");
        value(literal);
    }
    for i in program.instructions() {
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
                value(&result);
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

fn formats(filename: &str) -> Result<(), Box<dyn Error>> {
    use ottd_script::{Instruction, Program, Realm};
    for token in fs::read_to_string(filename)?.split_whitespace() {
        let bits: u32 = token.parse()?;
        let program = Program::from_parts(
            3,
            vec![Value::String(Realm::new().string(b""))],
            vec![
                Instruction {
                    opcode: 1,
                    arg0: 1,
                    arg1: 0,
                    arg2: 0,
                    arg3: 0,
                },
                Instruction {
                    opcode: 3,
                    arg0: 2,
                    arg1: i32::from_ne_bytes(bits.to_ne_bytes()),
                    arg2: 0,
                    arg3: 0,
                },
                Instruction {
                    opcode: 0x11,
                    arg0: 1,
                    arg1: 2,
                    arg2: 1,
                    arg3: b'+',
                },
                Instruction {
                    opcode: 0x13,
                    arg0: 1,
                    arg1: 1,
                    arg2: 0,
                    arg3: 0,
                },
            ],
        )?;
        let Execution::Returned(result) = Vm::new(&program)?.resume(100)? else {
            return Err("unexpected suspension".into());
        };
        print!("{bits} ");
        value(&result);
    }
    Ok(())
}
