//! Identities and registration metadata of the pinned bare VM's native functions.
use crate::{
    ByteString, Realm, VmError,
    heap::{Handle, Object, Stored},
};
mod catalog;
mod function;
mod object_catalog;
mod registration;
pub use function::{DefaultFunction, NativeFunction};
pub use registration::{NativeRegistration, NativeRegistry};
#[derive(Clone, Debug)]
pub(crate) struct Data {
    name: ByteString,
    registration: &'static NativeRegistration,
    type_checks: Vec<i64>,
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
        let descriptor = match function {
            NativeFunction::Assert
            | NativeFunction::Print
            | NativeFunction::Array
            | NativeFunction::Type => NativeRegistry::Base
                .registrations()
                .iter()
                .find(|row| row.function() == function)
                .ok_or(VmError::InvalidBytecode)?,
            NativeFunction::Default(function) => {
                return Err(VmError::UnsupportedNativeFunction(function));
            }
        };
        Self::registered(realm, descriptor)
    }
    pub(crate) fn registered(
        realm: &Realm,
        registration: &'static NativeRegistration,
    ) -> Result<Self, VmError> {
        let data = Self::data(realm, registration)?;
        realm
            .heap
            .allocate(realm, Object::NativeClosure(data))
            .map(Self)
    }
    /// Allocate a fresh zero-reference closure for source-order registration;
    /// only the actual table-slot or field assignment retains it.
    pub(crate) fn unpublished(
        realm: &Realm,
        registration: &'static NativeRegistration,
    ) -> Result<crate::heap::Unpublished, VmError> {
        let data = Self::data(realm, registration)?;
        realm.heap.unpublished(realm, Object::NativeClosure(data))
    }
    fn data(realm: &Realm, registration: &'static NativeRegistration) -> Result<Data, VmError> {
        let mut type_checks = Vec::new();
        type_checks
            .try_reserve_exact(registration.type_checks().len())
            .map_err(|_| VmError::Allocation)?;
        type_checks.extend_from_slice(registration.type_checks());
        Ok(Data {
            name: realm.string(registration.name()),
            registration,
            type_checks,
            environment: Stored::null(),
            outer_values: Vec::new(),
        })
    }
    #[expect(
        clippy::unreachable,
        reason = "Native closure handles only refer to native closure entries"
    )]
    fn read<T>(&self, read: impl FnOnce(&Data) -> T) -> T {
        self.0.read(|object| match object {
            Object::NativeClosure(data) => read(data),
            Object::Table(_) | Object::Array(_) | Object::ScriptClosure(_) | Object::Context(_) => {
                unreachable!()
            }
        })
    }
    /// Interned native registration name.
    pub fn name(&self) -> ByteString {
        self.read(|data| data.name.clone())
    }
    /// Original native implementation identity, distinct from callable support.
    pub fn function(&self) -> NativeFunction {
        self.read(|data| data.registration.function())
    }
    /// Native arity check, including negative minimum-arity encoding.
    pub fn parameter_check(&self) -> i64 {
        self.read(|data| data.registration.parameter_check())
    }
    /// Native compiled argument type masks; -1 admits any type, 6 admits numeric.
    pub fn type_checks(&self) -> &'static [i64] {
        self.read(|data| data.registration.type_checks())
    }
    /// Number of masks in this object's separately owned native vector.
    pub fn type_check_count(&self) -> usize {
        self.read(|data| data.type_checks.len())
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

#[cfg(test)]
mod invocation_tests;
#[cfg(test)]
mod tests;
