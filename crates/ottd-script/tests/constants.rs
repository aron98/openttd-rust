//! Shared constant declarations observed with pinned native compiler sessions.
use ottd_script::{Execution, Realm, Value, Vm};

#[test]
fn shared_realm_reads_previous_compilation() -> Result<(), Box<dyn std::error::Error>> {
    let realm = Realm::new();
    realm.compile("const K=7;")?;
    let shared = realm.clone();
    drop(realm);
    let program = shared.compile("return K;")?;
    assert_eq!(
        Vm::new(&program)?.resume(100)?,
        Execution::Returned(Value::Integer(7))
    );
    Ok(())
}

#[test]
fn failed_compilation_preserves_published_constant() -> Result<(), Box<dyn std::error::Error>> {
    let realm = Realm::new();
    assert!(realm.compile("const K=7; local =;").is_err());
    let program = realm.compile("return K;")?;
    assert_eq!(
        Vm::new(&program)?.resume(100)?,
        Execution::Returned(Value::Integer(7))
    );
    Ok(())
}

#[test]
fn bounded_constant_rejections_are_not_lexer_policy() -> Result<(), Box<dyn std::error::Error>> {
    use ottd_script::CompileErrorKind;
    let realm = Realm::new();
    realm.compile("const K=7;")?;
    for source in ["return K++;", "const M=-0x8000000000000000;"] {
        assert_eq!(
            realm
                .compile(source)
                .map(|_| ())
                .map_err(|error| error.kind),
            Err(CompileErrorKind::UnsupportedSyntax)
        );
    }
    assert!(matches!(
        realm.compile("K=(;").map_err(|error| error.kind),
        Err(CompileErrorKind::ExpectedToken)
    ));
    assert!(matches!(
        realm.compile("K=2;").map_err(|error| error.kind),
        Err(CompileErrorKind::UnsupportedSyntax)
    ));
    Ok(())
}
