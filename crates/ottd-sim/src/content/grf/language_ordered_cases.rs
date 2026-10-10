use super::{
    Pack, Result,
    cases::{Case, base, records},
    programs,
    text_cases::Query,
};
use std::path::Path;

fn snapshot_queries(case: &mut Case) {
    let mut queries = Vec::new();
    for query in &case.queries {
        for line in [1, 2, 3, 4] {
            queries.push(Query {
                id: format!("{}-line{line}", query.id),
                stage: query.stage,
                file: query.file,
                line,
                grfid: query.grfid,
                language: query.language,
                newlines: query.newlines,
                raw: query.raw.clone(),
            });
        }
    }
    case.queries.extend(queries);
}

fn map_records(bytes: &[u8]) -> Result<Vec<Case>> {
    let pack = Pack::header(bytes)?;
    let mut cases = Vec::new();
    for (name, program) in [
        (
            "maps-after-disable",
            vec![
                super::mapping(&pack, true)?,
                programs::condition(9, 0x84, 4, 3, 0x201, u32::MAX, 1),
                vec![6],
                vec![0x0c],
            ],
        ),
        (
            "maps-action6",
            vec![
                programs::set(0, 0, 255, 255, 3),
                vec![6, 0, 1, 6, 255],
                vec![0, 8, 1, 1, pack.language, 0x15, 0],
            ],
        ),
        (
            "maps-mixed-properties",
            vec![vec![
                0,
                8,
                3,
                1,
                pack.language,
                0x13,
                1,
                0,
                0,
                0x14,
                1,
                0,
                0,
                0x15,
                4,
            ]],
        ),
    ] {
        cases.push(base(name.into(), bytes, &program)?);
    }
    for action in [7, 9] {
        for expected in [0, 1] {
            cases.push(base(
                format!("maps-skip-{action}-{expected}"),
                bytes,
                &[
                    programs::set(0, 0, 255, 255, 1),
                    programs::condition(action, 0, 1, 2, expected, 255, 1),
                    super::mapping(&pack, true)?,
                    vec![0, 8, 1, 1, pack.language, 0x15, 2],
                ],
            )?);
        }
    }
    for reverse in [false, true] {
        let mut record = vec![0, 8, 1, 1, pack.language, 0x13];
        let pairs = if reverse {
            [(1, 1), (2, 0), (1, 0)]
        } else {
            [(1, 0), (2, 0), (1, 1)]
        };
        for (id, index) in pairs {
            record.push(id);
            record.extend(
                pack.genders
                    .get(index)
                    .ok_or("gender slot")?
                    .iter()
                    .copied()
                    .take_while(|byte| *byte != 0),
            );
            record.push(0);
        }
        record.push(0);
        let mut case = base(
            format!("maps-duplicate-{reverse}"),
            bytes,
            &[
                record,
                vec![0, 8, 1, 1, pack.language, 0x15, 3],
                vec![0, 8, 1, 1, pack.language, 0x15, 1],
            ],
        )?;
        snapshot_queries(&mut case);
        cases.push(case);
    }
    Ok(cases)
}

pub(super) fn all(directory: &Path, bytes: &[u8]) -> Result<Vec<Case>> {
    let pack = Pack::header(bytes)?;
    let mut cases = map_records(bytes)?;
    for reverse in [false, true] {
        let mut case = base(format!("two-files-{reverse}"), bytes, &records(&pack)?)?;
        case.sources.push(programs::source(
            0x4242_4242,
            &[vec![0, 8, 1, 1, pack.language, 0x15, 2]],
            &[],
            2,
        )?);
        if reverse {
            case.sources.reverse();
        }
        cases.push(case);
        let mut duplicate = base(
            format!("duplicate-grfid-later-init-only-{reverse}"),
            bytes,
            &records(&pack)?,
        )?;
        let mut second = programs::source(
            0x4141_4141,
            &[vec![0, 8, 1, 1, pack.language, 0x15, 2]],
            &[],
            1,
        )?;
        second.name = "second.grf".into();
        duplicate.sources.push(second);
        if reverse {
            duplicate.sources.reverse();
        }
        duplicate
            .sources
            .get_mut(1)
            .ok_or("second duplicate")?
            .flags
            .init_only = true;
        cases.push(duplicate);
    }
    let mut alias = base("filename-alias".into(), bytes, &records(&pack)?)?;
    let mut second = programs::source(0x4141_4141, &records(&pack)?, &[], 1)?;
    second.parameters = vec![77];
    alias.sources.push(second);
    cases.push(alias);
    for name in ["english", "german", "russian"] {
        let selected = std::fs::read(directory.join(format!("{name}.lng")))?;
        let mut case = base(
            format!("selected-{name}-defined-czech"),
            bytes,
            &records(&pack)?,
        )?;
        case.selected = Pack::header(&selected)?.language;
        case.packs.push(selected);
        case.queries = super::text_cases::all(&pack, 0x4141_4141)?;
        cases.push(case);
    }
    Ok(cases)
}
