//! Test-only borrowed graph snapshots and actual retirement events.
use super::{Heap, Object, ObjectId};
use crate::Table;
use std::fmt::Write;

pub(super) struct Probe<'a> {
    heap: &'a Heap,
    identities: Vec<(char, ObjectId)>,
    pub(super) output: String,
}
impl<'a> Probe<'a> {
    pub(super) fn new(heap: &'a Heap) -> Self {
        *heap.retired.borrow_mut() = Some(Vec::with_capacity(64));
        Self {
            heap,
            identities: Vec::with_capacity(16),
            output: String::new(),
        }
    }
    pub(super) fn register(&mut self, label: char, table: &Table) {
        self.identities.push((label, table.0.edge.life.id));
    }
    fn label(&self, id: ObjectId) -> char {
        let label = self
            .identities
            .iter()
            .find_map(|(label, current)| (*current == id).then_some(*label));
        assert!(label.is_some(), "unregistered retirement generation");
        label.unwrap_or('?')
    }
    pub(super) fn snapshot(&mut self, phase: &str) -> std::fmt::Result {
        writeln!(self.output, "snapshot {phase}")?;
        let entries = self.heap.entries.borrow();
        for (label, id) in &self.identities {
            let live = entries
                .get(id.index)
                .filter(|entry| entry.generation == id.generation)
                .and_then(|entry| Some((entry.life.as_ref()?, entry.object.as_ref()?)));
            if let Some((life, Object::Table(data))) = live {
                let parent = data
                    .delegate
                    .as_ref()
                    .map_or('-', |edge| self.label(edge.life.id));
                writeln!(
                    self.output,
                    "object {label} 1 {} {} {parent}",
                    life.references.get(),
                    data.nodes.len()
                )?;
            } else {
                writeln!(self.output, "object {label} 0")?;
            }
        }
        drop(entries);
        if let Some(trace) = self.heap.retired.borrow_mut().as_mut() {
            for id in trace.drain(..) {
                let label = self.label(id);
                writeln!(self.output, "free {label}")?;
            }
        }
        Ok(())
    }
    pub(super) fn chain(&mut self) -> std::fmt::Result {
        self.output.push_str("chain");
        for id in self.heap.chain.borrow().iter() {
            if self.identities.iter().any(|(_, current)| current == id) {
                let label = self.label(*id);
                write!(self.output, " {label}")?;
            }
        }
        self.output.push('\n');
        Ok(())
    }
    pub(super) fn finish(&mut self) {
        for (_, id) in &self.identities {
            assert!(
                self.heap
                    .entries
                    .borrow()
                    .get(id.index)
                    .is_none_or(|entry| entry.generation != id.generation || entry.life.is_none())
            );
        }
        self.output.push_str("recorder 1\n");
    }
}

pub(super) fn native(mode: &str, frozen: &str) -> std::io::Result<String> {
    let raw = if let Ok(directory) = std::env::var("OTTD_SCRIPT_DELEGATE_OWNER_DIR") {
        std::fs::read_to_string(std::path::Path::new(&directory).join(format!("{mode}.txt")))?
    } else {
        frozen.to_owned()
    };
    let mut common = String::new();
    for line in raw.lines() {
        if let Some(event) = line.strip_prefix("free ") {
            let fields = event.split_whitespace().collect::<Vec<_>>();
            assert_eq!(fields.len(), 2);
            assert_eq!(fields.first().map(|label| label.len()), Some(1));
            assert_eq!(
                fields.get(1),
                Some(&"88"),
                "native SQTable allocation bytes are checked separately from Rust layout"
            );
            common.push_str("free ");
            common.push_str(fields.first().copied().unwrap_or("?"));
        } else {
            common.push_str(line);
        }
        common.push('\n');
    }
    Ok(common)
}
