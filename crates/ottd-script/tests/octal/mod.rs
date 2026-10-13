//! Defined native char-narrowed octal continuation versus reached byte ctype.
use ottd_script::{CompileErrorKind, Execution, NativeCharacterContext, Value, Vm, compile_bytes};

#[test]
fn narrowed_octal_continuations_keep_native_parse_failure_zero()
-> Result<(), Box<dyn std::error::Error>> {
    for payload in [
        b"\xc4\xb0".as_slice(),
        b"\xc4\xb1",
        b"\xc4\xb2",
        b"\xc4\xb3",
        b"\xc4\xb4",
        b"\xc4\xb5",
        b"\xc4\xb6",
        b"\xc4\xb7",
        b"\xc8\xb0",
        b"\xed\xa0\xb0",
        b"\xef\xbc\xb0",
        b"\xef\xbc\xb7",
    ] {
        let source = [b"return 07", payload, b";"].concat();
        let program = compile_bytes(&source)?;
        assert_eq!(
            Vm::new(&program)?.resume(100)?,
            Execution::Returned(Value::Integer(0))
        );
    }
    Ok(())
}

#[test]
fn octal_continuation_preserves_later_decoder_and_number_diagnostics() {
    for (source, kind, offset) in [
        (
            b"return 07\xc4\xb08;".as_slice(),
            CompileErrorKind::InvalidNumber,
            11,
        ),
        (
            b"return 07\xc4\xb0\xff;",
            CompileErrorKind::InvalidCharacter,
            11,
        ),
        (
            b"return 07\xc4\xb0\xc4\x80;",
            CompileErrorKind::UndefinedNativeCharacter {
                codepoint: 256,
                context: NativeCharacterContext::Number,
            },
            11,
        ),
        (
            b"return 0\xc4\xb0;",
            CompileErrorKind::UndefinedNativeCharacter {
                codepoint: 304,
                context: NativeCharacterContext::Number,
            },
            8,
        ),
    ] {
        let error = compile_bytes(source).expect_err("reached number boundary");
        assert_eq!(error.kind, kind);
        assert_eq!(error.offset, offset);
    }
}

#[test]
fn octal_continuation_keeps_nul_suffix_unread() -> Result<(), Box<dyn std::error::Error>> {
    let program = compile_bytes(b"return 07\xc4\xb0\0\xff")?;
    assert_eq!(
        Vm::new(&program)?.resume(100)?,
        Execution::Returned(Value::Integer(0))
    );
    Ok(())
}
