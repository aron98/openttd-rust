//! Native comment token boundaries and switch control ownership.
use ottd_script::{Execution, Value, Vm, compile};

#[test]
fn comment_newlines_preserve_native_return_boundary() -> Result<(), Box<dyn std::error::Error>> {
    for (source, expected) in [
        ("return/*\n*/1;", Value::Integer(1)),
        ("return// text\n1;", Value::Null),
    ] {
        let program = compile(source)?;
        assert_eq!(
            Vm::new(&program)?.resume(100)?,
            Execution::Returned(expected)
        );
    }
    Ok(())
}

#[test]
fn switch_case_comparison_overwrites_named_operand() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile("local a=2,b=2;switch(a){case b:break;}return b;")?;
    assert_eq!(
        Vm::new(&program)?.resume(100)?,
        Execution::Returned(Value::Bool(true))
    );
    Ok(())
}

#[test]
fn switch_continue_uses_enclosing_loop_target() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile(
        "local s=0;for(local i=0;i<4;i++){switch(i){case 1:continue;case 2:break;default:s+=i;}s+=10;}return s;",
    )?;
    assert_eq!(
        Vm::new(&program)?.resume(100)?,
        Execution::Returned(Value::Integer(33))
    );
    Ok(())
}

#[test]
fn comment_eof_and_offsets_use_original_bytes() {
    for source in ["/*", "return 1;/*", "/*π\n", "/*x\0*/"] {
        let error = compile(source).expect_err("unterminated block comment");
        assert_eq!(error.kind, ottd_script::CompileErrorKind::ExpectedToken);
        assert_eq!(error.offset, source.find('\0').unwrap_or(source.len()));
    }
}

#[test]
fn comment_nul_and_control_bytes_follow_native_callback() -> Result<(), Box<dyn std::error::Error>>
{
    for (source, expected) in [
        ("return//x\0ignored\n1;", Value::Null),
        ("return/*\u{000b}\u{000c} π */1;", Value::Integer(1)),
        ("local a=1;a//x\n++a;return a;", Value::Integer(2)),
    ] {
        let program = compile(source)?;
        assert_eq!(
            Vm::new(&program)?.resume(100)?,
            Execution::Returned(expected)
        );
    }
    Ok(())
}

#[test]
fn switch_selector_alias_and_fallthrough_keep_native_order()
-> Result<(), Box<dyn std::error::Error>> {
    for (source, expected) in [
        (
            "local a=1;switch(a){case(a=3):return a;default:return 9;}",
            Value::Bool(true),
        ),
        (
            "local a=0;switch(1){case 1:a++;case 1/0:a++;}return a;",
            Value::Integer(2),
        ),
        (
            "local a=7;switch(1){case 1:{local a=2;}case 2:return a;}return 9;",
            Value::Integer(7),
        ),
    ] {
        let program = compile(source)?;
        assert_eq!(
            Vm::new(&program)?.resume(100)?,
            Execution::Returned(expected)
        );
    }
    Ok(())
}

#[test]
fn switch_malformed_labels_and_unowned_continue_fail_compilation() {
    for source in [
        "local a=1;a/*\n*/++a;return a;",
        "switch(1){case 1,2:return 3;}",
        "switch(1){default:;case 1:;}",
        "switch(1){case 1:continue;}",
        "case 1:return 2;",
        "switch(1){default:;default:;}",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
}
