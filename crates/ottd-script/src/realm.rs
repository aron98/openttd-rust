//! Shared compiler declarations and weak string interning.
use crate::{CompileError, Program};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::{Rc, Weak},
};
pub(crate) mod constants;
type Pool = RefCell<BTreeMap<Vec<u8>, Weak<StringData>>>;

/// Shared compiler constants and runtime string identity domain.
#[derive(Clone, Debug, Default)]
pub struct Realm {
    pool: Rc<Pool>,
    constants: Rc<RefCell<constants::Table>>,
}
impl Realm {
    /// Create an independent realm.
    pub fn new() -> Self {
        Self::default()
    }
    #[cfg(test)]
    pub(crate) fn owners(&self, bytes: &[u8]) -> usize {
        self.pool.borrow().get(bytes).map_or(0, Weak::strong_count)
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.pool, &other.pool)
    }
    /// Create an independent empty scalar root in this realm.
    pub fn empty_root(&self) -> crate::RootEnvironment {
        crate::RootEnvironment::new(self.clone())
    }
    /// Compile source using this realm's shared constants and live string interner.
    ///
    /// # Errors
    /// Returns the compiler's source diagnostic for rejected input.
    pub fn compile(&self, source: &str) -> Result<Program, CompileError> {
        self.compile_bytes(source.as_bytes())
    }
    /// Compile native compilebuffer bytes in this realm.
    ///
    /// # Errors
    /// Returns the compiler's diagnostic for consumed invalid or unsupported input.
    pub fn compile_bytes(&self, source: &[u8]) -> Result<Program, CompileError> {
        crate::compiler::compile_in(self, source)
    }
    /// Intern immutable length-delimited bytes, including embedded NUL.
    pub fn string(&self, bytes: &[u8]) -> ByteString {
        if let Some(existing) = self.pool.borrow().get(bytes).and_then(Weak::upgrade) {
            return ByteString(existing);
        }
        let value = Rc::new(StringData {
            bytes: bytes.into(),
            pool: Rc::downgrade(&self.pool),
        });
        self.pool
            .borrow_mut()
            .insert(bytes.to_vec(), Rc::downgrade(&value));
        ByteString(value)
    }
}
#[derive(Debug)]
struct StringData {
    bytes: Box<[u8]>,
    pool: Weak<Pool>,
}
impl Drop for StringData {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            pool.borrow_mut().remove(self.bytes.as_ref());
        }
    }
}
/// Owned immutable native string bytes. Clone retains the allocation.
#[derive(Clone, Debug)]
pub struct ByteString(Rc<StringData>);
impl ByteString {
    /// View all bytes; no UTF-8 or NUL-termination assumption is made.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0.bytes
    }
    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl PartialEq for ByteString {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl Eq for ByteString {}

#[cfg(test)]
mod tests;
