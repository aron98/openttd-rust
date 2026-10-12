//! Typed fields from the complete independently validated finite native receipt.
use crate::heap::observation::ValueKind;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct NativeObject {
    pub(super) kind: ValueKind,
    pub(super) references: usize,
}
pub(super) fn receipt(mode: &str) -> String {
    let directory = std::env::var("OTTD_SCRIPT_NATIVE_ROOT_CAPTURE_DIR")
        .expect("required actual pinned native root capture directory");
    let raw =
        std::fs::read_to_string(std::path::Path::new(&directory).join(format!("{mode}-on.stdout")))
            .expect("actual selected native stdout");
    assert!(raw.ends_with("recorder 1\n"));
    raw
}
fn object(line: &str) -> NativeObject {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 7);
    assert_eq!(fields.first(), Some(&"roots"));
    assert_eq!(fields.get(1), Some(&"object"));
    let kind = match fields.get(3).copied().expect("type") {
        "16777217" => ValueKind::Null,
        "83886082" => ValueKind::Integer,
        "134217744" => ValueKind::String,
        "167772192" => ValueKind::Table,
        "134217792" => ValueKind::Array,
        tag => panic!("unexpected finite native root field type {tag}"),
    };
    NativeObject {
        kind,
        references: fields
            .get(5)
            .expect("references")
            .parse()
            .expect("native count"),
    }
}
pub(super) fn field(raw: &str, phase: &str, name: &str) -> NativeObject {
    let begin = format!("roots begin {phase}");
    let end = format!("roots end {phase}");
    let prefix = format!("roots object {name} ");
    let found = raw
        .lines()
        .skip_while(|line| *line != begin)
        .skip(1)
        .take_while(|line| *line != end)
        .filter(|line| line.starts_with(&prefix))
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "one field in this actual snapshot");
    object(found.first().expect("field"))
}
pub(super) fn shared_table_minimum(raw: &str) -> usize {
    let labels = "RCMTASNFGHKIW";
    for label in labels.chars() {
        let prefix = format!("roots object {label} ");
        let fields = raw
            .lines()
            .filter(|line| line.starts_with(&prefix))
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), 1);
        let line = fields.first().expect("shared field");
        let observed = object(line);
        assert_eq!(observed.kind, ValueKind::Table);
        assert_eq!(observed.references, 1);
        assert_eq!(
            line.split_whitespace().nth(4),
            Some(label.to_string().as_str())
        );
    }
    assert_eq!(field(raw, "idle", "root").kind, ValueKind::Table);
    assert!(
        raw.lines()
            .any(|line| line.starts_with("roots object root 167772192 ? 1 "))
    );
    // The unlabeled root is distinct from the thirteen registered table identities.
    labels
        .len()
        .checked_add(1)
        .expect("finite known table bound")
}
pub(super) fn collected(raw: &str, phase: &str) -> usize {
    let prefix = format!("roots collected {phase} ");
    let values = raw
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1);
    values.first().expect("collection").parse().expect("count")
}
