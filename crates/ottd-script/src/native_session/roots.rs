//! The shared state's owning fields, in source declaration order.
use crate::{ByteString, NativeRegistry, heap::Edge};
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// Source field order of the ten default delegates, as declared in
/// `sqstate.h` and assigned by the shared constructor.
pub(super) const CONSTRUCTION_ORDER: [NativeRegistry; 10] = [
    NativeRegistry::Table,
    NativeRegistry::Array,
    NativeRegistry::String,
    NativeRegistry::Number,
    NativeRegistry::Closure,
    NativeRegistry::Generator,
    NativeRegistry::Thread,
    NativeRegistry::Class,
    NativeRegistry::Instance,
    NativeRegistry::Weakref,
];
/// `SQSharedState::CollectGarbage` root-enqueue order of the ten default
/// delegates: generator and thread precede closure, unlike declaration order.
pub(super) const COLLECT_ORDER: [NativeRegistry; 10] = [
    NativeRegistry::Table,
    NativeRegistry::Array,
    NativeRegistry::String,
    NativeRegistry::Number,
    NativeRegistry::Generator,
    NativeRegistry::Thread,
    NativeRegistry::Closure,
    NativeRegistry::Class,
    NativeRegistry::Instance,
    NativeRegistry::Weakref,
];
/// Interned-string owners of the shared scalar roots: fifteen system strings
/// in exact ledger order, then the eighteen metamethod names.
pub(super) const SYSTEM_STRINGS: [&[u8]; 15] = [
    b"null",
    b"table",
    b"array",
    b"closure",
    b"string",
    b"userdata",
    b"integer",
    b"float",
    b"userpointer",
    b"function",
    b"generator",
    b"thread",
    b"class",
    b"instance",
    b"bool",
];
/// Metamethod names in insertion order; the map key and the string vector
/// each separately own one interned copy.
pub(super) const METAMETHODS: [&[u8]; 18] = [
    b"_add",
    b"_sub",
    b"_mul",
    b"_div",
    b"_unm",
    b"_modulo",
    b"_set",
    b"_get",
    b"_typeof",
    b"_nexti",
    b"_cmp",
    b"_call",
    b"_cloned",
    b"_newslot",
    b"_delslot",
    b"_tostring",
    b"_newmember",
    b"_inherited",
];
/// Single internal ownership edges for the shared fields, plus the interned
/// string owners; no edge retains the realm or creates an `Rc` cycle.
pub(super) struct SharedRoots {
    pub(super) primary: Option<Edge>,
    pub(super) registry: Option<Edge>,
    pub(super) consts: Option<Edge>,
    pub(super) meta: Option<Edge>,
    pub(super) defaults: Vec<(NativeRegistry, Edge)>,
    pub(super) system_strings: Vec<ByteString>,
    pub(super) metamethods: Vec<ByteString>,
    pub(super) constructor: Option<ByteString>,
    pub(super) types: Vec<ByteString>,
}
impl Debug for SharedRoots {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("SharedRoots")
            .field("defaults", &self.defaults.len())
            .field("system_strings", &self.system_strings.len())
            .field("metamethods", &self.metamethods.len())
            .finish_non_exhaustive()
    }
}
/// A zero-reference construction token for a table whose owner edge was
/// published into a context field; registration still mutates through it.
#[derive(Debug)]
pub(super) struct TrackedTable {
    pub(super) token: crate::heap::Unpublished,
}
impl SharedRoots {
    pub(super) const fn empty() -> Self {
        Self {
            primary: None,
            registry: None,
            consts: None,
            meta: None,
            defaults: Vec::new(),
            system_strings: Vec::new(),
            metamethods: Vec::new(),
            constructor: None,
            types: Vec::new(),
        }
    }
    /// The default-delegate edge for one registry, by declaration identity.
    #[expect(
        clippy::expect_used,
        reason = "A fully constructed session owns exactly ten default tables"
    )]
    pub(super) fn default(&self, registry: NativeRegistry) -> &Edge {
        self.defaults
            .iter()
            .find(|(candidate, _)| *candidate == registry)
            .map(|(_, edge)| edge)
            .expect("ten constructed default delegates")
    }
    /// Root-enqueue order of the whole shared state, mirroring
    /// `SQSharedState::CollectGarbage`: primary context, physical `RefTable`
    /// objects, registry, consts, metamethod map, then the ten default
    /// delegates in generator/thread-before-closure order.
    pub(super) fn collect_order(&self) -> Vec<&Edge> {
        let mut roots = Vec::new();
        if let Some(primary) = &self.primary {
            roots.push(primary);
        }
        roots.extend(
            [&self.registry, &self.consts, &self.meta]
                .into_iter()
                .flatten(),
        );
        for registry in COLLECT_ORDER {
            roots.push(self.default(registry));
        }
        roots
    }
}
