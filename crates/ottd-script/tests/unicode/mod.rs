//! Native compilebuffer codepoint admission and lazy EOF behavior.
use ottd_script::compile;
#[test]
fn supplementary_codepoints_fail_when_inside_comments() {
    // Given: supplementary characters consumed inside either comment form.
    for source in ["return/*🚆*/1;", "return//🚆\n1;"] {
        // When: native compilebuffer decoding reaches that character.
        let result = compile(source);
        // Then: MAX_CHAR is 0xffff even inside comments.
        assert!(result.is_err(), "{source}");
    }
}

#[test]
fn invalid_utf8_is_rejected_when_the_callback_consumes_it() {
    // Given: malformed encoding or a valid but out-of-range codepoint.
    for (source, offset) in [
        (b"return/*\xff*/1;".as_slice(), 8),
        (b"return/*\xc0\x80*/1;".as_slice(), 8),
        (b"return/*\xf0\x90\x80\x80*/1;".as_slice(), 8),
        (b"return 1;\xff".as_slice(), 9),
        (b"return/*\xcf\x80*/1;\xff".as_slice(), 14),
    ] {
        // When: lazy lookahead reaches the invalid character.
        let error = ottd_script::compile_bytes(source).expect_err("invalid callback character");
        // Then: the original byte position and native failure category are retained.
        assert_eq!(error.kind, ottd_script::CompileErrorKind::InvalidCharacter);
        assert_eq!(error.offset, offset);
    }
}

#[test]
fn nul_stops_decoding_when_malformed_bytes_follow_it() -> Result<(), Box<dyn std::error::Error>> {
    // Given: native callback EOF before otherwise invalid bytes.
    for source in [b"return 1;\0\xff".as_slice(), b"return 1\0+9;".as_slice()] {
        // When: compiling and executing the consumed prefix.
        let program = ottd_script::compile_bytes(source)?;
        let result = ottd_script::Vm::new(&program)?.resume(100)?;
        // Then: no decoder or whole-input UTF-8 validation reaches the suffix.
        assert_eq!(
            result,
            ottd_script::Execution::Returned(ottd_script::Value::Integer(1))
        );
    }
    Ok(())
}

#[test]
fn native_bmp_surrogates_are_accepted_when_ignored_inside_comments()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: DecodeUtf8 accepts encoded surrogates and the highest BMP codepoint.
    for source in [
        b"return/*\xed\xa0\x80*/1;".as_slice(),
        b"return/*\xef\xbf\xbf*/1;".as_slice(),
    ] {
        // When: the comment consumes those codepoints.
        let program = ottd_script::compile_bytes(source)?;
        let result = ottd_script::Vm::new(&program)?.resume(100)?;
        // Then: Rust str's stricter Unicode-scalar validation does not change native behavior.
        assert_eq!(
            result,
            ottd_script::Execution::Returned(ottd_script::Value::Integer(1))
        );
    }
    Ok(())
}

#[test]
fn unterminated_comment_wins_when_nul_precedes_invalid_utf8() {
    // Given: native callback EOF inside a block comment, before invalid input.
    let source = b"return/*\0\xff*/1;";
    // When: block-comment parsing observes EOF.
    let error = ottd_script::compile_bytes(source).expect_err("unterminated block comment");
    // Then: the decoder never consumes the invalid suffix.
    assert_eq!(error.kind, ottd_script::CompileErrorKind::ExpectedToken);
    assert_eq!(error.offset, 8);
}

#[test]
fn earlier_syntax_error_wins_when_bad_encoding_is_not_looked_ahead() {
    // Given: a statement boundary error before a distant malformed byte.
    let source = b"return 1 2;\xff";
    // When: the compiler refuses the extra expression first.
    let error = ottd_script::compile_bytes(source).expect_err("missing statement boundary");
    // Then: a whole-input decoder must not replace the native earlier failure.
    assert_eq!(error.kind, ottd_script::CompileErrorKind::ExpectedToken);
    assert_eq!(error.offset, 9);
}
