use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) enum Kind {
    Rail,
    Road,
    Ship,
    Aircraft,
}

impl Kind {
    pub(super) const ALL: [Self; 4] = [Self::Rail, Self::Road, Self::Ship, Self::Aircraft];

    pub(super) const fn index(self) -> usize {
        match self {
            Self::Rail => 0,
            Self::Road => 1,
            Self::Ship => 2,
            Self::Aircraft => 3,
        }
    }

    pub(super) const fn offset(self) -> u16 {
        match self {
            Self::Rail => 0,
            Self::Road => 116,
            Self::Ship => 204,
            Self::Aircraft => 215,
        }
    }

    pub(super) const fn count(self) -> u16 {
        match self {
            Self::Rail => 116,
            Self::Road => 88,
            Self::Ship => 11,
            Self::Aircraft => 41,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Mapping {
    pub kind: Kind,
    pub grfid: u32,
    pub internal_id: u16,
    pub substitute_id: u16,
    pub engine: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Mappings {
    pub entries: [Vec<Mapping>; 4],
}

impl Default for Mappings {
    fn default() -> Self {
        Self {
            entries: Kind::ALL.map(|kind| {
                (0..kind.count())
                    .map(|local| Mapping {
                        kind,
                        grfid: u32::MAX,
                        internal_id: local,
                        substitute_id: local,
                        engine: kind.offset().saturating_add(local),
                    })
                    .collect()
            }),
        }
    }
}

impl Mappings {
    pub(super) fn get(&self, kind: Kind, local: u16, scope: u32) -> Option<u16> {
        self.entries
            .get(kind.index())?
            .iter()
            .find(|entry| (entry.grfid, entry.internal_id) == (scope, local))
            .map(|entry| entry.engine)
    }

    pub(super) fn reserve(
        &mut self,
        kind: Kind,
        local: u16,
        scope: u32,
        static_access: bool,
    ) -> Option<u16> {
        let entries = self.entries.get_mut(kind.index())?;
        let entry = entries
            .iter_mut()
            .find(|entry| (entry.grfid, entry.internal_id) == (u32::MAX, local))?;
        let engine = entry.engine;
        if !static_access && scope != u32::MAX {
            entry.grfid = scope;
            entries.sort_by_key(|entry| (entry.grfid, entry.internal_id));
        }
        Some(engine)
    }

    pub(super) fn insert(&mut self, mapping: Mapping) {
        if let Some(entries) = self.entries.get_mut(mapping.kind.index()) {
            if let Some(entry) = entries.iter_mut().find(|entry| {
                (entry.grfid, entry.internal_id) == (mapping.grfid, mapping.internal_id)
            }) {
                entry.engine = mapping.engine;
            } else {
                entries.push(mapping);
                entries.sort_by_key(|entry| (entry.grfid, entry.internal_id));
            }
        }
    }
}
