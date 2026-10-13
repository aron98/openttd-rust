use super::{Case, Result, define, programs, property};

pub(super) fn cases() -> Result<Vec<Case>> {
    let mut cases = super::cases()?;
    let a = || programs::source(7, &[property(0, &[0xd800])?, define(0xd800, b'A')], &[], 1);
    let b = || programs::source(8, &[property(0, &[0xd800])?, define(0xd800, b'B')], &[], 2);
    cases.push(("overwrite-reverse", vec![b()?, a()?], None));
    cases.push((
        "missing-custom",
        vec![programs::source(
            7,
            &[property(0, &[0xd800, 0xffff])?],
            &[],
            1,
        )?],
        None,
    ));
    cases.push((
        "builtin-unknown-empty",
        vec![programs::source(
            7,
            &[property(0, &[0, 1, 0x47, 0x600, 0x800, 0x4000, 0x7fff])?],
            &[],
            2,
        )?],
        None,
    ));
    cases.push((
        "custom-callback-aliases",
        vec![programs::source(
            7,
            &[
                property(0, &[0xd000, 0xd400, 0xd800, 0xffff])?,
                define(0xd000, b'A'),
                define(0xd800, b'B'),
                define(0xffff, b'C'),
            ],
            &[],
            1,
        )?],
        None,
    ));
    let mut init_only = a()?;
    init_only.flags.init_only = true;
    cases.push(("init-only-no-owner-write", vec![init_only], None));
    cases.push(("empty-configured-files", vec![], None));
    cases.push((
        "zero-item-property",
        vec![programs::source(7, &[property(0, &[])?], &[], 1)?],
        None,
    ));
    cases.push((
        "maximum-count-wrap",
        vec![programs::source(
            7,
            &[property(255, &vec![0xd800; 255])?, define(0xd800, b'A')],
            &[],
            2,
        )?],
        None,
    ));
    let mut malformed = property(46, &[0xd800])?;
    malformed.pop();
    cases.push((
        "malformed-invalid-owner",
        vec![programs::source(
            7,
            std::slice::from_ref(&malformed),
            &[],
            1,
        )?],
        None,
    ));
    cases.push((
        "malformed-after-valid-record",
        vec![programs::source(
            7,
            &[property(0, &[0xd800])?, malformed],
            &[],
            2,
        )?],
        None,
    ));
    let mut repeated = Vec::new();
    for id in [0xd800, 0, 0xd801, 0xffff, 0xd800] {
        repeated.push(property(31, &[id])?);
    }
    repeated.extend([define(0xd800, b'A'), define(0xd801, b'B')]);
    cases.push((
        "repeated-custom-owner",
        vec![programs::source(7, &repeated, &[], 1)?],
        None,
    ));
    cases.push(("same-local-distinct-grfid", vec![a()?, b()?], None));
    cases.push(("reload-same-config", vec![a()?], Some(vec![0])));
    cases.push(("reload-remove-first", vec![a()?, b()?], Some(vec![1])));
    cases.push(("reload-remove-second", vec![a()?, b()?], Some(vec![0])));
    Ok(cases)
}

#[test]
fn expanded_currency_roster_is_unique_and_buildable() -> Result {
    let cases = cases()?;
    let names = cases
        .iter()
        .map(|(name, _, _)| *name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(cases.len(), 20);
    assert_eq!(names.len(), cases.len());
    assert_eq!(
        cases
            .iter()
            .filter(|(_, _, reload)| reload.is_some())
            .count(),
        5
    );
    Ok(())
}
