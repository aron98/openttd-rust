//! String allocation and selection from OpenTTD 14ec60f `newgrf_text.cpp` (GPL-2.0-only).
use std::{collections::BTreeMap, convert::Infallible, fmt};

pub(super) const FIRST_ID: u32 = 64 << 11;
pub(super) const CAPACITY: usize = 2048 * 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub(super) struct StringKey {
    pub grfid: u32,
    pub local_id: u32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Definition {
    pub key: StringKey,
    pub language: u8,
    pub new_scheme: bool,
    pub default_id: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TableError<E = Infallible> {
    InvalidId(u32),
    DefaultCycle(u32),
    Resource(&'static str),
    Translation(E),
}
impl<E: fmt::Display> fmt::Display for TableError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(id) => write!(f, "invalid custom string ID {id}"),
            Self::DefaultCycle(id) => write!(f, "host refuses cyclic custom default {id}"),
            Self::Resource(resource) => write!(f, "custom string host limit: {resource}"),
            Self::Translation(error) => write!(f, "custom string translation: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TableError<E> {}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct Entry {
    pub key: StringKey,
    pub default_id: u32,
    pub translations: Vec<(u8, Vec<u8>)>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Limits {
    pub entries: usize,
    pub definitions: usize,
    pub emitted_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: CAPACITY,
            definitions: 1_048_576,
            emitted_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub(super) struct StringTable {
    pub entries: Vec<Entry>,
    index: BTreeMap<StringKey, u32>,
    limits: Limits,
    definitions: usize,
    emitted: usize,
}
impl Default for StringTable {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}
impl StringTable {
    pub(super) fn snapshot_bytes(&self) -> usize {
        self.entries.iter().fold(0_usize, |bytes, entry| {
            bytes
                .saturating_add(std::mem::size_of::<Entry>())
                .saturating_add(entry.translations.iter().fold(0_usize, |bytes, (_, text)| {
                    bytes
                        .saturating_add(std::mem::size_of::<(u8, Vec<u8>)>())
                        .saturating_add(text.len())
                }))
        })
    }
    pub(super) const fn new(limits: Limits) -> Self {
        Self {
            entries: Vec::new(),
            index: BTreeMap::new(),
            limits,
            definitions: 0,
            emitted: 0,
        }
    }

    pub(super) fn lookup(&self, key: StringKey) -> u32 {
        self.index.get(&key).copied().unwrap_or(2)
    }

    pub(super) fn inline_id(&self, grfid: u32, local_id: u16) -> u32 {
        match local_id {
            0xd000..=0xd7ff => self.lookup(StringKey {
                grfid,
                local_id: u32::from(local_id & !0x400),
            }),
            0xd800..=0xffff => self.lookup(StringKey {
                grfid,
                local_id: u32::from(local_id),
            }),
            _ => super::string_ids::map(local_id),
        }
    }

    pub(super) fn define<E>(
        &mut self,
        request: Definition,
        mut translate: impl FnMut(&Self, u8) -> Result<Vec<u8>, E>,
    ) -> Result<u32, TableError<E>> {
        let languages: &[u8] = if request.new_scheme {
            std::slice::from_ref(&request.language)
        } else if request.language & 3 != 0 {
            &[1]
        } else {
            &[2, 3, 4]
        };
        let mut result = 1;
        for &language in languages {
            if !request.new_scheme
                && request.language.trailing_zeros() >= 2
                && request.language & (1_u8 << language) == 0
            {
                continue;
            }
            self.definitions = self
                .definitions
                .checked_add(1)
                .ok_or(TableError::Resource("definitions"))?;
            if self.definitions > self.limits.definitions {
                return Err(TableError::Resource("definitions"));
            }
            let id = if let Some(&id) = self.index.get(&request.key) {
                id
            } else {
                if self.entries.len() == CAPACITY {
                    return Ok(1);
                }
                if self.entries.len() >= self.limits.entries {
                    return Err(TableError::Resource("entries"));
                }
                let index = u32::try_from(self.entries.len())
                    .map_err(|_| TableError::Resource("entries"))?;
                let id = FIRST_ID
                    .checked_add(index)
                    .ok_or(TableError::Resource("entries"))?;
                self.entries.push(Entry {
                    key: request.key,
                    default_id: request.default_id,
                    translations: Vec::new(),
                });
                self.index.insert(request.key, id);
                id
            };
            let bytes = translate(self, language).map_err(TableError::Translation)?;
            self.emitted = self
                .emitted
                .checked_add(bytes.len())
                .ok_or(TableError::Resource("translated bytes"))?;
            if self.emitted > self.limits.emitted_bytes {
                return Err(TableError::Resource("translated bytes"));
            }
            let index = usize::try_from(id.saturating_sub(FIRST_ID))
                .map_err(|_| TableError::InvalidId(id))?;
            let entry = self
                .entries
                .get_mut(index)
                .ok_or(TableError::InvalidId(id))?;
            if let Some((_, previous)) = entry
                .translations
                .iter_mut()
                .find(|(lang, _)| *lang == language)
            {
                *previous = bytes;
            } else {
                entry.translations.push((language, bytes));
            }
            result = id;
        }
        Ok(result)
    }

    fn entry(&self, id: u32) -> Result<&Entry, TableError> {
        let index = id
            .checked_sub(FIRST_ID)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(TableError::InvalidId(id))?;
        self.entries.get(index).ok_or(TableError::InvalidId(id))
    }

    pub(super) fn default_id(&self, id: u32) -> Result<u32, TableError> {
        Ok(self.entry(id)?.default_id)
    }

    pub(super) fn translation(&self, id: u32, selected: u8) -> Result<Option<&[u8]>, TableError> {
        let entry = self.entry(id)?;
        if entry.key.grfid == 0 {
            return Err(TableError::InvalidId(id));
        }
        let mut fallback = None;
        for (language, bytes) in &entry.translations {
            if *language == selected {
                return Ok(Some(bytes));
            }
            if *language == 0x7f || (fallback.is_none() && (*language == 0 || *language == 1)) {
                fallback = Some(bytes.as_slice());
            }
        }
        Ok(fallback)
    }

    pub(super) fn resolve<'a, E>(
        &'a self,
        mut id: u32,
        selected: u8,
        mut builtin: impl FnMut(u32) -> Result<&'a [u8], E>,
    ) -> Result<&'a [u8], TableError<E>> {
        let initial = id;
        for _ in 0..=self.entries.len() {
            if id < FIRST_ID {
                return builtin(id).map_err(TableError::Translation);
            }
            if let Some(text) = self
                .translation(id, selected)
                .map_err(|_| TableError::InvalidId(id))?
            {
                return Ok(text);
            }
            id = self.default_id(id).map_err(|_| TableError::InvalidId(id))?;
        }
        Err(TableError::DefaultCycle(initial))
    }
}
