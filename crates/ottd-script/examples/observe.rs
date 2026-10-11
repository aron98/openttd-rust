//! Test-only native differential observer for scalar programs.
use ottd_script::{Execution, Value, Vm};
use std::{cell::RefCell, env, error::Error, fs, io::Write, rc::Rc};
#[derive(Debug)]
struct Prints(Rc<RefCell<Vec<Vec<u8>>>>);
impl ottd_script::PrintSink for Prints {
    fn print(&mut self, bytes: &[u8]) {
        self.0.borrow_mut().push(bytes.to_vec());
    }
}
fn value(value: &Value) {
    match value {
        Value::String(bytes) => {
            print!("string {} ", bytes.as_bytes().len());
            for byte in bytes.as_bytes() {
                print!("{byte:02x}");
            }
            println!();
        }
        Value::NativeClosure(_) => println!("unsupported 134218240"),
        Value::ScriptClosure(_) => println!("unsupported 134217984"),
        Value::Table(_) => println!("unsupported 167772192"),
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
    let realm = ottd_script::Realm::new();
    let mut runner = ottd_script::Runner::new(realm.default_root()?);
    let prints = Rc::new(RefCell::new(Vec::new()));
    let print_file = env::var_os("OTTD_SCRIPT_PRINT_FILE");
    if let Some(file) = &print_file {
        drop(fs::File::create(file)?);
    }
    if print_file.is_some() {
        runner.set_print_sink(Some(Box::new(Prints(Rc::clone(&prints)))));
    }
    let program = match runner.compile_bytes_named(&source, c"fixture") {
        Ok(program) => program,
        Err(error) => {
            println!("compile_error");
            eprintln!("{error}");
            return Ok(());
        }
    };
    let closure_trace = env::var_os("OTTD_SCRIPT_CLOSURE_TRACE").is_some();
    let varargs_trace = env::var_os("OTTD_SCRIPT_VARARGS_TRACE").is_some();
    if closure_trace {
        metadata(&program.prototype(), "main");
    }
    println!("stack {}", program.stack_size());
    for literal in program.literals() {
        print!("literal ");
        value(&literal);
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
    let host = ottd_script::HostStack::new();
    host.push_main(&program);
    let mut vm = runner.start_with_host(&program, host)?;
    for credit in credits {
        match vm.resume(*credit) {
            Ok(Execution::Suspended) => {
                if varargs_trace {
                    varargs(&vm);
                }
                if closure_trace {
                    let active = vm.active_prototype().ok_or("active closure")?;
                    let path = function_path(&program.prototype(), &active, "main")
                        .ok_or("prototype outside main tree")?;
                    let (base, top, count) = vm.frame_position().ok_or("frame coordinates")?;
                    println!(
                        "suspend {} {path} {} {count} {base} {top}",
                        vm.remaining_ops(),
                        vm.instruction_pointer()
                    );
                } else {
                    println!(
                        "suspend {} {}",
                        vm.remaining_ops(),
                        vm.instruction_pointer()
                    );
                }
            }
            Ok(Execution::Returned(result)) => {
                print!("return {} ", vm.remaining_ops());
                value(&result);
                break;
            }
            Err(error) => {
                println!("runtime_error {}", vm.remaining_ops());
                if let Some(error) = vm.native_error() {
                    std::io::stderr().write_all(error.as_bytes())?;
                    std::io::stderr().write_all(b"\n")?;
                } else {
                    eprintln!("{error}");
                }
                break;
            }
        }
    }
    if let Some(file) = print_file {
        let mut file = fs::File::create(file)?;
        for payload in prints.borrow().iter() {
            file.write_all(&u64::try_from(payload.len())?.to_le_bytes())?;
            file.write_all(payload)?;
        }
    }
    Ok(())
}

fn varargs(vm: &Vm<'_>) {
    let (values, base, len) = vm.vararg_state();
    println!("varargs {} {base} {len}", values.len());
    for (index, arg) in values.iter().enumerate() {
        print!("vararg {index} ");
        value(arg);
    }
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

fn metadata(proto: &ottd_script::Prototype, path: &str) {
    let children = proto.children();
    let captures = proto.captures();
    println!(
        "prototype {path} {} {} {} {} {} {}",
        proto.stack_size(),
        proto.parameter_count(),
        children.len(),
        captures.len(),
        proto.default_registers().len(),
        u8::from(proto.is_variadic())
    );
    print!("source {path} ");
    value(
        &proto
            .source_name()
            .cloned()
            .map_or(Value::Null, Value::String),
    );
    print!("name {path} ");
    value(&proto.name().cloned().map_or(Value::Null, Value::String));
    for (label, names) in [
        ("parameter", proto.parameter_names()),
        ("local", proto.local_names()),
        ("capture-name", proto.capture_names()),
    ] {
        for (index, name) in names.iter().enumerate() {
            print!("{label} {path} {index} ");
            value(&Value::String(name.clone()));
        }
    }
    for (index, (kind, source)) in captures.iter().enumerate() {
        print!("outer {path} {index} {kind} ");
        value(source);
    }
    for (index, register) in proto.default_registers().iter().enumerate() {
        println!("default {path} {index} {register}");
    }
    for i in proto.instructions() {
        println!(
            "child-op {path} {} {} {} {} {}",
            i.opcode, i.arg0, i.arg1, i.arg2, i.arg3
        );
    }
    for (index, child) in children.iter().enumerate() {
        metadata(child, &format!("{path}.{index}"));
    }
}
fn function_path(
    root: &ottd_script::Prototype,
    target: &ottd_script::Prototype,
    path: &str,
) -> Option<String> {
    if root.same_identity(target) {
        return Some(path.into());
    }
    root.children()
        .iter()
        .enumerate()
        .find_map(|(index, child)| function_path(child, target, &format!("{path}.{index}")))
}
