//! Native string compilation and ownership.
use ottd_script::{Execution, Value, Vm, compile};

#[test]
fn concatenated_strings_compare_equal_to_literal_when_interned()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a runtime string and an equal literal.
    let source = "local a=\"a\";return (a+\"b\")==\"ab\";";
    // When: the native instruction stream executes.
    let program = compile(source)?;
    let result = Vm::new(&program)?.resume(100)?;
    // Then: live content interning preserves native equality.
    assert_eq!(result, Execution::Returned(Value::Bool(true)));
    Ok(())
}

#[test]
fn returned_string_owns_bytes_when_vm_program_and_realm_are_dropped()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a temporary realm, program, and VM returning embedded NUL bytes.
    let result = {
        let realm = ottd_script::Realm::new();
        let program = realm.compile("local a=\"a\\0\";return a+\"b\";")?;
        // When: the returned value outlives every execution/compile owner.
        Vm::new(&program)?.resume(100)?
    };
    // Then: the immutable byte view remains valid without the realm wrapper.
    let Execution::Returned(Value::String(value)) = result else {
        return Err("missing string".into());
    };
    assert_eq!(value.as_bytes(), b"a\0b");
    Ok(())
}

#[test]
fn byte_source_preserves_native_surrogates_and_ignores_tail_after_nul()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: native codepoint input permits encoded surrogates, and NUL ends the source.
    let source = b"return \"\xed\xa0\x80\";\0\xff";
    // When: the persistent realm compiles raw bytes and executes the literal.
    let program = ottd_script::Realm::new().compile_bytes(source)?;
    let result = Vm::new(&program)?.resume(100)?;
    // Then: native encoded bytes survive without Rust str validation.
    let Execution::Returned(Value::String(value)) = result else {
        return Err("missing string".into());
    };
    assert_eq!(value.as_bytes(), b"\xed\xa0\x80");
    Ok(())
}

#[test]
fn empty_strings_are_truthy_and_encoded_surrogates_order_by_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: native truthiness and distinct encoded surrogate byte strings.
    let truth = ottd_script::compile("return \"\"?7:9;")?;
    let order = ottd_script::compile_bytes(include_bytes!(
        "../../../../scripts/compat/script-vm/fixtures/strings_surrogate_order.nut"
    ))?;
    // When: both real native-compatible instruction streams execute.
    let truth = Vm::new(&truth)?.resume(100)?;
    let order = Vm::new(&order)?.resume(100)?;
    // Then: empty text is truthy, and lossy Unicode replacement cannot collapse ordering.
    assert_eq!(truth, Execution::Returned(Value::Integer(7)));
    assert_eq!(order, Execution::Returned(Value::Bool(true)));
    Ok(())
}

#[test]
fn escape_errors_precede_unread_encoding_and_verbatim_prefix_reads_lookahead() {
    // Given: malformed encoding immediately after an invalid escape or verbatim prefix.
    for (source, kind, offset) in [
        (
            b"return \"\\q\xff\";".as_slice(),
            ottd_script::CompileErrorKind::ExpectedToken,
            9,
        ),
        (
            b"return \"\\n\xff\";".as_slice(),
            ottd_script::CompileErrorKind::InvalidCharacter,
            10,
        ),
        (
            b"return @\xff".as_slice(),
            ottd_script::CompileErrorKind::InvalidCharacter,
            8,
        ),
        (
            b"return @a\xff".as_slice(),
            ottd_script::CompileErrorKind::ExpectedToken,
            8,
        ),
    ] {
        // When: native character requests stop at the first lexical error.
        let error = ottd_script::compile_bytes(source).expect_err("native lexical failure");
        // Then: the callback is neither advanced too early nor skipped.
        assert_eq!((error.kind, error.offset), (kind, offset));
    }
}

#[test]
fn hex_escape_rejects_only_reached_undefined_native_classification() {
    for prefix in [
        b"return \"\\x".as_slice(),
        b"return \"\\x1",
        b"return \"\\x1234",
    ] {
        for (encoded, codepoint) in [
            (b"\xc4\x80".as_slice(), 256),
            (b"\xe2\x82\xac", 0x20ac),
            (b"\xed\xa0\x80", 0xd800),
        ] {
            let source = [prefix, encoded, b"\";"].concat();
            let error = ottd_script::compile_bytes(&source).expect_err("undefined hex lookahead");
            assert_eq!(error.offset, prefix.len());
            assert_eq!(
                error.kind,
                ottd_script::CompileErrorKind::UndefinedNativeCharacter {
                    codepoint,
                    context: ottd_script::NativeCharacterContext::HexEscape,
                }
            );
        }
    }
}

#[test]
fn plain_unicode_strings_and_defined_hex_terminators_stay_supported()
-> Result<(), Box<dyn std::error::Error>> {
    for (source, expected) in [
        (
            b"return \"\xe2\x82\xac\";".as_slice(),
            b"\xe2\x82\xac".as_slice(),
        ),
        (b"return \"\xed\xa0\x80\";", b"\xed\xa0\x80"),
        (b"return \"\\x1!\xe2\x82\xac\";", b"\x01!\xe2\x82\xac"),
        (b"return \"\\x1\xc3\xa9\";", b"\x01\xc3\xa9"),
    ] {
        let program = ottd_script::compile_bytes(source)?;
        let Execution::Returned(Value::String(value)) = Vm::new(&program)?.resume(100)? else {
            return Err("missing Unicode string result".into());
        };
        assert_eq!(value.as_bytes(), expected);
    }
    Ok(())
}
