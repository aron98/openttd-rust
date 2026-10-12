//! Typed fields from the independently validated finite native receipts of
//! the pinned observer; raw outputs are compared exactly, unnormalized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VmReceipt {
    pub(super) references: i64,
    pub(super) is_root: i64,
    pub(super) stack_size: i64,
    pub(super) top: i64,
    pub(super) stackbase: i64,
    pub(super) vargs: i64,
    pub(super) callsstack: i64,
    pub(super) etraps: i64,
    pub(super) ops: i64,
}
/// One `roots object` line: the finite native field type, its raw refcount
/// and the extent of table/array payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ObjectReceipt {
    pub(super) kind: ReceiptKind,
    pub(super) references: i64,
    pub(super) extent: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReceiptKind {
    Null,
    Table,
    Array,
    Integer,
    String,
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
/// The `roots vm` line of the first snapshot in this receipt.
pub(super) fn vm_line(raw: &str) -> VmReceipt {
    let line = raw
        .lines()
        .find(|line| line.starts_with("roots vm "))
        .expect("native vm line");
    vm_line_of(line)
}
/// The `roots vm` line of one phase's snapshot.
pub(super) fn vm_line_in(raw: &str, phase: &str) -> VmReceipt {
    let begin = format!("roots begin {phase}");
    let end = format!("roots end {phase}");
    let line = raw
        .lines()
        .skip_while(|line| *line != begin)
        .skip(1)
        .take_while(|line| *line != end)
        .find(|line| line.starts_with("roots vm "))
        .expect("native vm line");
    vm_line_of(line)
}
/// Parse one `roots vm` line into its nine native scalars.
fn vm_line_of(line: &str) -> VmReceipt {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 12);
    assert_eq!(fields.first(), Some(&"roots"));
    assert_eq!(fields.get(1), Some(&"vm"));
    let numbers = fields
        .iter()
        .skip(3)
        .map(|field| field.parse::<i64>().expect("native scalar"))
        .collect::<Vec<_>>();
    let Some(
        [
            references,
            is_root,
            stack_size,
            top,
            stackbase,
            vargs,
            callsstack,
            etraps,
            ops,
        ],
    ) = numbers.first_chunk()
    else {
        panic!("nine native vm scalars")
    };
    VmReceipt {
        references: *references,
        is_root: *is_root,
        stack_size: *stack_size,
        top: *top,
        stackbase: *stackbase,
        vargs: *vargs,
        callsstack: *callsstack,
        etraps: *etraps,
        ops: *ops,
    }
}
/// One `roots object <name>` line inside the given phase.
pub(super) fn object(raw: &str, phase: &str, name: &str) -> ObjectReceipt {
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
    assert_eq!(found.len(), 1, "one native field in this phase");
    object_line(found.first().expect("field"))
}
/// One unique `roots object <name>` line outside any phase; the shared
/// receipt emits its thirteen registered tables between two snapshots.
pub(super) fn shared_object(raw: &str, name: &str) -> ObjectReceipt {
    let prefix = format!("roots object {name} ");
    let found = raw
        .lines()
        .filter(|line| line.starts_with(&prefix))
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "one native shared field");
    object_line(found.first().expect("field"))
}
/// Parse one `roots object` line into its finite observed field.
fn object_line(line: &str) -> ObjectReceipt {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 7);
    let Some(["roots", "object", _, tag, _, references, extent]) = fields.first_chunk() else {
        panic!("seven native object fields")
    };
    let kind = match *tag {
        "16777217" => ReceiptKind::Null,
        "167772192" => ReceiptKind::Table,
        "134217792" => ReceiptKind::Array,
        "83886082" => ReceiptKind::Integer,
        "134217744" => ReceiptKind::String,
        tag => panic!("unexpected finite native field type {tag}"),
    };
    ObjectReceipt {
        kind,
        references: references.parse().expect("native count"),
        extent: extent.parse().expect("native extent"),
    }
}
/// Labeled positions of the given chain occurrence; allocation-order chains
/// come first, rebuilt post-collection chains second.
pub(super) fn chain_positions(raw: &str, occurrence: usize) -> Vec<(usize, char)> {
    let chains = raw
        .lines()
        .filter(|line| line.starts_with("roots chain "))
        .collect::<Vec<_>>();
    let line = chains
        .get(occurrence)
        .unwrap_or_else(|| panic!("native chain {occurrence}"));
    let mut fields = line.split_whitespace();
    assert_eq!(fields.next(), Some("roots"));
    assert_eq!(fields.next(), Some("chain"));
    let count = fields.next().expect("count").parse::<usize>().expect("n");
    let chain = fields.next().expect("chain");
    assert_eq!(chain.chars().count(), count);
    chain
        .chars()
        .enumerate()
        .filter(|(_, character)| *character != '?')
        .collect()
}
/// The `roots collected <phase> <n>` count.
pub(super) fn collected(raw: &str, phase: &str) -> usize {
    let prefix = format!("roots collected {phase} ");
    let values = raw
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1);
    values.first().expect("count").parse().expect("count")
}
/// The labeled free order recorded at the observer's teardown.
pub(super) fn free_order(raw: &str) -> Vec<char> {
    raw.lines()
        .filter_map(|line| line.strip_prefix("free "))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter_map(|label| label.chars().next())
        .collect()
}
