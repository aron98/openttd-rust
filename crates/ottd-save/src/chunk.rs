use crate::{Error, Reader};

/// OpenTTD's five structural chunk encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkKind {
    /// One length-prefixed binary body.
    Riff,
    /// Consecutive records, including empty object slots.
    Array,
    /// Records prefixed with explicit object indices.
    SparseArray,
    /// Schema header followed by consecutive records.
    Table,
    /// Schema header followed by explicitly indexed records.
    SparseTable,
}

/// A parsed chunk frame retaining its full, uninterpreted body.
#[derive(Debug)]
pub struct Chunk {
    id: [u8; 4],
    mode: u8,
    kind: ChunkKind,
    body: Vec<u8>,
    records: usize,
}

impl Chunk {
    /// Four-byte upstream chunk identifier.
    pub const fn id(&self) -> [u8; 4] {
        self.id
    }

    /// Structural encoding used by this chunk.
    pub const fn kind(&self) -> ChunkKind {
        self.kind
    }

    /// Encoded chunk body, excluding the ID and mode/RIFF length.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Framed record count, including a table's schema and empty array slots.
    pub const fn records(&self) -> usize {
        self.records
    }

    pub(crate) fn write_to(&self, output: &mut Vec<u8>) -> Result<(), Error> {
        output.extend_from_slice(&self.id);
        match self.kind {
            ChunkKind::Riff => {
                let len = u32::try_from(self.body.len())
                    .map_err(|_| Error::Framing("RIFF body exceeds 32 bits"))?;
                let [high, a, b, c] = len.to_be_bytes();
                output.extend_from_slice(&[high << 4, a, b, c]);
            }
            ChunkKind::Array
            | ChunkKind::SparseArray
            | ChunkKind::Table
            | ChunkKind::SparseTable => {
                output.push(self.mode);
            }
        }
        output.extend_from_slice(&self.body);
        Ok(())
    }
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(input: &'a [u8]) -> Self {
        Self { remaining: input }
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let (head, rest) = self
            .remaining
            .split_at_checked(len)
            .ok_or(Error::Truncated)?;
        self.remaining = rest;
        Ok(head)
    }

    pub(crate) fn word(&mut self) -> Result<[u8; 4], Error> {
        let (word, rest) = self
            .remaining
            .split_first_chunk::<4>()
            .ok_or(Error::Truncated)?;
        self.remaining = rest;
        Ok(*word)
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let (value, rest) = self.remaining.split_first().ok_or(Error::Truncated)?;
        self.remaining = rest;
        Ok(*value)
    }

    fn gamma(&mut self) -> Result<usize, Error> {
        let first = self.byte()?;
        let (mut value, extra) = match first {
            0x00..=0x7f => (u32::from(first), 0),
            0x80..=0xbf => (u32::from(first & 0x3f), 1),
            0xc0..=0xdf => (u32::from(first & 0x1f), 2),
            0xe0..=0xef => (u32::from(first & 0x0f), 3),
            0xf0..=0xf7 => (0, 4),
            0xf8..=0xff => return Err(Error::Framing("unsupported gamma integer")),
        };
        for _ in 0..extra {
            value = (value << 8) | u32::from(self.byte()?);
        }
        usize::try_from(value).map_err(|_| Error::Framing("length exceeds platform size"))
    }

    pub(crate) fn chunks(mut self) -> Result<Vec<Chunk>, Error> {
        let mut chunks = Vec::new();
        loop {
            let id = self.word()?;
            if id == [0; 4] {
                if !self.remaining.is_empty() {
                    return Err(Error::Framing("data after final chunk"));
                }
                return Ok(chunks);
            }
            if chunks.len() >= 4096 {
                return Err(Error::ChunkLimit);
            }
            let mode = self.byte()?;
            let kind = match mode & 0xf {
                0 => ChunkKind::Riff,
                1 => ChunkKind::Array,
                2 => ChunkKind::SparseArray,
                3 => ChunkKind::Table,
                4 => ChunkKind::SparseTable,
                _ => return Err(Error::Framing("unknown chunk mode")),
            };
            let (body, records) = match kind {
                ChunkKind::Riff => {
                    let a = self.byte()?;
                    let b = self.byte()?;
                    let c = self.byte()?;
                    let length = u32::from_be_bytes([mode >> 4, a, b, c]);
                    let length = usize::try_from(length)
                        .map_err(|_| Error::Framing("RIFF length exceeds platform size"))?;
                    (self.take(length)?.to_vec(), 0)
                }
                ChunkKind::Array
                | ChunkKind::SparseArray
                | ChunkKind::Table
                | ChunkKind::SparseTable => self.read_array(kind)?,
            };
            chunks.push(Chunk {
                id,
                mode,
                kind,
                body,
                records,
            });
        }
    }

    fn read_array(&mut self, kind: ChunkKind) -> Result<(Vec<u8>, usize), Error> {
        let start = self.remaining;
        let table = matches!(kind, ChunkKind::Table | ChunkKind::SparseTable);
        let sparse = matches!(kind, ChunkKind::SparseArray | ChunkKind::SparseTable);
        let mut records = 0_usize;
        loop {
            let length = self.gamma()?;
            let Some(body_len) = length.checked_sub(1) else {
                if table && records == 0 {
                    return Err(Error::Framing("table has no schema header"));
                }
                let consumed = start.len().saturating_sub(self.remaining.len());
                let body = start.get(..consumed).ok_or(Error::Truncated)?.to_vec();
                return Ok((body, records));
            };
            let mut record = Self::new(self.take(body_len)?);
            if sparse && !(table && records == 0) {
                record.gamma()?;
            }
            records = records.saturating_add(1);
        }
    }
}
