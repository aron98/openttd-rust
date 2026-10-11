//! Persistent execution owners, separate from shared root slots and compiler state.
use crate::{CompileError, Program, Realm, Storage, Table, Temporary, Value, Vm, VmError};
/// A configured root retaining the ordinary graph-backed physical table identity.
#[derive(Clone, Debug)]
pub struct RootEnvironment {
    pub(crate) realm: Realm,
    pub(crate) table: Table,
}
impl RootEnvironment {
    #[expect(
        clippy::expect_used,
        reason = "The existing empty_root API is infallible; native table creation likewise requires allocation"
    )]
    pub(crate) fn new(realm: Realm) -> Self {
        let table = realm.table().expect("empty root allocation");
        Self { realm, table }
    }
    /// Configure an existing first-class table as a root without copying slots.
    pub fn from_table(table: Table) -> Self {
        Self {
            realm: table.0.realm.clone(),
            table,
        }
    }
    /// Retain this root as an ordinary table value, without copying its slots.
    pub fn table(&self) -> Table {
        self.table.clone()
    }
    /// Read an own byte-key slot without delegates or compiler-constant fallback.
    pub fn raw_get(&self, key: &[u8]) -> Option<Value> {
        self.table
            .raw_get(Value::String(self.realm.string(key)))
            .ok()
            .flatten()
    }
    fn check_realm(&self, value: &Value) -> Result<(), VmError> {
        let same = match value {
            Value::Table(table) => table.same_realm(&self.realm),
            Value::Array(array) => array.same_realm(&self.realm),
            Value::NativeClosure(closure) => closure.same_realm(&self.realm),
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_) => true,
        };
        if same {
            Ok(())
        } else {
            Err(VmError::RealmMismatch)
        }
    }
    /// Insert or replace an own byte-key slot.
    /// # Errors
    /// Rejects foreign graph values before changing the root; allocation can fail.
    pub fn new_slot(&self, key: &[u8], value: Value) -> Result<(), VmError> {
        self.check_realm(&value)?;
        self.table
            .new_slot(Value::String(self.realm.string(key)), value)
    }
    /// Replace an existing own byte-key slot.
    /// # Errors
    /// Preserves host API foreign-value rejection before missing-key lookup.
    pub fn set_existing(&self, key: &[u8], value: Value) -> Result<(), VmError> {
        self.check_realm(&value)?;
        self.table
            .set_existing(Value::String(self.realm.string(key)), value)
    }
    /// Remove an own byte-key slot without fallback lookup.
    /// # Errors
    /// Returns `MissingIndex` if the own slot does not exist.
    pub fn delete_slot(&self, key: &[u8]) -> Result<Value, VmError> {
        self.table
            .delete_slot(Value::String(self.realm.string(key)))
    }
    pub(crate) fn register_defaults(&self) -> Result<(), VmError> {
        use crate::{NativeClosure, NativeFunction};
        for function in [
            NativeFunction::Assert,
            NativeFunction::Print,
            NativeFunction::Array,
            NativeFunction::Type,
        ] {
            let closure = NativeClosure::new(&self.realm, function)?;
            self.new_slot(
                closure.name().as_bytes(),
                Value::NativeClosure(closure.clone()),
            )?;
        }
        self.new_slot(
            b"_version_",
            Value::String(
                self.realm
                    .string(b"Squirrel 2.2.5 stable - With custom OpenTTD modifications"),
            ),
        )?;
        self.new_slot(b"_charsize_", Value::Integer(1))?;
        self.new_slot(b"_intsize_", Value::Integer(8))?;
        self.new_slot(b"_floatsize_", Value::Integer(4))
    }
}
impl Realm {
    /// Allocate and populate the pinned bare VM's original root table.
    /// # Errors
    /// Returns Allocation if table or closure storage cannot be allocated.
    pub fn default_root(&self) -> Result<RootEnvironment, VmError> {
        let root = RootEnvironment {
            realm: self.clone(),
            table: self.table()?,
        };
        root.register_defaults()?;
        Ok(root)
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
    /// Create a friend context sharing the current root and re-registering defaults.
    /// # Errors
    /// Allocation failure may leave the same partial registration as the native host.
    pub fn friend(&self) -> Result<Self, VmError> {
        self.root.register_defaults()?;
        Ok(Self::new(self.root.clone()))
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

#[cfg(test)]
mod native_roots;
