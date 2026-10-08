use std::io::{Read, Write};

use adler2::Adler32;
use flate2::{Decompress, FlushDecompress, Status};
use xz2::{
    bufread::XzDecoder,
    stream::{Check, Stream},
    write::XzEncoder,
};

use crate::{Compression, Error, Reader};

impl Compression {
    pub(crate) fn decode(self, input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
        match self {
            Self::None => Ok(input.to_vec()),
            Self::Zlib => decode_zlib(input, limit),
            Self::Lzma => {
                let stream = Stream::new_auto_decoder(256 * 1024 * 1024, 0)
                    .map_err(|error| Error::Compression(std::io::Error::other(error)))?;
                let mut decoder = XzDecoder::new_stream(input, stream);
                let mut output = Vec::new();
                let take_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
                decoder.by_ref().take(take_limit).read_to_end(&mut output)?;
                if output.len() > limit {
                    return Err(Error::SizeLimit(limit));
                }
                if !decoder.into_inner().is_empty() {
                    return Err(Error::Framing("data after XZ stream"));
                }
                Ok(output)
            }
            Self::Lzo => decode_lzo(input, limit),
        }
    }
}

fn decode_zlib(mut input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let mut decoder = Decompress::new(true);
    let mut output = Vec::new();
    loop {
        let mut buffer = [0; 8192];
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let status = decoder
            .decompress(input, &mut buffer, FlushDecompress::None)
            .map_err(|error| Error::Compression(std::io::Error::other(error)))?;
        let consumed = usize::try_from(decoder.total_in().saturating_sub(before_in))
            .map_err(|_| Error::SizeLimit(limit))?;
        let produced = usize::try_from(decoder.total_out().saturating_sub(before_out))
            .map_err(|_| Error::SizeLimit(limit))?;
        input = input.get(consumed..).ok_or(Error::Truncated)?;
        append_bounded(
            &mut output,
            buffer.get(..produced).ok_or(Error::Truncated)?,
            limit,
        )?;
        if status == Status::StreamEnd {
            if !input.is_empty() {
                return Err(Error::Framing("data after zlib stream"));
            }
            return Ok(output);
        }
        if consumed == 0 && produced == 0 {
            return Err(Error::Truncated);
        }
    }
}

fn decode_lzo(input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let mut reader = Reader::new(input);
    let mut output = Vec::new();
    while !reader.remaining.is_empty() {
        let expected = u32::from_be_bytes(reader.word()?);
        let length_bytes = reader.word()?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes))
            .map_err(|_| Error::Framing("LZO block size exceeds platform size"))?;
        if length >= 8779 {
            return Err(Error::Framing("LZO block exceeds upstream buffer size"));
        }
        let block = reader.take(length)?;
        let mut checksum = Adler32::from_checksum(0);
        checksum.write_slice(&length_bytes);
        checksum.write_slice(block);
        if checksum.checksum() != expected {
            return Err(Error::Framing("LZO block checksum mismatch"));
        }
        let mut buffer = [0; 8192];
        let produced = lzokay::decompress::decompress(block, &mut buffer)
            .map_err(|error| Error::Lzo(error.to_string()))?;
        append_bounded(
            &mut output,
            buffer.get(..produced).ok_or(Error::Truncated)?,
            limit,
        )?;
    }
    Ok(output)
}

fn append_bounded(output: &mut Vec<u8>, bytes: &[u8], limit: usize) -> Result<(), Error> {
    if bytes.len() > limit.saturating_sub(output.len()) {
        return Err(Error::SizeLimit(limit));
    }
    output.extend_from_slice(bytes);
    Ok(())
}

impl Compression {
    pub(crate) fn encode(self, input: &[u8], output: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::None => output.extend_from_slice(input),
            Self::Zlib => {
                let mut encoder =
                    flate2::write::ZlibEncoder::new(output, flate2::Compression::new(6));
                encoder.write_all(input)?;
                encoder.finish()?;
            }
            Self::Lzma => {
                let stream = Stream::new_easy_encoder(2, Check::Crc32)
                    .map_err(|error| Error::Compression(std::io::Error::other(error)))?;
                let mut encoder = XzEncoder::new_stream(output, stream);
                encoder.write_all(input)?;
                encoder.finish()?;
            }
            Self::Lzo => {
                for block in input.chunks(8192) {
                    let compressed = lzokay::compress::compress(block)
                        .map_err(|error| Error::Lzo(error.to_string()))?;
                    let length = u32::try_from(compressed.len())
                        .map_err(|_| Error::Framing("LZO compressed length exceeds 32 bits"))?;
                    let length_bytes = length.to_be_bytes();
                    let mut checksum = Adler32::from_checksum(0);
                    checksum.write_slice(&length_bytes);
                    checksum.write_slice(&compressed);
                    output.extend_from_slice(&checksum.checksum().to_be_bytes());
                    output.extend_from_slice(&length_bytes);
                    output.extend_from_slice(&compressed);
                }
            }
        }
        Ok(())
    }
}
