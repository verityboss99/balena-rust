use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use url::Url;

use crate::errors::{ErrorCode, EtcherError};
use crate::partition::inspect_partition_table;
use crate::types::{Authentication, SourceMetadata, SourceType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionFormat {
    None,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
    Zip,
    Dmg,
}

impl CompressionFormat {
    pub fn archive_extension(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Gzip => Some("gz"),
            Self::Bzip2 => Some("bz2"),
            Self::Xz => Some("xz"),
            Self::Zstd => Some("zst"),
            Self::Zip => Some("zip"),
            Self::Dmg => Some("dmg"),
        }
    }

    pub fn is_compressed(self) -> bool {
        !matches!(self, Self::None)
    }
}

pub fn detect_format(magic: &[u8], path: &str) -> CompressionFormat {
    if magic.len() >= 2 && magic[0] == 0x1F && magic[1] == 0x8B {
        return CompressionFormat::Gzip;
    }
    if magic.len() >= 3 && &magic[..3] == b"BZh" {
        return CompressionFormat::Bzip2;
    }
    if magic.len() >= 6 && magic[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00] {
        return CompressionFormat::Xz;
    }
    if magic.len() >= 4 && magic[..4] == [0x28, 0xB5, 0x2F, 0xFD] {
        return CompressionFormat::Zstd;
    }
    if magic.len() >= 4 && magic[..4] == [0x50, 0x4B, 0x03, 0x04] {
        return CompressionFormat::Zip;
    }
    if path.to_ascii_lowercase().ends_with(".dmg") {
        return CompressionFormat::Dmg;
    }
    CompressionFormat::None
}

pub use crate::supported_formats::{is_http_url, normalize_image_url};

pub fn probe_source_metadata(
    selected: &str,
    source_type: SourceType,
    auth: Option<Authentication>,
) -> Result<SourceMetadata, EtcherError> {
    match source_type {
        SourceType::File => probe_file_metadata(selected),
        SourceType::Http => probe_http_metadata(selected, auth),
        SourceType::BlockDevice => probe_block_metadata(selected),
    }
}

fn probe_file_metadata(path_str: &str) -> Result<SourceMetadata, EtcherError> {
    let path = Path::new(path_str);
    let mut file = File::open(path).map_err(|e| EtcherError::from_io_error(&e, Some(path_str)))?;
    let file_len = file
        .metadata()
        .map_err(|e| EtcherError::from_io_error(&e, Some(path_str)))?
        .len();

    let mut magic = [0u8; 16];
    let n = file.read(&mut magic)?;
    let format = detect_format(&magic[..n], path_str);
    file.seek(SeekFrom::Start(0))?;

    let mut header = vec![0u8; 64 * 1024];
    let mut size: Option<u64> = None;
    let mut compressed_size: Option<u64> = None;
    let mut is_size_estimated = false;
    let mut inner_name: Option<String> = None;

    match format {
        CompressionFormat::None => {
            size = Some(file_len);
            let read_n = read_up_to(&mut file, &mut header)?;
            header.truncate(read_n);
        }
        CompressionFormat::Gzip => {
            compressed_size = Some(file_len);
            // Gzip ISIZE trailer (last 4 bytes, mod 2^32) gives an estimate/exact size for < 4 GiB
            if file_len >= 4 {
                file.seek(SeekFrom::End(-4))?;
                let mut tail = [0u8; 4];
                if file.read_exact(&mut tail).is_ok() {
                    let isize_val = u32::from_le_bytes(tail) as u64;
                    if isize_val >= file_len || file_len < 4 * 1024 * 1024 {
                        size = Some(isize_val);
                    }
                }
                file.seek(SeekFrom::Start(0))?;
            }
            is_size_estimated = true;
            let mut dec = flate2::read::MultiGzDecoder::new(BufReader::new(file));
            let read_n = read_up_to(&mut dec, &mut header)?;
            header.truncate(read_n);
        }
        CompressionFormat::Bzip2 => {
            compressed_size = Some(file_len);
            is_size_estimated = true;
            let mut dec = bzip2::read::BzDecoder::new(BufReader::new(file));
            let read_n = read_up_to(&mut dec, &mut header)?;
            header.truncate(read_n);
        }
        CompressionFormat::Xz => {
            compressed_size = Some(file_len);
            is_size_estimated = true;
            let mut dec = xz2::read::XzDecoder::new_multi_decoder(BufReader::new(file));
            let read_n = read_up_to(&mut dec, &mut header)?;
            header.truncate(read_n);
        }
        CompressionFormat::Zstd => {
            compressed_size = Some(file_len);
            is_size_estimated = true;
            let mut dec = zstd::stream::read::Decoder::new(BufReader::new(file))
                .map_err(|e| EtcherError::new("Invalid zstd archive", e.to_string()))?;
            let read_n = read_up_to(&mut dec, &mut header)?;
            header.truncate(read_n);
        }
        CompressionFormat::Zip => {
            compressed_size = Some(file_len);
            let mut archive = zip::ZipArchive::new(BufReader::new(file))
                .map_err(|e| EtcherError::new("Invalid ZIP archive", e.to_string()))?;
            let idx = select_primary_zip_entry(&mut archive)?;
            let mut entry = archive
                .by_index(idx)
                .map_err(|e| EtcherError::new("Failed reading ZIP entry", e.to_string()))?;
            size = Some(entry.size());
            inner_name = Some(
                Path::new(entry.name())
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(entry.name())
                    .to_string(),
            );
            let read_n = read_up_to(&mut entry, &mut header)?;
            header.truncate(read_n);
        }
        CompressionFormat::Dmg => {
            return Err(EtcherError::user_error(
                "DMG inspection not supported",
                "Please convert the DMG image to a raw .img or .iso before flashing.",
            ));
        }
    }

    let (has_mbr, partitions) = inspect_partition_table(&header);
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path_str)
        .to_string();
    let display_name = inner_name.clone().unwrap_or_else(|| file_name.clone());
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .map(ToString::to_string);

    Ok(SourceMetadata {
        path: path_str.to_string(),
        name: Some(inner_name.unwrap_or_else(|| file_name.clone())),
        display_name,
        description: file_name,
        size,
        compressed_size,
        is_size_estimated,
        recommended_drive_size: None,
        has_mbr,
        partitions,
        source_type: SourceType::File,
        extension,
        archive_extension: format.archive_extension().map(ToString::to_string),
        drive: None,
        auth: None,
        logo: None,
        support_url: None,
        is_compressed: format.is_compressed(),
        blockmapped_size: None,
    })
}

fn probe_block_metadata(path_str: &str) -> Result<SourceMetadata, EtcherError> {
    let mut file =
        File::open(path_str).map_err(|e| EtcherError::from_io_error(&e, Some(path_str)))?;
    let size = file.seek(SeekFrom::End(0)).ok();
    file.seek(SeekFrom::Start(0))?;
    let mut header = vec![0u8; 64 * 1024];
    let n = read_up_to(&mut file, &mut header)?;
    header.truncate(n);
    let (has_mbr, partitions) = inspect_partition_table(&header);
    let name = Path::new(path_str)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path_str)
        .to_string();
    Ok(SourceMetadata {
        path: path_str.to_string(),
        name: Some(name.clone()),
        display_name: name.clone(),
        description: name,
        size,
        compressed_size: None,
        is_size_estimated: false,
        recommended_drive_size: None,
        has_mbr,
        partitions,
        source_type: SourceType::BlockDevice,
        extension: None,
        archive_extension: None,
        drive: None,
        auth: None,
        logo: None,
        support_url: None,
        is_compressed: false,
        blockmapped_size: None,
    })
}

fn build_http_client() -> Result<reqwest::blocking::Client, EtcherError> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("balenaEtcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EtcherError::new("Failed to initialize HTTP client", e.to_string()))
}

fn probe_http_metadata(
    raw_url: &str,
    auth: Option<Authentication>,
) -> Result<SourceMetadata, EtcherError> {
    let normalized = normalize_image_url(raw_url);
    let parsed = Url::parse(&normalized)
        .map_err(|e| EtcherError::user_error("Invalid URL", e.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(EtcherError::user_error(
            "Unsupported protocol",
            "Only http:// and https:// URLs are supported.",
        ));
    }

    let client = build_http_client()?;
    let mut req = client.get(parsed.clone()).header("Range", "bytes=0-65535");
    if let Some(a) = &auth
        && !a.username.is_empty()
    {
        req = req.basic_auth(&a.username, Some(&a.password));
    }
    let mut resp = req
        .send()
        .map_err(|e| EtcherError::new("Failed to fetch image URL", e.to_string()))?;
    if !resp.status().is_success() {
        return Err(EtcherError::user_error(
            format!("HTTP error {}", resp.status().as_u16()),
            format!("Server returned status {} for {normalized}", resp.status()),
        ));
    }

    let total_len = parse_content_range_total(resp.headers().get("content-range"))
        .or_else(|| resp.content_length());
    let filename = parse_content_disposition_filename(resp.headers().get("content-disposition"))
        .or_else(|| {
            parsed
                .path_segments()
                .and_then(|mut s| s.next_back().map(ToString::to_string))
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "image.img".to_string());

    let mut buf = vec![0u8; 64 * 1024];
    let n = read_up_to(&mut resp, &mut buf)?;
    buf.truncate(n);

    let format = detect_format(&buf, &filename);
    let mut header = buf.clone();
    let mut is_size_estimated = false;
    let (size, compressed_size) = if format.is_compressed() {
        is_size_estimated = true;
        // Peek decompressed header if possible
        if let Ok(mut dec) = open_decompressor_for_slice(format, &buf) {
            let mut dec_hdr = vec![0u8; 64 * 1024];
            if let Ok(dn) = read_up_to(&mut dec, &mut dec_hdr) {
                dec_hdr.truncate(dn);
                header = dec_hdr;
            }
        }
        (None, total_len)
    } else {
        (total_len, None)
    };

    let (has_mbr, partitions) = inspect_partition_table(&header);
    let extension = Path::new(&filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(ToString::to_string);

    Ok(SourceMetadata {
        path: normalized,
        name: Some(filename.clone()),
        display_name: filename.clone(),
        description: filename,
        size,
        compressed_size,
        is_size_estimated,
        recommended_drive_size: None,
        has_mbr,
        partitions,
        source_type: SourceType::Http,
        extension,
        archive_extension: format.archive_extension().map(ToString::to_string),
        drive: None,
        auth,
        logo: None,
        support_url: None,
        is_compressed: format.is_compressed(),
        blockmapped_size: None,
    })
}

fn open_decompressor_for_slice<'a>(
    format: CompressionFormat,
    data: &'a [u8],
) -> Result<Box<dyn Read + 'a>, EtcherError> {
    let cursor = std::io::Cursor::new(data);
    match format {
        CompressionFormat::Gzip => Ok(Box::new(flate2::read::MultiGzDecoder::new(cursor))),
        CompressionFormat::Bzip2 => Ok(Box::new(bzip2::read::BzDecoder::new(cursor))),
        CompressionFormat::Xz => Ok(Box::new(xz2::read::XzDecoder::new_multi_decoder(cursor))),
        CompressionFormat::Zstd => Ok(Box::new(
            zstd::stream::read::Decoder::new(cursor)
                .map_err(|e| EtcherError::new("Zstd error", e.to_string()))?,
        )),
        _ => Ok(Box::new(cursor)),
    }
}

fn parse_content_range_total(val: Option<&reqwest::header::HeaderValue>) -> Option<u64> {
    let s = val?.to_str().ok()?;
    let total = s.rsplit('/').next()?;
    total.parse::<u64>().ok()
}

fn parse_content_disposition_filename(val: Option<&reqwest::header::HeaderValue>) -> Option<String> {
    let s = val?.to_str().ok()?;
    for part in s.split(';') {
        let trimmed = part.trim();
        if let Some(rest) = trimmed.strip_prefix("filename=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

pub enum OpenedSource {
    /// Raw seekable source (raw file or block device)
    RawSeekable {
        file: File,
        size: Option<u64>,
    },
    /// Streaming source (compressed file or HTTP stream)
    Stream {
        reader: Box<dyn Read + Send>,
        compressed_bytes_read: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
        compressed_total: Option<u64>,
        uncompressed_size: Option<u64>,
    },
}

struct CountingReader<R> {
    inner: R,
    counter: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.counter
            .fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
        Ok(n)
    }
}

pub fn open_source_for_write(image: &SourceMetadata) -> Result<OpenedSource, EtcherError> {
    match image.source_type {
        SourceType::BlockDevice => {
            let dev_path = image
                .drive
                .as_ref()
                .map(|d| d.raw.as_str())
                .unwrap_or(&image.path);
            let mut file = File::open(dev_path)
                .map_err(|e| EtcherError::from_io_error(&e, Some(dev_path)))?;
            let size = image.size.or_else(|| file.seek(SeekFrom::End(0)).ok());
            file.seek(SeekFrom::Start(0))?;
            Ok(OpenedSource::RawSeekable { file, size })
        }
        SourceType::File => {
            let mut file = File::open(&image.path)
                .map_err(|e| EtcherError::from_io_error(&e, Some(&image.path)))?;
            let file_len = file.metadata()?.len();
            let mut magic = [0u8; 16];
            let n = file.read(&mut magic)?;
            let format = detect_format(&magic[..n], &image.path);
            file.seek(SeekFrom::Start(0))?;

            if format == CompressionFormat::None {
                return Ok(OpenedSource::RawSeekable {
                    file,
                    size: Some(file_len),
                });
            }

            if format == CompressionFormat::Zip {
                let archive = zip::ZipArchive::new(BufReader::new(file))
                    .map_err(|e| EtcherError::new("Invalid ZIP archive", e.to_string()))?;
                let zip_reader = OwnedZipEntryReader::new(archive)?;
                let uncompressed_size = Some(zip_reader.entry_size);
                return Ok(OpenedSource::Stream {
                    reader: Box::new(zip_reader),
                    compressed_bytes_read: None,
                    compressed_total: Some(file_len),
                    uncompressed_size,
                });
            }

            let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
            let counting = CountingReader {
                inner: BufReader::new(file),
                counter: counter.clone(),
            };
            let reader: Box<dyn Read + Send> = match format {
                CompressionFormat::Gzip => Box::new(flate2::read::MultiGzDecoder::new(counting)),
                CompressionFormat::Bzip2 => Box::new(bzip2::read::BzDecoder::new(counting)),
                CompressionFormat::Xz => {
                    Box::new(xz2::read::XzDecoder::new_multi_decoder(counting))
                }
                CompressionFormat::Zstd => Box::new(
                    zstd::stream::read::Decoder::new(counting)
                        .map_err(|e| EtcherError::new("Invalid zstd stream", e.to_string()))?,
                ),
                CompressionFormat::Dmg => {
                    return Err(EtcherError::user_error(
                        "DMG format unsupported",
                        "Please extract the DMG to .img/.iso before flashing.",
                    ));
                }
                _ => unreachable!(),
            };

            Ok(OpenedSource::Stream {
                reader,
                compressed_bytes_read: Some(counter),
                compressed_total: Some(file_len),
                uncompressed_size: image.size,
            })
        }
        SourceType::Http => {
            let url = normalize_image_url(&image.path);
            let client = build_http_client()?;
            let mut req = client.get(&url);
            if let Some(a) = &image.auth
                && !a.username.is_empty()
            {
                req = req.basic_auth(&a.username, Some(&a.password));
            }
            let mut resp = req
                .send()
                .map_err(|e| EtcherError::new("Failed downloading image", e.to_string()))?;
            if !resp.status().is_success() {
                return Err(EtcherError::new(
                    format!("HTTP error {}", resp.status().as_u16()),
                    format!("Server returned {}", resp.status()),
                ));
            }
            let total = resp.content_length().or(image.compressed_size).or(image.size);
            let mut magic = [0u8; 16];
            let n = read_up_to(&mut resp, &mut magic)?;
            let format = detect_format(&magic[..n], &image.display_name);
            let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(n as u64));
            let mut chained = std::io::Cursor::new(magic[..n].to_vec()).chain(CountingReader {
                inner: resp,
                counter: counter.clone(),
            });

            if format == CompressionFormat::Zip {
                // Zip requires seeking; spool to temp file first
                let mut tmp = tempfile::NamedTempFile::new()
                    .map_err(|e| EtcherError::from_io_error(&e, None))?;
                std::io::copy(&mut chained, &mut tmp)?;
                tmp.seek(SeekFrom::Start(0))?;
                let archive = zip::ZipArchive::new(BufReader::new(tmp.reopen()?))
                    .map_err(|e| EtcherError::new("Invalid ZIP archive", e.to_string()))?;
                let zr = OwnedZipEntryReader::new(archive)?;
                let sz = zr.entry_size;
                return Ok(OpenedSource::Stream {
                    reader: Box::new(zr),
                    compressed_bytes_read: None,
                    compressed_total: total,
                    uncompressed_size: Some(sz),
                });
            }

            let reader: Box<dyn Read + Send> = match format {
                CompressionFormat::None => Box::new(chained),
                CompressionFormat::Gzip => Box::new(flate2::read::MultiGzDecoder::new(chained)),
                CompressionFormat::Bzip2 => Box::new(bzip2::read::BzDecoder::new(chained)),
                CompressionFormat::Xz => {
                    Box::new(xz2::read::XzDecoder::new_multi_decoder(chained))
                }
                CompressionFormat::Zstd => Box::new(
                    zstd::stream::read::Decoder::new(chained)
                        .map_err(|e| EtcherError::new("Invalid zstd stream", e.to_string()))?,
                ),
                CompressionFormat::Dmg => {
                    return Err(EtcherError::user_error(
                        "DMG format unsupported",
                        "DMG over HTTP cannot be flashed directly.",
                    ));
                }
                CompressionFormat::Zip => unreachable!(),
            };

            Ok(OpenedSource::Stream {
                reader,
                compressed_bytes_read: Some(counter),
                compressed_total: total,
                uncompressed_size: if format.is_compressed() { None } else { total },
            })
        }
    }
}

fn select_primary_zip_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<usize, EtcherError> {
    let mut best: Option<(usize, u64, bool)> = None;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| EtcherError::new("ZIP entry error", e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name();
        if name.starts_with("__MACOSX") || name.split('/').any(|p| p.starts_with('.')) {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let is_img = lower.ends_with(".img")
            || lower.ends_with(".iso")
            || lower.ends_with(".raw")
            || lower.ends_with(".bin")
            || lower.ends_with(".wic")
            || lower.ends_with(".sdcard");
        let sz = entry.size();
        match best {
            None => best = Some((i, sz, is_img)),
            Some((_, best_sz, best_img)) => {
                if (is_img && !best_img) || (is_img == best_img && sz > best_sz) {
                    best = Some((i, sz, is_img));
                }
            }
        }
    }
    best.map(|(i, _, _)| i)
        .ok_or_else(|| EtcherError::user_error("Empty ZIP archive", "No image file found in ZIP."))
}

/// Self-contained reader for a single entry inside a ZipArchive without self-referential borrows.
struct OwnedZipEntryReader {
    entry_size: u64,
    rx: std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>,
    current_chunk: Vec<u8>,
    chunk_pos: usize,
}

impl OwnedZipEntryReader {
    fn new<R: Read + Seek + Send + 'static>(mut archive: zip::ZipArchive<R>) -> Result<Self, EtcherError> {
        let entry_index = select_primary_zip_entry(&mut archive)?;
        let entry_size = archive
            .by_index(entry_index)
            .map_err(|e| EtcherError::new("ZIP error", e.to_string()))?
            .size();
        let (tx, rx) = std::sync::mpsc::sync_channel::<std::io::Result<Vec<u8>>>(8);
        std::thread::spawn(move || {
            let mut archive = archive;
            let Ok(mut entry) = archive.by_index(entry_index) else {
                return;
            };
            let mut buf = vec![0u8; 256 * 1024];
            loop {
                match entry.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(Ok(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            entry_size,
            rx,
            current_chunk: Vec::new(),
            chunk_pos: 0,
        })
    }
}

impl Read for OwnedZipEntryReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.chunk_pos >= self.current_chunk.len() {
            match self.rx.recv() {
                Ok(Ok(chunk)) => {
                    self.current_chunk = chunk;
                    self.chunk_pos = 0;
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => return Ok(0),
            }
        }
        let avail = &self.current_chunk[self.chunk_pos..];
        let to_copy = avail.len().min(out.len());
        out[..to_copy].copy_from_slice(&avail[..to_copy]);
        self.chunk_pos += to_copy;
        Ok(to_copy)
    }
}
fn read_up_to<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                if total > 0 {
                    break;
                }
                return Err(e);
            }
        }
    }
    Ok(total)
}

pub fn ensure_valid_error_code(code: ErrorCode, msg: &str) -> EtcherError {
    EtcherError::user_error("Source error", msg).with_code(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    fn sample_mbr_image(size: usize) -> Vec<u8> {
        let mut data = vec![0xABu8; size];
        data[510] = 0x55;
        data[511] = 0xAA;
        data[446 + 4] = 0x0C;
        data[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());
        data[446 + 12..446 + 16].copy_from_slice(&4096u32.to_le_bytes());
        data
    }

    #[test]
    fn test_probe_and_read_all_formats() {
        let dir = tempfile::tempdir().unwrap();
        let raw_data = sample_mbr_image(8192);

        // Raw
        let raw_path = dir.path().join("test.img");
        std::fs::write(&raw_path, &raw_data).unwrap();
        let meta = probe_source_metadata(raw_path.to_str().unwrap(), SourceType::File, None).unwrap();
        assert!(meta.has_mbr);
        assert_eq!(meta.size, Some(8192));
        assert!(!meta.is_compressed);

        // Gzip
        let gz_path = dir.path().join("test.img.gz");
        {
            let mut enc = flate2::write::GzEncoder::new(
                File::create(&gz_path).unwrap(),
                flate2::Compression::default(),
            );
            enc.write_all(&raw_data).unwrap();
            enc.finish().unwrap();
        }
        let gz_meta = probe_source_metadata(gz_path.to_str().unwrap(), SourceType::File, None).unwrap();
        assert!(gz_meta.has_mbr);
        assert!(gz_meta.is_compressed);
        if let OpenedSource::Stream { mut reader, .. } = open_source_for_write(&gz_meta).unwrap() {
            let mut out = Vec::new();
            reader.read_to_end(&mut out).unwrap();
            assert_eq!(out, raw_data);
        } else {
            panic!("expected stream");
        }

        // Bzip2
        let bz_path = dir.path().join("test.img.bz2");
        {
            let mut enc = bzip2::write::BzEncoder::new(
                File::create(&bz_path).unwrap(),
                bzip2::Compression::default(),
            );
            enc.write_all(&raw_data).unwrap();
            enc.finish().unwrap();
        }
        let bz_meta = probe_source_metadata(bz_path.to_str().unwrap(), SourceType::File, None).unwrap();
        assert!(bz_meta.has_mbr);

        // Xz
        let xz_path = dir.path().join("test.img.xz");
        {
            let mut enc = xz2::write::XzEncoder::new(File::create(&xz_path).unwrap(), 1);
            enc.write_all(&raw_data).unwrap();
            enc.finish().unwrap();
        }
        let xz_meta = probe_source_metadata(xz_path.to_str().unwrap(), SourceType::File, None).unwrap();
        assert!(xz_meta.has_mbr);

        // Zstd
        let zst_path = dir.path().join("test.img.zst");
        {
            let mut enc = zstd::stream::write::Encoder::new(File::create(&zst_path).unwrap(), 1).unwrap();
            enc.write_all(&raw_data).unwrap();
            enc.finish().unwrap();
        }
        let zst_meta = probe_source_metadata(zst_path.to_str().unwrap(), SourceType::File, None).unwrap();
        assert!(zst_meta.has_mbr);

        // Zip (with __MACOSX junk ignored)
        let zip_path = dir.path().join("test.zip");
        {
            let mut zw = zip::ZipWriter::new(File::create(&zip_path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("__MACOSX/._test.img", opts).unwrap();
            zw.write_all(b"junk").unwrap();
            zw.start_file("os/test.img", opts).unwrap();
            zw.write_all(&raw_data).unwrap();
            zw.finish().unwrap();
        }
        let zip_meta = probe_source_metadata(zip_path.to_str().unwrap(), SourceType::File, None).unwrap();
        assert!(zip_meta.has_mbr);
        assert_eq!(zip_meta.size, Some(8192));
        if let OpenedSource::Stream { mut reader, .. } = open_source_for_write(&zip_meta).unwrap() {
            let mut out = Vec::new();
            reader.read_to_end(&mut out).unwrap();
            assert_eq!(out, raw_data);
        } else {
            panic!("expected stream");
        }
    }

    #[test]
    fn test_http_probe_with_basic_auth() {
        let raw_data = sample_mbr_image(4096);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let data_clone = raw_data.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                let mut req_buf = [0u8; 2048];
                let n = stream.read(&mut req_buf).unwrap();
                let req_str = String::from_utf8_lossy(&req_buf[..n]).to_ascii_lowercase();
                assert!(
                    req_str.contains("authorization: basic"),
                    "missing auth header in:\n{req_str}"
                );
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Disposition: attachment; filename=\"net.img\"\r\nConnection: close\r\n\r\n",
                    data_clone.len()
                );
                stream.write_all(resp.as_bytes()).unwrap();
                stream.write_all(&data_clone).unwrap();
            }
        });

        let url = format!("http://127.0.0.1:{}/download", addr.port());
        let auth = Some(Authentication {
            username: "user".to_string(),
            password: "secret".to_string(),
        });
        let meta = probe_source_metadata(&url, SourceType::Http, auth).unwrap();
        assert!(meta.has_mbr);
        assert_eq!(meta.display_name, "net.img");
        assert_eq!(meta.size, Some(4096));
    }

    #[test]
    fn test_http_zip_stream() {
        let dir = tempfile::tempdir().unwrap();
        let raw_data = sample_mbr_image(8192);
        let zip_path = dir.path().join("net.zip");
        {
            let mut zw = zip::ZipWriter::new(File::create(&zip_path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("os/net.img", opts).unwrap();
            zw.write_all(&raw_data).unwrap();
            zw.finish().unwrap();
        }
        let zip_bytes = std::fs::read(&zip_path).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let zip_clone = zip_bytes.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                let mut req_buf = [0u8; 2048];
                let _ = stream.read(&mut req_buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Disposition: attachment; filename=\"net.zip\"\r\nConnection: close\r\n\r\n",
                    zip_clone.len()
                );
                stream.write_all(resp.as_bytes()).unwrap();
                stream.write_all(&zip_clone).unwrap();
            }
        });

        let url = format!("http://127.0.0.1:{}/net.zip", addr.port());
        let meta = SourceMetadata {
            path: url,
            name: Some("net.zip".to_string()),
            display_name: "net.zip".to_string(),
            description: "net.zip".to_string(),
            size: None,
            compressed_size: Some(zip_bytes.len() as u64),
            is_size_estimated: true,
            recommended_drive_size: None,
            has_mbr: true,
            partitions: Vec::new(),
            source_type: SourceType::Http,
            extension: Some("zip".to_string()),
            archive_extension: Some("zip".to_string()),
            drive: None,
            auth: None,
            logo: None,
            support_url: None,
            is_compressed: true,
            blockmapped_size: None,
        };

        if let OpenedSource::Stream { mut reader, .. } = open_source_for_write(&meta).unwrap() {
            let mut out = Vec::new();
            reader.read_to_end(&mut out).unwrap();
            assert_eq!(out, raw_data);
        } else {
            panic!("expected stream");
        }
    }
}
