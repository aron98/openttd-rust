//! Independently generated files compared with original native scanner functions.
use ottd_sim::content::grf::{GrfContainer, RecordKind, ScanFailure, ScanStatus, scan_file};
use serde_json::{Value, json};
pub mod grf_cases;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn encode(actions: &[Vec<u8>], version: u8) -> Result<Vec<u8>> {
    let mut bytes = if version == 1 {
        Vec::new()
    } else {
        b"\0\0GRF\x82\r\n\x1a\n\0\0\0\0\0".to_vec()
    };
    for action in std::iter::once(vec![0; 4]).chain(actions.iter().cloned()) {
        if version == 1 {
            bytes.extend(u16::try_from(action.len())?.to_le_bytes());
        } else {
            bytes.extend(u32::try_from(action.len())?.to_le_bytes());
        }
        bytes.push(255);
        bytes.extend(action);
    }
    bytes.extend(vec![0; if version == 1 { 2 } else { 4 }]);
    if version == 2 {
        let offset = u32::try_from(bytes.len().checked_sub(14).ok_or("offset")?)?;
        bytes
            .get_mut(10..14)
            .ok_or("offset")?
            .copy_from_slice(&offset.to_le_bytes());
        bytes.extend([0; 4]);
    }
    Ok(bytes)
}
fn action8(version: u8, id: [u8; 4], name: &str) -> Vec<u8> {
    let mut bytes = vec![8, version];
    bytes.extend(id);
    bytes.extend(name.as_bytes());
    bytes.push(0);
    bytes.extend(b"description\0");
    bytes
}
fn project(bytes: &[u8], trace: bool) -> Result<Value> {
    let scan = scan_file(bytes)?;
    let identity = scan
        .identity
        .map_or(Value::Null, |v| json!({"grfid":v.grfid,"md5":v.md5}));
    let failure=scan.failure.map_or(Value::Null,|(reason,line)|json!({"reason":match reason{ScanFailure::ReadBounds=>"ReadBounds",ScanFailure::UnexpectedSprite=>"UnexpectedSprite"},"line":line}));
    let mut value = json!({"failure":failure,"accepted":scan.accepted,"status":match scan.status{ScanStatus::Unknown=>0,ScanStatus::Disabled=>1},"grfid":scan.metadata.as_ref().map_or(0,|v|v.grfid),"identity":identity,"invalid_version":scan.invalid_version,"system":scan.system});
    if let Some(metadata) = scan.metadata {
        value
            .as_object_mut()
            .ok_or("projection")?
            .insert("name".into(), json!(std::str::from_utf8(metadata.name)?));
        if let Some(info) = metadata.info {
            value
                .as_object_mut()
                .ok_or("projection")?
                .insert("info".into(), json!(std::str::from_utf8(info)?));
        }
    }
    if trace {
        let file = GrfContainer::parse(bytes)?;
        let records:Vec<_>=file.records.iter().map(|r|json!({"start":r.span.start,"end":r.span.end,"length":r.declared_length,"type":match r.kind{RecordKind::Pseudo(_)=>255,RecordKind::SpriteReference(_)=>253,RecordKind::InlineSprite{flags,..}=>flags}})).collect();
        let sprites: Vec<_> = file
            .sprites
            .iter()
            .map(
                |r| json!({"id":r.id,"start":r.span.start,"end":r.span.end,"length":r.bytes.len()}),
            )
            .collect();
        value.as_object_mut().ok_or("projection")?.insert(
            "trace".into(),
            json!({"version":file.version,"records":records,"sprites":sprites,"checksum_extent":file.checksum_extent}),
        );
    }
    Ok(value)
}
fn compare(native: &Value, rust: &Value) -> Result {
    if native != rust {
        return Err("native scan mismatch".into());
    }
    Ok(())
}
fn run(root: &Path, directory: &Path, bytes: &[u8], trace: bool) -> Result {
    std::fs::create_dir(directory)?;
    let input = directory.join("input.grf");
    std::fs::write(&input, bytes)?;
    let output = Command::new("cmake")
        .arg(format!("-DORACLE={}", std::env::var("OTTD_GRF_ORACLE")?))
        .arg(format!("-DRUN_DIR={}", directory.join("native").display()))
        .arg(format!("-DGRF={}", input.display()))
        .arg(format!(
            "-DINPUT={}",
            root.join("fixtures/replay/clear-v362.sav").display()
        ))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg(format!("-DTRACE={trace}"))
        .arg("-P")
        .arg(root.join("scripts/check-grf-scan-reference.cmake"))
        .output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    if !output.status.success() {
        return Err(format!("native scan failed: {}", directory.display()).into());
    }
    let native: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/scan.json"))?)?;
    let rust = project(bytes, trace)?;
    std::fs::write(directory.join("rust.json"), serde_json::to_vec(&rust)?)?;
    compare(&native, &rust).map_err(|error| format!("{error}: {}", directory.display()))?;
    for key in ["grfid", "status", "accepted"] {
        let mut wrong = rust.clone();
        *wrong.get_mut(key).ok_or("negative field")? = json!("deliberate wrong value");
        assert!(compare(&native, &wrong).is_err());
        std::fs::write(
            directory.join(format!("negative-{key}.json")),
            serde_json::to_vec(&wrong)?,
        )?;
    }
    for (name, pointer) in [
        ("md5", "/identity/md5"),
        ("record-end", "/trace/records/0/end"),
    ] {
        let mut wrong = rust.clone();
        if let Some(field) = wrong.pointer_mut(pointer) {
            *field = json!("deliberate wrong value");
            assert!(compare(&native, &wrong).is_err());
            std::fs::write(
                directory.join(format!("negative-{name}.json")),
                serde_json::to_vec(&wrong)?,
            )?;
        }
    }
    std::fs::write(
        directory.join("comparison.txt"),
        "PASS native identity/status/metadata/trace; altered controls rejected\n",
    )?;
    Ok(())
}

#[test]
fn generated_native_cases_are_structurally_admitted() -> Result {
    for (_, bytes, trace) in grf_cases::additional()? {
        project(&bytes, trace)?;
    }
    Ok(())
}

#[test]
#[ignore = "requires original scanner observer; set OTTD_GRF_ORACLE and OTTD_GRF_NATIVE_DIR"]
fn native_file_scan_matrix() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_NATIVE_DIR")?);
    if directory.exists() {
        return Err("fresh directory required".into());
    }
    std::fs::create_dir_all(&directory)?;
    let directory = directory.canonicalize()?;
    run(
        &root,
        &directory.join("unchanged-contract"),
        include_bytes!("../../../fixtures/content/contract-speed.grf"),
        true,
    )?;
    let mut count = 1_u32;
    for (name, bytes, trace) in grf_cases::additional()? {
        run(&root, &directory.join(name), &bytes, trace)?;
        count = count.saturating_add(1);
    }
    for container in [1, 2] {
        for version in [0, 1, 2, 7, 8, 9, 255] {
            for (kind, id) in [
                ("normal", *b"TEST"),
                ("system", [255, 65, 66, 67]),
                ("zero", [0; 4]),
            ] {
                let actions = vec![
                    vec![0x0d, 0, 0, 255, 0, 3, 0, 0, 0],
                    vec![0x07, 0, 1, 0, 0, 1],
                    action8(version, id, "first"),
                    action8(8, *b"LAST", "last"),
                ];
                run(
                    &root,
                    &directory.join(format!("v{container}-language{version}-{kind}")),
                    &encode(&actions, container)?,
                    true,
                )?;
                count = count.saturating_add(1);
            }
        }
        for (name, actions) in [
            ("missing", vec![vec![0x0c, 1, 2, 3]]),
            ("short8", vec![vec![8, 8]]),
            (
                "skip",
                vec![
                    vec![1, 0, 1, 1],
                    action8(8, *b"FAIL", "skipped"),
                    action8(8, *b"TEST", "kept"),
                ],
            ),
        ] {
            run(
                &root,
                &directory.join(format!("v{container}-{name}")),
                &encode(&actions, container)?,
                true,
            )?;
            count = count.saturating_add(1);
        }
    }
    std::fs::write(
        directory.join("summary.txt"),
        format!("PASS {count} original FILESCAN cases; changed controls retained per case\n"),
    )?;
    Ok(())
}
