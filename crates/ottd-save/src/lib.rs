//! Lossless save-container handling. Game-object fields are not yet interpreted.

mod chunk;
mod compression;

pub use chunk::{Chunk, ChunkKind};

#[derive(Debug)]
struct Reader<'a> {
    remaining: &'a [u8],
}

/// Maximum save version emitted by the pinned OpenTTD 15.3 release.
pub const SAVEGAME_VERSION: u16 = 362;

/// Default maximum size of both the encoded file and the decoded payload.
pub const DEFAULT_MAX_BYTES: usize = 256 * 1024 * 1024;

/// Compression formats understood by OpenTTD 15.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Uncompressed data (`OTTN`).
    None,
    /// Zlib stream (`OTTZ`).
    Zlib,
    /// XZ container (`OTTX`).
    Lzma,
    /// Checksummed LZO blocks (`OTTD`).
    Lzo,
}

/// A structurally validated container, not a validated game state.
#[derive(Debug)]
pub struct Savegame {
    version_bytes: [u8; 4],
    compression: Compression,
    chunks: Vec<Chunk>,
}

/// Invalid or unsupported save input.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The container is not recognized.
    #[error("unrecognized save container")]
    Format,
    /// The version predates the supported header or exceeds the pinned baseline.
    #[error("save version {0} is unsupported (supported: 1..=362, excluding patchpacks 220..=286)")]
    Version(u16),
    /// A read crossed the end of a field, record, or compressed stream.
    #[error("truncated save data")]
    Truncated,
    /// The encoded or decoded data exceeds the caller's byte budget.
    #[error("save exceeds the {0}-byte size limit")]
    SizeLimit(usize),
    /// Excessive chunk count would amplify small inputs into large allocations.
    #[error("save exceeds the 4096-chunk structural limit")]
    ChunkLimit,
    /// A chunk mode, record boundary, or terminal marker is invalid.
    #[error("invalid chunk framing: {0}")]
    Framing(&'static str),
    /// Compression failed or a checksum was invalid.
    #[error("compression error: {0}")]
    Compression(#[from] std::io::Error),
    /// A block did not contain a valid LZO stream.
    #[error("LZO error: {0}")]
    Lzo(String),
}

impl Savegame {
    /// Decode a container with an encoded and decoded byte limit.
    ///
    /// # Errors
    /// Returns an error for unsupported, invalid, or oversized input.
    pub fn decode(input: &[u8], max_bytes: usize) -> Result<Self, Error> {
        if input.len() > max_bytes {
            return Err(Error::SizeLimit(max_bytes));
        }
        let (header, encoded) = input.split_first_chunk::<8>().ok_or(Error::Truncated)?;
        let [a, b, c, d, major_high, major_low, minor, reserved] = *header;
        let compression = match &[a, b, c, d] {
            b"OTTN" => Compression::None,
            b"OTTZ" => Compression::Zlib,
            b"OTTX" => Compression::Lzma,
            b"OTTD" => Compression::Lzo,
            _ => return Err(Error::Format),
        };
        let version = u16::from_be_bytes([major_high, major_low]);
        if version == 0 || version > SAVEGAME_VERSION || (220..=286).contains(&version) {
            return Err(Error::Version(version));
        }
        let payload = compression.decode(encoded, max_bytes)?;
        Ok(Self {
            version_bytes: [major_high, major_low, minor, reserved],
            compression,
            chunks: Reader::new(&payload).chunks()?,
        })
    }

    /// Serialize the decoded container using the requested compression.
    ///
    /// # Errors
    /// Returns an error if compression fails.
    pub fn encode(&self, compression: Compression) -> Result<Vec<u8>, Error> {
        let mut payload = Vec::new();
        for chunk in &self.chunks {
            chunk.write_to(&mut payload)?;
        }
        payload.extend_from_slice(&[0; 4]);
        let mut output = match compression {
            Compression::None => b"OTTN".to_vec(),
            Compression::Zlib => b"OTTZ".to_vec(),
            Compression::Lzma => b"OTTX".to_vec(),
            Compression::Lzo => b"OTTD".to_vec(),
        };
        output.extend_from_slice(&self.version_bytes);
        compression.encode(&payload, &mut output)?;
        Ok(output)
    }

    /// Original save version; rewriting never upgrades the version.
    pub const fn version(&self) -> u16 {
        let [high, low, _, _] = self.version_bytes;
        u16::from_be_bytes([high, low])
    }

    /// Compression used by the input file.
    pub const fn compression(&self) -> Compression {
        self.compression
    }

    /// Chunks in their original order, including opaque game-object data.
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }
}
