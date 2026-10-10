//! Persistent execution owners, separate from shared root slots and compiler state.
use crate::{ByteString, CompileError, Program, Realm, Storage, Temporary, Value, Vm, VmError};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

type Slots = BTreeMap<Vec<u8>, (ByteString, Value)>;
/// Shared plain root with byte keys, scalar values and same-realm scalar arrays.
#[derive(Clone, Debug)]
pub struct RootEnvironment {
    pub(crate) realm: Realm,
    slots: Rc<RefCell<Slots>>,
}
impl RootEnvironment {
    pub(crate) fn new(realm: Realm) -> Self {
        Self {
            realm,
            slots: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.slots, &other.slots)
    }
    /// Read an own slot without delegates or compiler-constant fallback.
    pub fn raw_get(&self, key: &[u8]) -> Option<Value> {
        self.slots.borrow().get(key).map(|(_, value)| value.clone())
    }
    fn intern(&self, value: Value) -> Result<Value, VmError> {
        match value {
            Value::String(bytes) => Ok(Value::String(self.realm.string(bytes.as_bytes()))),
            Value::Array(array) => {
                if !array.same_realm(&self.realm) {
                    return Err(VmError::RealmMismatch);
                }
                Ok(Value::Array(array))
            }
            Value::Null | Value::Bool(_) | Value::Integer(_) | Value::Float(_) => Ok(value),
        }
    }
    /// Insert or replace an own slot, retaining scalar bytes or array identity.
    ///
    /// # Errors
    /// Rejects an array from another realm before changing the root.
    pub fn new_slot(&self, key: &[u8], value: Value) -> Result<(), VmError> {
        let value = self.intern(value)?;
        let name = self.realm.string(key);
        self.slots.borrow_mut().insert(key.to_vec(), (name, value));
        Ok(())
    }
    /// Replace an existing own slot.
    ///
    /// # Errors
    /// Returns `RealmMismatch` for a foreign array before looking up the key.
    /// Otherwise returns `MissingIndex` without inserting when the key is absent.
    pub fn set_existing(&self, key: &[u8], value: Value) -> Result<(), VmError> {
        let value = self.intern(value)?;
        let mut slots = self.slots.borrow_mut();
        let (_, target) = slots.get_mut(key).ok_or(VmError::MissingIndex)?;
        *target = value;
        Ok(())
    }
    pub(crate) fn lookup(&self, key: &[u8], this: &Self, current: &Self) -> Result<Value, VmError> {
        if let Some(value) = self.raw_get(key) {
            return Ok(value);
        }
        if matches!(
            key,
            b"len"
                | b"rawget"
                | b"rawset"
                | b"rawdelete"
                | b"rawin"
                | b"weakref"
                | b"tostring"
                | b"clear"
        ) {
            return Err(VmError::UnsupportedRuntimeValue);
        }
        if self.same(this) {
            if let Some(value) = current.raw_get(key) {
                return Ok(value);
            }
            if let Some(binding) = current.realm.constant_bytes(key) {
                return match binding {
                    crate::realm::constants::Binding::Scalar(value) => Ok(value.value()),
                    crate::realm::constants::Binding::Enum(_) => {
                        Err(VmError::UnsupportedRuntimeValue)
                    }
                };
            }
        }
        Err(VmError::MissingIndex)
    }
}
/// Rust diagnostic history, not the native error object's string identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerFailure {
    /// A source compile failed.
    Compile(CompileError),
    /// A main frame failed at runtime or reached an unsupported runtime domain.
    Runtime(VmError),
}
/// Persistent configured execution context with independent temporary ownership.
#[derive(Debug)]
pub struct Runner {
    pub(crate) root: RootEnvironment,
    pub(crate) temporary: Temporary,
    pub(crate) failure: Option<RunnerFailure>,
}
impl Runner {
    /// Create an idle context sharing this root, with no prior temporary or failure.
    pub const fn new(root: RootEnvironment) -> Self {
        Self {
            root,
            temporary: Temporary::Value(Value::Null),
            failure: None,
        }
    }
    /// Compile in this runner's realm without replacing its runtime temporary.
    ///
    /// # Errors
    /// Returns and records the compiler diagnostic on failure.
    pub fn compile(&mut self, source: &str) -> Result<Program, CompileError> {
        self.compile_bytes(source.as_bytes())
    }
    /// Compile lazy native buffer bytes in this runner's realm.
    ///
    /// # Errors
    /// Returns and records the compiler diagnostic on failure.
    pub fn compile_bytes(&mut self, source: &[u8]) -> Result<Program, CompileError> {
        self.root.realm.compile_bytes(source).inspect_err(|error| {
            self.failure = Some(RunnerFailure::Compile(*error));
        })
    }
    /// Borrow this context for one main frame; the program's immutable data is shared.
    ///
    /// # Errors
    /// Rejects a program from another realm or malformed stack size.
    pub fn start<'a>(&'a mut self, program: &Program) -> Result<Vm<'a>, VmError> {
        if !self.root.realm.same(&program.data.realm) {
            return Err(VmError::RealmMismatch);
        }
        Vm::with_runner(program, Storage::Borrowed(self))
    }
    /// Install another configured root while this runner is idle.
    ///
    /// # Errors
    /// Rejects a different realm without changing the current root.
    pub fn replace_root(&mut self, root: RootEnvironment) -> Result<(), VmError> {
        if !self.root.realm.same(&root.realm) {
            return Err(VmError::RealmMismatch);
        }
        self.root = root;
        Ok(())
    }
    /// Last Rust diagnostic; successful calls leave this history intact.
    pub const fn last_failure(&self) -> Option<RunnerFailure> {
        self.failure
    }
}
impl Storage<'_> {
    pub(crate) const fn get(&self) -> &Runner {
        match self {
            Self::Owned(runner) => runner,
            Self::Borrowed(runner) => runner,
        }
    }
    pub(crate) const fn get_mut(&mut self) -> &mut Runner {
        match self {
            Self::Owned(runner) => runner,
            Self::Borrowed(runner) => runner,
        }
    }
}

#[cfg(test)]
mod tests;
