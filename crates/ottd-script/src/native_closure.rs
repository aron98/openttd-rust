//! Identities and registration metadata of the pinned bare VM's native functions.
use crate::{
    ByteString, Realm, VmError,
    heap::{Handle, Object, Stored},
};
/// Source-defined native function identity for the four bare-root builtins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFunction {
    /// The bare VM assertion function.
    Assert,
    /// The bare VM print function.
    Print,
    /// The bare VM array constructor.
    Array,
    /// The bare VM type function.
    Type,
}
impl NativeFunction {
    const fn name(self) -> &'static [u8] {
        match self {
            Self::Assert => b"assert",
            Self::Print => b"print",
            Self::Array => b"array",
            Self::Type => b"type",
        }
    }
}
#[derive(Debug)]
pub(crate) struct Data {
    name: ByteString,
    function: NativeFunction,
    parameter_check: i64,
    type_checks: &'static [i64],
    environment: Stored,
    pub(crate) outer_values: Vec<Stored>,
}
impl Data {
    pub(crate) fn values(&self) -> impl Iterator<Item = &Stored> {
        self.outer_values.iter()
    }
}
/// A real graph object retaining native registration identity and metadata.
#[derive(Clone, Debug)]
pub struct NativeClosure(pub(crate) Handle);
impl NativeClosure {
    pub(crate) fn new(realm: &Realm, function: NativeFunction) -> Result<Self, VmError> {
        realm
            .heap
            .allocate(
                realm,
                Object::NativeClosure(Data {
                    name: realm.string(function.name()),
                    function,
                    parameter_check: if function == NativeFunction::Array {
                        -2
                    } else {
                        2
                    },
                    type_checks: if function == NativeFunction::Array {
                        &[-1, 6]
                    } else {
                        &[]
                    },
                    environment: Stored::null(),
                    outer_values: Vec::new(),
                }),
            )
            .map(Self)
    }
    #[expect(
        clippy::unreachable,
        reason = "Native closure handles only refer to native closure entries"
    )]
    fn read<T>(&self, read: impl FnOnce(&Data) -> T) -> T {
        self.0.read(|object| match object {
            Object::NativeClosure(data) => read(data),
            Object::Table(_) | Object::Array(_) | Object::ScriptClosure(_) => unreachable!(),
        })
    }
    /// Interned native registration name.
    pub fn name(&self) -> ByteString {
        self.read(|data| data.name.clone())
    }
    /// Original native implementation identity, distinct from callable support.
    pub fn function(&self) -> NativeFunction {
        self.read(|data| data.function)
    }
    /// Native arity check, including negative minimum-arity encoding.
    pub fn parameter_check(&self) -> i64 {
        self.read(|data| data.parameter_check)
    }
    /// Native compiled argument type masks; -1 admits any type, 6 admits numeric.
    pub fn type_checks(&self) -> &'static [i64] {
        self.read(|data| data.type_checks)
    }
    /// Number of captured outer values in this registered native closure.
    pub fn outer_value_count(&self) -> usize {
        self.read(|data| data.outer_values.len())
    }
    /// Retain the native closure's environment value.
    pub fn environment(&self) -> crate::Value {
        self.read(|data| data.environment.clone())
            .value(&self.0.realm)
    }
    /// Whether both owners refer to the same native closure object.
    pub fn same_identity(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub(crate) fn same_realm(&self, realm: &Realm) -> bool {
        self.0.realm.same(realm)
    }
    #[cfg(test)]
    pub(crate) fn watch(&self) -> impl Fn() -> Option<Self> + use<> {
        let watch = self.0.watch();
        move || watch().map(Self)
    }
}
impl PartialEq for NativeClosure {
    fn eq(&self, other: &Self) -> bool {
        self.same_identity(other)
    }
}
impl Eq for NativeClosure {}
