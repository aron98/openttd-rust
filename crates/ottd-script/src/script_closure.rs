//! Graph closure ownership; prototype storage never owns this graph.
use crate::{
    Prototype, Realm, VmError,
    heap::{Handle, Object, Stored},
};
use std::rc::Rc;

#[derive(Debug)]
pub(crate) struct Data {
    // C++ member destruction releases defaults, then outers, then the prototype.
    pub(crate) defaults: Vec<Stored>,
    pub(crate) outers: Vec<Stored>,
    pub(crate) prototype: Rc<crate::prototype::Data>,
}
impl Data {
    pub(crate) fn values(&self) -> impl Iterator<Item = &Stored> {
        self.outers.iter().chain(&self.defaults)
    }
}

/// A native script closure with shared immutable code and graph-owned captures.
#[derive(Clone, Debug)]
pub struct ScriptClosure(pub(crate) Handle);
impl ScriptClosure {
    pub(crate) fn new(
        realm: &Realm,
        prototype: Rc<crate::prototype::Data>,
    ) -> Result<Self, VmError> {
        realm
            .heap
            .allocate(
                realm,
                Object::ScriptClosure(Data {
                    prototype,
                    outers: Vec::new(),
                    defaults: Vec::new(),
                }),
            )
            .map(Self)
    }
    #[expect(
        clippy::unreachable,
        reason = "Typed script closure handles only refer to script closure storage"
    )]
    pub(crate) fn read<T>(&self, read: impl FnOnce(&Data) -> T) -> T {
        self.0.read(|object| match object {
            Object::ScriptClosure(data) => read(data),
            Object::Array(_) | Object::Table(_) | Object::NativeClosure(_) | Object::Context(_) => {
                unreachable!()
            }
        })
    }
    /// Retain an independent prototype owner without retaining this closure.
    pub fn prototype(&self) -> Prototype {
        self.read(|data| Prototype(Rc::clone(&data.prototype)))
    }
    /// Whether two handles name the same graph object.
    pub fn same_identity(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub(crate) fn same_realm(&self, realm: &Realm) -> bool {
        self.0.realm.same(realm)
    }
}
impl PartialEq for ScriptClosure {
    fn eq(&self, other: &Self) -> bool {
        self.same_identity(other)
    }
}
impl Eq for ScriptClosure {}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod persistence;

#[cfg(test)]
mod safety;

#[cfg(test)]
mod stores;
