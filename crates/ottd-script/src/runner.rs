//! Persistent execution owners, separate from shared root slots and compiler state.
use crate::RootSlot;
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
            Value::ScriptClosure(closure) => closure.same_realm(&self.realm),
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
/// Synchronous host print callback receiving the exact native string bytes.
pub trait PrintSink: std::fmt::Debug {
    /// Emit one callback payload, without adding delimiters or changing NUL bytes.
    fn print(&mut self, bytes: &[u8]);
}
/// Persistent configured execution context with independent temporary ownership.
#[derive(Debug)]
pub struct Runner {
    pub(crate) root: RootSlot<RootEnvironment>,
    pub(crate) native_error: RootSlot<Value>,
    pub(crate) temporary: RootSlot<Temporary>,
    pub(crate) failure: Option<RunnerFailure>,
    pub(crate) print_sink: Option<Box<dyn PrintSink>>,
}
impl Runner {
    /// Create an idle context sharing this root, with no prior temporary or failure.
    pub const fn new(root: RootEnvironment) -> Self {
        Self {
            root: RootSlot::new(root),
            native_error: RootSlot::new(Value::Null),
            temporary: RootSlot::new(Temporary::Value(Value::Null)),
            failure: None,
            print_sink: None,
        }
    }
    /// Replace the synchronous native print callback; absence still performs conversion.
    pub fn set_print_sink(&mut self, sink: Option<Box<dyn PrintSink>>) {
        self.print_sink = sink;
    }
    /// Last native compiler or execution error bytes. Successful operations retain this object.
    pub const fn native_error(&self) -> Option<&crate::ByteString> {
        if let Value::String(message) = self.native_error.as_ref() {
            Some(message)
        } else {
            None
        }
    }
    /// Borrow the retained native error object without converting or copying it.
    pub const fn native_error_value(&self) -> &Value {
        self.native_error.as_ref()
    }
    /// Release the retained native error object, preserving Rust diagnostic history.
    pub fn reset_native_error(&mut self) {
        self.native_error.set(Value::Null);
    }
    /// Create a friend context sharing the current root and re-registering defaults.
    /// # Errors
    /// Allocation failure may leave the same partial registration as the native host.
    pub fn friend(&self) -> Result<Self, VmError> {
        self.root.as_ref().register_defaults()?;
        Ok(Self::new(self.root.as_ref().clone()))
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
        self.compile_bytes_named(source, c"buffer")
    }
    /// Compile bytes with a retained source name without replacing the runtime temporary.
    /// # Errors
    /// Returns and records the compiler diagnostic on failure.
    pub fn compile_bytes_named(
        &mut self,
        source: &[u8],
        name: &std::ffi::CStr,
    ) -> Result<Program, CompileError> {
        crate::compiler::compile_in(&self.root.as_ref().realm, source, name.to_bytes()).map_err(
            |failure| {
                self.failure = Some(RunnerFailure::Compile(failure.error));
                self.native_error.set(
                    failure
                        .native_message(&self.root.as_ref().realm)
                        .map_or(Value::Null, Value::String),
                );
                failure.error
            },
        )
    }
    /// Borrow this context for one main frame; the program's immutable data is shared.
    ///
    /// # Errors
    /// Rejects a program from another realm or malformed stack size.
    pub fn start<'a>(&'a mut self, program: &Program) -> Result<Vm<'a>, VmError> {
        if !self.root.as_ref().realm.same(&program.data.0.realm) {
            return Err(VmError::RealmMismatch);
        }
        Vm::with_runner(program, Storage::Borrowed(self))
    }
    /// Borrow this runner for a frame above actual retained host entries.
    /// # Errors
    /// Rejects foreign programs/entries or an invalid frame shape.
    pub fn start_with_host<'a>(
        &'a mut self,
        program: &Program,
        host: crate::HostStack,
    ) -> Result<Vm<'a>, VmError> {
        if !self.root.as_ref().realm.same(&program.data.0.realm) {
            return Err(VmError::RealmMismatch);
        }
        Vm::with_host_runner(program, Storage::Borrowed(self), host)
    }
    /// Install another configured root while this runner is idle.
    ///
    /// # Errors
    /// Rejects a different realm without changing the current root.
    pub fn replace_root(&mut self, root: RootEnvironment) -> Result<(), VmError> {
        if !self.root.as_ref().realm.same(&root.realm) {
            return Err(VmError::RealmMismatch);
        }
        self.root.set(root);
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

#[cfg(test)]
mod error_state;

#[cfg(test)]
mod observation;
#[cfg(test)]
mod observation_tests;

#[cfg(test)]
mod observation_native;
#[cfg(test)]
mod observation_red;
