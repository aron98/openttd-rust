//! Immutable descriptors are the finite bridge for the static mask API.
use super::{NativeFunction, catalog, object_catalog};
/// A pinned registration row; constructing host-defined rows is not admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeRegistration {
    name: &'static [u8],
    function: NativeFunction,
    parameter_check: i64,
    typemask: Option<&'static str>,
    type_checks: &'static [i64],
}
impl NativeRegistration {
    pub(super) const fn new(
        name: &'static [u8],
        function: NativeFunction,
        parameter_check: i64,
        typemask: Option<&'static str>,
        type_checks: &'static [i64],
    ) -> Self {
        Self {
            name,
            function,
            parameter_check,
            typemask,
            type_checks,
        }
    }
    /// Exact registered byte name, including alias spelling.
    pub const fn name(&self) -> &'static [u8] {
        self.name
    }
    /// Callback code identity, shared by aliases.
    pub const fn function(&self) -> NativeFunction {
        self.function
    }
    /// Signed arity including the receiver; negative values denote a minimum.
    pub const fn parameter_check(&self) -> i64 {
        self.parameter_check
    }
    /// Literal source typemask, preserving spaces and absent masks.
    pub const fn typemask(&self) -> Option<&'static str> {
        self.typemask
    }
    /// Source-compiled signed masks; each closure additionally owns its vector.
    pub const fn type_checks(&self) -> &'static [i64] {
        self.type_checks
    }
}
/// Native registration families; no default table is allocated by this catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeRegistry {
    /// Pinned `table` registrations.
    Table,
    /// Pinned `array` registrations.
    Array,
    /// Pinned `string` registrations.
    String,
    /// Pinned `number` registrations.
    Number,
    /// Pinned `closure` registrations.
    Closure,
    /// Pinned `generator` registrations.
    Generator,
    /// Pinned `thread` registrations.
    Thread,
    /// Pinned `class` registrations.
    Class,
    /// Pinned `instance` registrations.
    Instance,
    /// Pinned `weakref` registrations.
    Weakref,
    /// Pinned `base` registrations.
    Base,
}
impl NativeRegistry {
    /// Ten default families in shared-constructor order, followed by base globals.
    pub const ALL: &'static [Self] = &[
        Self::Table,
        Self::Array,
        Self::String,
        Self::Number,
        Self::Closure,
        Self::Generator,
        Self::Thread,
        Self::Class,
        Self::Instance,
        Self::Weakref,
        Self::Base,
    ];
    /// Stable source family name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::Array => "array",
            Self::String => "string",
            Self::Number => "number",
            Self::Closure => "closure",
            Self::Generator => "generator",
            Self::Thread => "thread",
            Self::Class => "class",
            Self::Instance => "instance",
            Self::Weakref => "weakref",
            Self::Base => "base",
        }
    }
    /// Exact ordered rows; obtaining descriptors creates no runtime objects.
    pub const fn registrations(self) -> &'static [NativeRegistration] {
        match self {
            Self::Table => catalog::TABLE,
            Self::Array => catalog::ARRAY,
            Self::String => catalog::STRING,
            Self::Number => catalog::NUMBER,
            Self::Closure => object_catalog::CLOSURE,
            Self::Generator => object_catalog::GENERATOR,
            Self::Thread => object_catalog::THREAD,
            Self::Class => object_catalog::CLASS,
            Self::Instance => object_catalog::INSTANCE,
            Self::Weakref => object_catalog::WEAKREF,
            Self::Base => catalog::BASE,
        }
    }
}
