use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use bzip2::read::BzDecoder;
use flate2::read::ZlibDecoder;

use crate::errors::EtcherError;

const KOLY_MAGIC: &[u8; 4] = b"koly";
const MISH_MAGIC: &[u8; 4] = b"mish";
const SECTOR_SIZE: u64 = 512;
const MAX_PLIST_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkKind {
    ZeroFill,
    Raw,
    Zlib,
    Bzip2,
}

#[derive(Debug, Clone)]
struct DmgChunk {
    kind: ChunkKind,
    out_offset: u64,
    out_len: u64,
    in_offset: u64,
    in_len: u64,
}

pub fn is_udif_dmg(file: &mut File) -> bool {
    let Ok(len) = file.seek(SeekFrom::End(0)) else {
        return false;
    };
    if len < 512 {
        let _ = file.seek(SeekFrom::Start(0));
        return false;
    }
    if file.seek(SeekFrom::End(-512)).is_err() {
        let _ = file.seek(SeekFrom::Start(0));
        return false;
    }
    let mut magic = [0u8; 4];
    let ok = file.read_exact(&mut magic).is_ok() && &magic == KOLY_MAGIC;
    let _ = file.seek(SeekFrom::Start(0));
    ok
}

pub struct DmgReader<R: Read + Seek> {
    reader: R,
    chunks: Vec<DmgChunk>,
    total_size: u64,
    pos: u64,
    chunk_idx: usize,
    active_chunk_idx: Option<usize>,
    active_decoder: Option<Box<dyn Read + Send>>,
}

impl<R: Read + Seek> DmgReader<R> {
    pub fn open(mut reader: R) -> Result<Self, EtcherError> {
        let file_len = reader.seek(SeekFrom::End(0))?;
        if file_len < 512 {
            return Err(EtcherError::user_error(
                "Invalid DMG file",
                "DMG file is smaller than 512 bytes.",
            ));
        }

        reader.seek(SeekFrom::End(-512))?;
        let mut koly = [0u8; 512];
        reader.read_exact(&mut koly)?;
        if &koly[0..4] != KOLY_MAGIC {
            return Err(EtcherError::user_error(
                "Invalid DMG file",
                "Missing UDIF koly trailer.",
            ));
        }

        let data_fork_offset = be_u64(&koly[24..32]);
        let xml_offset = be_u64(&koly[216..224]);
        let xml_length = be_u64(&koly[224..232]);
        let sector_count = be_u64(&koly[492..500]);

        if xml_length == 0 || xml_length > MAX_PLIST_BYTES || xml_offset + xml_length > file_len {
            return Err(EtcherError::user_error(
                "Invalid DMG metadata",
                "UDIF XML plist offset or length is out of bounds.",
            ));
        }

        reader.seek(SeekFrom::Start(xml_offset))?;
        let mut xml_bytes = vec![0u8; xml_length as usize];
        reader.read_exact(&mut xml_bytes)?;
        let xml = String::from_utf8_lossy(&xml_bytes);

        let mut chunks = Vec::new();
        for mish_bytes in extract_blkx_mish_tables(&xml)? {
            parse_mish_table(&mish_bytes, data_fork_offset, &mut chunks)?;
        }

        chunks.sort_by_key(|c| c.out_offset);
        let max_chunk_end = chunks
            .iter()
            .map(|c| c.out_offset.saturating_add(c.out_len))
            .max()
            .unwrap_or(0);
        let total_size = sector_count.saturating_mul(SECTOR_SIZE).max(max_chunk_end);

        Ok(Self {
            reader,
            chunks,
            total_size,
            pos: 0,
            chunk_idx: 0,
            active_chunk_idx: None,
            active_decoder: None,
        })
    }

    pub fn uncompressed_size(&self) -> u64 {
        self.total_size
    }
}

impl<R: Read + Seek> Read for DmgReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() || self.pos >= self.total_size {
            return Ok(0);
        }

        while self.chunk_idx < self.chunks.len() {
            let c = &self.chunks[self.chunk_idx];
            if c.out_offset.saturating_add(c.out_len) <= self.pos {
                self.active_chunk_idx = None;
                self.active_decoder = None;
                self.chunk_idx += 1;
            } else {
                break;
            }
        }

        if self.chunk_idx >= self.chunks.len() {
            let rem = (self.total_size - self.pos).min(buf.len() as u64) as usize;
            buf[..rem].fill(0);
            self.pos += rem as u64;
            return Ok(rem);
        }

        let chunk = self.chunks[self.chunk_idx].clone();
        if self.pos < chunk.out_offset {
            let gap = (chunk.out_offset - self.pos).min(buf.len() as u64) as usize;
            buf[..gap].fill(0);
            self.pos += gap as u64;
            return Ok(gap);
        }

        let chunk_end = chunk.out_offset + chunk.out_len;
        let rem_in_chunk = (chunk_end - self.pos).min(buf.len() as u64) as usize;

        match chunk.kind {
            ChunkKind::ZeroFill => {
                buf[..rem_in_chunk].fill(0);
                self.pos += rem_in_chunk as u64;
                Ok(rem_in_chunk)
            }
            ChunkKind::Raw => {
                if self.active_chunk_idx != Some(self.chunk_idx) {
                    let off_in_chunk = self.pos - chunk.out_offset;
                    self.reader
                        .seek(SeekFrom::Start(chunk.in_offset + off_in_chunk))?;
                    self.active_chunk_idx = Some(self.chunk_idx);
                    self.active_decoder = None;
                }
                let n = self.reader.read(&mut buf[..rem_in_chunk])?;
                if n == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Unexpected EOF in raw DMG chunk",
                    ));
                }
                self.pos += n as u64;
                Ok(n)
            }
            ChunkKind::Zlib | ChunkKind::Bzip2 => {
                if self.active_chunk_idx != Some(self.chunk_idx) {
                    self.reader.seek(SeekFrom::Start(chunk.in_offset))?;
                    let mut compressed = vec![0u8; chunk.in_len as usize];
                    self.reader.read_exact(&mut compressed)?;
                    let cursor = std::io::Cursor::new(compressed);
                    let dec: Box<dyn Read + Send> = match chunk.kind {
                        ChunkKind::Zlib => Box::new(ZlibDecoder::new(cursor)),
                        ChunkKind::Bzip2 => Box::new(BzDecoder::new(cursor)),
                        _ => unreachable!(),
                    };
                    self.active_chunk_idx = Some(self.chunk_idx);
                    self.active_decoder = Some(dec);
                }
                let dec = self.active_decoder.as_mut().expect("active decoder");
                let n = dec.read(&mut buf[..rem_in_chunk])?;
                if n == 0 {
                    // Pad trailing bytes if compressed stream ended slightly early on sector boundary
                    buf[..rem_in_chunk].fill(0);
                    self.pos += rem_in_chunk as u64;
                    return Ok(rem_in_chunk);
                }
                self.pos += n as u64;
                Ok(n)
            }
        }
    }
}

fn be_u32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn be_u64(b: &[u8]) -> u64 {
    u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

fn parse_mish_table(
    mish: &[u8],
    data_fork_offset: u64,
    out: &mut Vec<DmgChunk>,
) -> Result<(), EtcherError> {
    if mish.len() < 204 || &mish[0..4] != MISH_MAGIC {
        return Err(EtcherError::user_error(
            "Invalid DMG block map",
            "Corrupt UDIF mish resource table.",
        ));
    }

    let base_sector = be_u64(&mish[8..16]);
    let mish_data_offset = be_u64(&mish[24..32]);
    let chunk_count = be_u32(&mish[200..204]) as usize;

    let entries_bytes = &mish[204..];
    let available = entries_bytes.len() / 40;
    let count = chunk_count.min(available);

    for i in 0..count {
        let e = &entries_bytes[i * 40..(i + 1) * 40];
        let entry_type = be_u32(&e[0..4]);
        let sector_number = be_u64(&e[8..16]);
        let sector_count = be_u64(&e[16..24]);
        let comp_offset = be_u64(&e[24..32]);
        let comp_length = be_u64(&e[32..40]);

        match entry_type {
            0x7FFF_FFFF => continue, // comment
            0xFFFF_FFFF => break,    // end
            0x0000_0000 | 0x0000_0002 => {
                if sector_count > 0 {
                    out.push(DmgChunk {
                        kind: ChunkKind::ZeroFill,
                        out_offset: base_sector.saturating_add(sector_number).saturating_mul(SECTOR_SIZE),
                        out_len: sector_count.saturating_mul(SECTOR_SIZE),
                        in_offset: 0,
                        in_len: 0,
                    });
                }
            }
            0x0000_0001 => {
                if sector_count > 0 {
                    out.push(DmgChunk {
                        kind: ChunkKind::Raw,
                        out_offset: base_sector.saturating_add(sector_number).saturating_mul(SECTOR_SIZE),
                        out_len: sector_count.saturating_mul(SECTOR_SIZE),
                        in_offset: data_fork_offset + mish_data_offset + comp_offset,
                        in_len: comp_length,
                    });
                }
            }
            0x8000_0005 => {
                if sector_count > 0 {
                    out.push(DmgChunk {
                        kind: ChunkKind::Zlib,
                        out_offset: base_sector.saturating_add(sector_number).saturating_mul(SECTOR_SIZE),
                        out_len: sector_count.saturating_mul(SECTOR_SIZE),
                        in_offset: data_fork_offset + mish_data_offset + comp_offset,
                        in_len: comp_length,
                    });
                }
            }
            0x8000_0006 => {
                if sector_count > 0 {
                    out.push(DmgChunk {
                        kind: ChunkKind::Bzip2,
                        out_offset: base_sector.saturating_add(sector_number).saturating_mul(SECTOR_SIZE),
                        out_len: sector_count.saturating_mul(SECTOR_SIZE),
                        in_offset: data_fork_offset + mish_data_offset + comp_offset,
                        in_len: comp_length,
                    });
                }
            }
            other => {
                return Err(EtcherError::user_error(
                    "Unsupported DMG compression",
                    format!(
                        "DMG uses unsupported UDIF chunk codec 0x{other:08x} (e.g. ADC/LZFSE/LZMA). Convert to raw .img/.iso first."
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn extract_blkx_mish_tables(xml: &str) -> Result<Vec<Vec<u8>>, EtcherError> {
    let Some(blkx_pos) = xml.find("<key>blkx</key>") else {
        return Err(EtcherError::user_error(
            "Invalid DMG metadata",
            "UDIF XML plist does not contain a blkx block map.",
        ));
    };
    let after_blkx = &xml[blkx_pos..];
    let Some(arr_start) = after_blkx.find("<array>") else {
        return Err(EtcherError::user_error(
            "Invalid DMG metadata",
            "Missing <array> in UDIF blkx section.",
        ));
    };
    let Some(arr_end) = after_blkx[arr_start..].find("</array>") else {
        return Err(EtcherError::user_error(
            "Invalid DMG metadata",
            "Unterminated <array> in UDIF blkx section.",
        ));
    };
    let arr_slice = &after_blkx[arr_start..arr_start + arr_end];

    let mut tables = Vec::new();
    let mut cursor = 0;
    while let Some(rel_start) = arr_slice[cursor..].find("<data>") {
        let data_start = cursor + rel_start + "<data>".len();
        let Some(rel_end) = arr_slice[data_start..].find("</data>") else {
            break;
        };
        let b64 = &arr_slice[data_start..data_start + rel_end];
        let decoded = decode_base64(b64)?;
        if decoded.len() >= 204 && &decoded[0..4] == MISH_MAGIC {
            tables.push(decoded);
        }
        cursor = data_start + rel_end + "</data>".len();
    }

    if tables.is_empty() {
        return Err(EtcherError::user_error(
            "Invalid DMG metadata",
            "No valid UDIF mish tables found in DMG.",
        ));
    }
    Ok(tables)
}

fn decode_base64(input: &str) -> Result<Vec<u8>, EtcherError> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u8 = 0;

    for b in input.bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'=' {
            break;
        }
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => {
                return Err(EtcherError::user_error(
                    "Invalid DMG metadata",
                    "Corrupt base64 data in UDIF plist.",
                ));
            }
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}
