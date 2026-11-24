use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};
use uuid::Uuid;
use xxhash_rust::xxh3::Xxh3;

use crate::drives::unmount_drive_partitions;
use crate::errors::{ErrorCode, EtcherError};
use crate::partition::{Extent, compute_sparse_extents, inspect_partition_table};
use crate::source::{OpenedSource, open_source_for_write};
use crate::types::{Drive, FlashError, FlashPhase, FlashResults, ProgressState, WriteOptions};

pub const DECOMPRESSED_IMAGE_PREFIX: &str = "etcher-decompressed-";
const CHUNK_SIZE: usize = 1024 * 1024; // 1 MiB
const PROGRESS_THROTTLE: Duration = Duration::from_millis(100);

pub fn cleanup_tmp_files(older_than: Option<Duration>) {
    let tmp_dir = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&tmp_dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(DECOMPRESSED_IMAGE_PREFIX) {
            continue;
        }
        let should_remove = match older_than {
            None => true,
            Some(max_age) => entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| now.duration_since(t).ok())
                .map(|age| age >= max_age)
                .unwrap_or(false),
        };
        if should_remove {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

struct TargetHandle {
    drive: Drive,
    file: File,
    is_block: bool,
    failed: bool,
}

pub fn write_and_validate<FP, FF, FL>(
    options: &WriteOptions,
    cancel: Arc<AtomicBool>,
    skip_verify: Arc<AtomicBool>,
    mut on_progress: FP,
    mut on_fail: FF,
    mut on_log: FL,
) -> Result<FlashResults, EtcherError>
where
    FP: FnMut(ProgressState),
    FF: FnMut(FlashError),
    FL: FnMut(&str),
{
    cleanup_tmp_files(Some(Duration::from_secs(3600)));

    on_log(&format!("Image: {}", options.image.path));
    let dev_list: Vec<&str> = options
        .destinations
        .iter()
        .map(|d| d.device.as_str())
        .collect();
    on_log(&format!("Devices: {}", dev_list.join(", ")));
    on_log(&format!("Auto blockmapping: {}", options.auto_blockmapping));
    on_log(&format!("Decompress first: {}", options.decompress_first));

    on_progress(ProgressState {
        phase: FlashPhase::Starting,
        active: options.destinations.len(),
        ..Default::default()
    });

    if cancel.load(Ordering::Relaxed) {
        return Ok(FlashResults {
            cancelled: true,
            ..Default::default()
        });
    }

    let opened = open_source_for_write(&options.image)?;
    let mut temp_file_path: Option<PathBuf> = None;

    let (mut raw_file, source_size, stream_opt): (Option<File>, Option<u64>, Option<OpenedSource>) =
        match opened {
            OpenedSource::RawSeekable { file, size } => (Some(file), size, None),
            OpenedSource::Stream {
                mut reader,
                compressed_bytes_read,
                compressed_total,
                uncompressed_size,
            } => {
                if options.decompress_first {
                    let tmp_path = std::env::temp_dir().join(format!(
                        "{DECOMPRESSED_IMAGE_PREFIX}{}.img",
                        Uuid::new_v4()
                    ));
                    temp_file_path = Some(tmp_path.clone());
                    let mut tmp_out = OpenOptions::new()
                        .create(true)
                        .truncate(true)
                        .read(true)
                        .write(true)
                        .open(&tmp_path)
                        .map_err(|e| EtcherError::from_io_error(&e, tmp_path.to_str()))?;

                    let start = Instant::now();
                    let mut last_emit = Instant::now() - PROGRESS_THROTTLE;
                    let mut buf = vec![0u8; CHUNK_SIZE];
                    let mut total_decompressed: u64 = 0;

                    loop {
                        if cancel.load(Ordering::Relaxed) {
                            drop(tmp_out);
                            let _ = std::fs::remove_file(&tmp_path);
                            return Ok(FlashResults {
                                cancelled: true,
                                ..Default::default()
                            });
                        }
                        let n = reader.read(&mut buf)?;
                        if n == 0 {
                            break;
                        }
                        tmp_out.write_all(&buf[..n])?;
                        total_decompressed += n as u64;

                        if last_emit.elapsed() >= PROGRESS_THROTTLE {
                            last_emit = Instant::now();
                            let elapsed = start.elapsed().as_secs_f64().max(0.001);
                            let comp_read = compressed_bytes_read
                                .as_ref()
                                .map(|c| c.load(Ordering::Relaxed))
                                .unwrap_or(total_decompressed);
                            let (pct, eta) = if let Some(ct) = compressed_total
                                && ct > 0
                                && compressed_bytes_read.is_some()
                            {
                                let frac = (comp_read as f64 / ct as f64).clamp(0.0, 1.0);
                                let speed_c = comp_read as f64 / elapsed;
                                let rem = if speed_c > 0.0 {
                                    Some((ct.saturating_sub(comp_read)) as f64 / speed_c)
                                } else {
                                    None
                                };
                                (Some((frac * 100.0).floor() as u8), rem)
                            } else if let Some(us) = uncompressed_size
                                && us > 0
                            {
                                let frac = (total_decompressed as f64 / us as f64).clamp(0.0, 1.0);
                                let speed_u = total_decompressed as f64 / elapsed;
                                let rem = if speed_u > 0.0 {
                                    Some((us.saturating_sub(total_decompressed)) as f64 / speed_u)
                                } else {
                                    None
                                };
                                (Some((frac * 100.0).floor() as u8), rem)
                            } else {
                                (None, None)
                            };
                            let speed = total_decompressed as f64 / elapsed;
                            on_progress(ProgressState {
                                phase: FlashPhase::Decompressing,
                                percentage: pct,
                                position: total_decompressed,
                                total: uncompressed_size.unwrap_or(total_decompressed),
                                bytes_written: 0,
                                speed,
                                average_speed: speed,
                                eta,
                                active: options.destinations.len(),
                                failed: 0,
                            });
                        }
                    }
                    tmp_out.flush()?;
                    tmp_out.seek(SeekFrom::Start(0))?;
                    (Some(tmp_out), Some(total_decompressed), None)
                } else {
                    (
                        None,
                        uncompressed_size,
                        Some(OpenedSource::Stream {
                            reader,
                            compressed_bytes_read,
                            compressed_total,
                            uncompressed_size,
                        }),
                    )
                }
            }
        };

    // Compute sparse extents if seekable raw source & auto_blockmapping enabled
    let extents: Option<Vec<Extent>> = if options.auto_blockmapping
        && let (Some(rf), Some(sz)) = (&mut raw_file, source_size)
    {
        rf.seek(SeekFrom::Start(0))?;
        let mut hdr = vec![0u8; (64 * 1024).min(sz as usize)];
        let n = read_exact_or_eof(rf, &mut hdr)?;
        let (has_tbl, parts) = inspect_partition_table(&hdr[..n]);
        rf.seek(SeekFrom::Start(0))?;
        if has_tbl {
            compute_sparse_extents(rf, sz, &parts)
        } else {
            None
        }
    } else {
        None
    };

    let blockmapped_size = extents
        .as_ref()
        .map(|exs| exs.iter().map(|e| e.length).sum::<u64>());
    let total_to_write = blockmapped_size.or(source_size);

    // Unmount and open all destinations
    let mut handles: Vec<TargetHandle> = Vec::new();
    let mut errors: Vec<FlashError> = Vec::new();

    for drive in &options.destinations {
        if is_block_device_path(&drive.raw)
            && let Err(err) = unmount_drive_partitions(&drive.device)
        {
            let fe = FlashError {
                description: drive.description.clone(),
                device: drive.device.clone(),
                device_path: drive.device_path.clone(),
                code: err.code.clone(),
                message: err.to_string(),
            };
            on_fail(fe.clone());
            errors.push(fe);
            continue;
        }
        match open_target_file(&drive.raw) {
            Ok((file, is_block)) => handles.push(TargetHandle {
                drive: drive.clone(),
                file,
                is_block,
                failed: false,
            }),
            Err(e) => {
                let ee = EtcherError::from_io_error(&e, Some(&drive.raw));
                let fe = FlashError {
                    description: drive.description.clone(),
                    device: drive.device.clone(),
                    device_path: drive.device_path.clone(),
                    code: ee.code.clone(),
                    message: ee.to_string(),
                };
                on_fail(fe.clone());
                errors.push(fe);
            }
        }
    }

    if handles.is_empty() {
        cleanup_temp_path(temp_file_path.as_deref());
        return Ok(FlashResults {
            bytes_written: 0,
            source_size: source_size.unwrap_or(0),
            blockmapped_size,
            average_flashing_speed: 0.0,
            successful_devices: 0,
            failed_devices: errors.len(),
            errors,
            cancelled: false,
            skip: false,
            source_checksum: None,
        });
    }

    // Flashing stage
    let flash_start = Instant::now();
    let mut last_emit = Instant::now() - PROGRESS_THROTTLE;
    let mut bytes_written_per_drive: u64 = 0;
    let mut source_hasher = Xxh3::new();
    let mut expected_extent_hashes: Vec<(Extent, u64)> = Vec::new();

    if let Some(ref mut rf) = raw_file {
        let write_extents = extents.clone().unwrap_or_else(|| {
            vec![Extent {
                offset: 0,
                length: source_size.unwrap_or(0),
            }]
        });
        let mut buf = vec![0u8; CHUNK_SIZE];

        for ext in write_extents {
            rf.seek(SeekFrom::Start(ext.offset))?;
            let mut ext_hasher = Xxh3::new();
            let mut rem = ext.length;
            let mut cur_offset = ext.offset;

            while rem > 0 {
                if cancel.load(Ordering::Relaxed) {
                    cleanup_temp_path(temp_file_path.as_deref());
                    return Ok(FlashResults {
                        cancelled: true,
                        ..Default::default()
                    });
                }
                let to_read = (rem as usize).min(CHUNK_SIZE);
                let n = read_exact_or_eof(rf, &mut buf[..to_read])?;
                if n == 0 {
                    break;
                }
                let slice = &buf[..n];
                source_hasher.update(slice);
                ext_hasher.update(slice);

                write_chunk_to_targets(&mut handles, Some(cur_offset), slice, &mut errors, &mut on_fail);
                cur_offset += n as u64;
                rem -= n as u64;
                bytes_written_per_drive += n as u64;

                let active = handles.iter().filter(|h| !h.failed).count();
                if active == 0 {
                    break;
                }

                if last_emit.elapsed() >= PROGRESS_THROTTLE {
                    last_emit = Instant::now();
                    emit_progress(
                        &mut on_progress,
                        FlashPhase::Flashing,
                        bytes_written_per_drive,
                        total_to_write,
                        flash_start,
                        active,
                        errors.len(),
                    );
                }
            }
            expected_extent_hashes.push((ext, ext_hasher.digest()));
            if handles.iter().all(|h| h.failed) {
                break;
            }
        }
    } else if let Some(OpenedSource::Stream {
        mut reader,
        uncompressed_size,
        ..
    }) = stream_opt
    {
        let mut buf = vec![0u8; CHUNK_SIZE];
        loop {
            if cancel.load(Ordering::Relaxed) {
                cleanup_temp_path(temp_file_path.as_deref());
                return Ok(FlashResults {
                    cancelled: true,
                    ..Default::default()
                });
            }
            let n = read_exact_or_eof(&mut reader, &mut buf)?;
            if n == 0 {
                break;
            }
            let slice = &buf[..n];
            source_hasher.update(slice);
            write_chunk_to_targets(&mut handles, None, slice, &mut errors, &mut on_fail);
            bytes_written_per_drive += n as u64;

            let active = handles.iter().filter(|h| !h.failed).count();
            if active == 0 {
                break;
            }
            if last_emit.elapsed() >= PROGRESS_THROTTLE {
                last_emit = Instant::now();
                emit_progress(
                    &mut on_progress,
                    FlashPhase::Flashing,
                    bytes_written_per_drive,
                    uncompressed_size,
                    flash_start,
                    active,
                    errors.len(),
                );
            }
        }
        expected_extent_hashes.push((
            Extent {
                offset: 0,
                length: bytes_written_per_drive,
            },
            source_hasher.digest(),
        ));
    }

    let flash_elapsed = flash_start.elapsed().as_secs_f64().max(0.001);
    let average_flashing_speed = bytes_written_per_drive as f64 / flash_elapsed;
    let final_source_size = source_size.unwrap_or(bytes_written_per_drive);
    let source_checksum = format!("{:016x}", source_hasher.digest());

    // Emit 100% flashing -> Finishing before sync/verify
    let active_after_flash = handles.iter().filter(|h| !h.failed).count();
    emit_progress(
        &mut on_progress,
        FlashPhase::Flashing,
        bytes_written_per_drive,
        Some(bytes_written_per_drive),
        flash_start,
        active_after_flash,
        errors.len(),
    );

    // Sync & flush buffers on surviving targets
    for h in handles.iter_mut().filter(|h| !h.failed) {
        if let Err(e) = h.file.sync_all() {
            mark_target_failed(h, e, &mut errors, &mut on_fail);
            continue;
        }
        if h.is_block {
            flush_block_device(&h.file);
        }
    }

    let mut skipped_verify = false;
    if options.verify && handles.iter().any(|h| !h.failed) {
        let verify_start = Instant::now();
        let mut last_verify_emit = Instant::now() - PROGRESS_THROTTLE;
        let total_verify_bytes = bytes_written_per_drive;
        let surviving_indices: Vec<usize> = handles
            .iter()
            .enumerate()
            .filter(|(_, h)| !h.failed)
            .map(|(i, _)| i)
            .collect();

        // Verify each surviving drive concurrently
        type VerifyOutcome = Result<(), (Option<String>, String)>;
        let mut verify_results: Vec<(usize, VerifyOutcome)> = Vec::new();
        let verified_bytes_atomic = std::sync::atomic::AtomicU64::new(0);
        let cancel_ref = &cancel;
        let skip_ref = &skip_verify;

        // Share handle files across threads via mutable references split from handles
        let mut active_refs: Vec<(usize, &mut TargetHandle)> = handles
            .iter_mut()
            .enumerate()
            .filter(|(idx, _)| surviving_indices.contains(idx))
            .collect();

        std::thread::scope(|scope| {
            let mut join_handles = Vec::new();
            let num_targets = active_refs.len().max(1) as u64;
            for (idx, handle) in active_refs.iter_mut() {
                let idx = *idx;
                let file = &mut handle.file;
                let is_block = handle.is_block;
                let hashes_ref = &expected_extent_hashes;
                let v_bytes = &verified_bytes_atomic;
                join_handles.push(scope.spawn(move || -> (usize, Result<(), (Option<String>, String)>) {
                    let mut buf = vec![0u8; CHUNK_SIZE];
                    for (ext, expected_digest) in hashes_ref {
                        if let Err(e) = file.seek(SeekFrom::Start(ext.offset)) {
                            let ee = EtcherError::from_io_error(&e, None);
                            let msg = ee.to_string();
                            return (idx, Err((ee.0.code, msg)));
                        }
                        let mut h = Xxh3::new();
                        let mut rem = ext.length;
                        while rem > 0 {
                            if cancel_ref.load(Ordering::Relaxed) || skip_ref.load(Ordering::Relaxed) {
                                return (idx, Ok(()));
                            }
                            let to_read = (rem as usize).min(CHUNK_SIZE);
                            #[cfg(windows)]
                            let read_len = if is_block && !to_read.is_multiple_of(4096) {
                                to_read.div_ceil(4096).saturating_mul(4096).min(buf.len())
                            } else {
                                to_read
                            };
                            #[cfg(not(windows))]
                            let read_len = {
                                let _ = is_block;
                                to_read
                            };
                            match file.read(&mut buf[..read_len]) {
                                Ok(0) => {
                                    return (
                                        idx,
                                        Err((
                                            Some(ErrorCode::Evalidation.as_str().to_string()),
                                            "Unexpected EOF while validating drive".to_string(),
                                        )),
                                    );
                                }
                                Ok(n) => {
                                    let valid_n = n.min(to_read);
                                    h.update(&buf[..valid_n]);
                                    rem -= valid_n as u64;
                                    v_bytes.fetch_add(valid_n as u64, Ordering::Relaxed);
                                }
                                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                                Err(e) => {
                                    let ee = EtcherError::from_io_error(&e, None);
                                    let msg = ee.to_string();
                                    return (idx, Err((ee.0.code, msg)));
                                }
                            }
                        }
                        if h.digest() != *expected_digest {
                            return (
                                idx,
                                Err((
                                    Some(ErrorCode::Evalidation.as_str().to_string()),
                                    "Validation mismatch: data read back differs from source image"
                                        .to_string(),
                                )),
                            );
                        }
                    }
                    (idx, Ok(()))
                }));
            }

            // Poll progress while verification threads run
            while join_handles.iter().any(|jh| !jh.is_finished()) {
                if last_verify_emit.elapsed() >= PROGRESS_THROTTLE {
                    last_verify_emit = Instant::now();
                    let read_avg = verified_bytes_atomic.load(Ordering::Relaxed) / num_targets;
                    emit_progress(
                        &mut on_progress,
                        FlashPhase::Verifying,
                        read_avg,
                        Some(total_verify_bytes),
                        verify_start,
                        surviving_indices.len(),
                        errors.len(),
                    );
                }
                std::thread::sleep(Duration::from_millis(25));
            }

            for jh in join_handles {
                if let Ok(res) = jh.join() {
                    verify_results.push(res);
                }
            }
        });

        if cancel.load(Ordering::Relaxed) {
            cleanup_temp_path(temp_file_path.as_deref());
            return Ok(FlashResults {
                cancelled: true,
                ..Default::default()
            });
        }
        if skip_verify.load(Ordering::Relaxed) {
            skipped_verify = true;
        } else {
            for (idx, res) in verify_results {
                if let Err((code, msg)) = res {
                    let h = &mut handles[idx];
                    h.failed = true;
                    let fe = FlashError {
                        description: h.drive.description.clone(),
                        device: h.drive.device.clone(),
                        device_path: h.drive.device_path.clone(),
                        code,
                        message: msg,
                    };
                    on_fail(fe.clone());
                    errors.push(fe);
                }
            }
        }
    }

    for h in handles.iter().filter(|h| !h.failed && h.is_block) {
        reread_partition_table(&h.file, &h.drive.raw);
    }

    cleanup_temp_path(temp_file_path.as_deref());

    let successful_devices = handles.iter().filter(|h| !h.failed).count();
    let failed_devices = errors.len();

    Ok(FlashResults {
        bytes_written: bytes_written_per_drive,
        source_size: final_source_size,
        blockmapped_size,
        average_flashing_speed,
        successful_devices,
        failed_devices,
        errors,
        cancelled: false,
        skip: skipped_verify,
        source_checksum: Some(source_checksum),
    })
}

fn write_chunk_to_targets<FF>(
    handles: &mut [TargetHandle],
    seek_offset: Option<u64>,
    slice: &[u8],
    errors: &mut Vec<FlashError>,
    on_fail: &mut FF,
) where
    FF: FnMut(FlashError),
{
    let results: Vec<(usize, std::io::Result<()>)> = std::thread::scope(|scope| {
        let mut jobs = Vec::new();
        for (idx, h) in handles.iter_mut().enumerate() {
            if h.failed {
                continue;
            }
            let file = &mut h.file;
            let is_block = h.is_block;
            jobs.push(scope.spawn(move || -> (usize, std::io::Result<()>) {
                if let Some(off) = seek_offset
                    && let Err(e) = file.seek(SeekFrom::Start(off))
                {
                    return (idx, Err(e));
                }
                #[cfg(windows)]
                if is_block && !slice.len().is_multiple_of(4096) {
                    let padded_len = slice.len().div_ceil(4096) * 4096;
                    let mut padded = vec![0u8; padded_len];
                    padded[..slice.len()].copy_from_slice(slice);
                    return (idx, file.write_all(&padded));
                }
                let _ = is_block;
                (idx, file.write_all(slice))
            }));
        }
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    });

    for (idx, res) in results {
        if let Err(e) = res {
            mark_target_failed(&mut handles[idx], e, errors, on_fail);
        }
    }
}

fn mark_target_failed<FF>(
    handle: &mut TargetHandle,
    err: std::io::Error,
    errors: &mut Vec<FlashError>,
    on_fail: &mut FF,
) where
    FF: FnMut(FlashError),
{
    handle.failed = true;
    let ee = EtcherError::from_io_error(&err, Some(&handle.drive.raw));
    let fe = FlashError {
        description: handle.drive.description.clone(),
        device: handle.drive.device.clone(),
        device_path: handle.drive.device_path.clone(),
        code: ee.code.clone(),
        message: ee.to_string(),
    };
    on_fail(fe.clone());
    errors.push(fe);
}

fn emit_progress<FP>(
    on_progress: &mut FP,
    phase: FlashPhase,
    position: u64,
    total: Option<u64>,
    start: Instant,
    active: usize,
    failed: usize,
) where
    FP: FnMut(ProgressState),
{
    let elapsed = start.elapsed().as_secs_f64().max(0.001);
    let speed = position as f64 / elapsed;
    let (percentage, eta) = match total {
        Some(t) if t > 0 => {
            let pct = ((position as f64 / t as f64) * 100.0).clamp(0.0, 100.0).floor() as u8;
            let rem = if speed > 0.0 {
                Some(t.saturating_sub(position) as f64 / speed)
            } else {
                None
            };
            (Some(pct), rem)
        }
        _ => (None, None),
    };
    on_progress(ProgressState {
        phase,
        percentage,
        position,
        total: total.unwrap_or(position),
        bytes_written: position,
        speed,
        average_speed: speed,
        eta,
        active,
        failed,
    });
}

fn cleanup_temp_path(path: Option<&Path>) {
    if let Some(p) = path {
        let _ = std::fs::remove_file(p);
    }
}

fn is_block_device_path(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        std::fs::metadata(path)
            .map(|m| m.file_type().is_block_device())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        path.starts_with(r"\\.\")
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        false
    }
}

fn open_target_file(path: &str) -> std::io::Result<(File, bool)> {
    let is_block = is_block_device_path(path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        if is_block {
            opts.custom_flags(libc::O_SYNC);
        }
        let f = opts.open(path)?;
        Ok((f, is_block))
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_WRITE: u32 = 0x0000_0002;
        const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        if is_block {
            opts.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_WRITE_THROUGH);
        }
        let f = opts.open(path)?;
        Ok((f, is_block))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let f = OpenOptions::new().read(true).write(true).open(path)?;
        Ok((f, is_block))
    }
}

fn flush_block_device(file: &File) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        const BLKFLSBUF: libc::c_ulong = 0x1261;
        unsafe {
            libc::ioctl(file.as_raw_fd(), BLKFLSBUF as _, 0);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = file;
    }
}

fn reread_partition_table(file: &File, device: &str) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        const BLKRRPART: libc::c_ulong = 0x125F;
        let _ = device;
        unsafe {
            libc::ioctl(file.as_raw_fd(), BLKRRPART as _, 0);
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = file;
        if let Some(disk_num) = crate::drives::parse_windows_disk_number(device) {
            let _ = std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &format!("Update-Disk -Number {disk_num}"),
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (file, device);
    }
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::probe_source_metadata;
    use crate::types::{DriveKind, SourceType};

    fn dummy_drive(path: &Path, size: u64) -> Drive {
        let s = path.to_string_lossy().into_owned();
        Drive {
            device: s.clone(),
            raw: s.clone(),
            device_path: None,
            display_name: s,
            description: "Temp Test Target".to_string(),
            size: Some(size),
            is_removable: true,
            is_system: false,
            is_read_only: false,
            is_usb: true,
            is_card: false,
            is_virtual: false,
            bus_type: Some("USB".to_string()),
            mountpoints: Vec::new(),
            partition_table_type: None,
            disabled: false,
            kind: DriveKind::BlockDevice,
        }
    }

    #[test]
    fn test_multi_target_flash_and_verify_gz() {
        let dir = tempfile::tempdir().unwrap();
        let payload: Vec<u8> = (0..(2 * 1024 * 1024 + 12345))
            .map(|i| ((i * 31) % 251) as u8)
            .collect();
        let gz_path = dir.path().join("os.img.gz");
        {
            let mut enc = flate2::write::GzEncoder::new(
                File::create(&gz_path).unwrap(),
                flate2::Compression::fast(),
            );
            enc.write_all(&payload).unwrap();
            enc.finish().unwrap();
        }
        let meta = probe_source_metadata(gz_path.to_str().unwrap(), SourceType::File, None).unwrap();

        let t1 = tempfile::NamedTempFile::new().unwrap();
        let t2 = tempfile::NamedTempFile::new().unwrap();
        t1.as_file().set_len(4 * 1024 * 1024).unwrap();
        t2.as_file().set_len(4 * 1024 * 1024).unwrap();

        let opts = WriteOptions {
            image: meta,
            destinations: vec![
                dummy_drive(t1.path(), 4 * 1024 * 1024),
                dummy_drive(t2.path(), 4 * 1024 * 1024),
            ],
            auto_blockmapping: false,
            decompress_first: true,
            verify: true,
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let skip = Arc::new(AtomicBool::new(false));
        let mut phases = Vec::new();
        let res = write_and_validate(
            &opts,
            cancel,
            skip,
            |p| phases.push(p.phase),
            |e| panic!("unexpected failure: {e:?}"),
            |_| {},
        )
        .unwrap();

        assert_eq!(res.successful_devices, 2);
        assert_eq!(res.failed_devices, 0);
        assert_eq!(res.bytes_written, payload.len() as u64);
        assert!(res.source_checksum.is_some());

        let mut out1 = vec![0u8; payload.len()];
        let mut out2 = vec![0u8; payload.len()];
        t1.reopen().unwrap().read_exact(&mut out1).unwrap();
        t2.reopen().unwrap().read_exact(&mut out2).unwrap();
        assert_eq!(out1, payload);
        assert_eq!(out2, payload);
    }

    #[test]
    fn test_cancel_and_skip_verify() {
        let dir = tempfile::tempdir().unwrap();
        let payload = vec![0x5Au8; 1024 * 1024];
        let img_path = dir.path().join("raw.img");
        std::fs::write(&img_path, &payload).unwrap();
        let meta = probe_source_metadata(img_path.to_str().unwrap(), SourceType::File, None).unwrap();

        let t1 = tempfile::NamedTempFile::new().unwrap();
        t1.as_file().set_len(2 * 1024 * 1024).unwrap();

        // Cancel immediately
        let opts = WriteOptions {
            image: meta.clone(),
            destinations: vec![dummy_drive(t1.path(), 2 * 1024 * 1024)],
            auto_blockmapping: false,
            decompress_first: false,
            verify: true,
        };
        let cancel = Arc::new(AtomicBool::new(true));
        let skip = Arc::new(AtomicBool::new(false));
        let res = write_and_validate(&opts, cancel, skip, |_| {}, |_| {}, |_| {}).unwrap();
        assert!(res.cancelled);

        // Skip verification mid-verify
        let cancel2 = Arc::new(AtomicBool::new(false));
        let skip2 = Arc::new(AtomicBool::new(false));
        let skip_trigger = skip2.clone();
        let res2 = write_and_validate(
            &opts,
            cancel2,
            skip2,
            |p| {
                if p.phase == FlashPhase::Verifying {
                    skip_trigger.store(true, Ordering::Relaxed);
                }
            },
            |_| {},
            |_| {},
        )
        .unwrap();
        assert!(res2.skip);
        assert_eq!(res2.successful_devices, 1);
    }

    #[test]
    fn test_validation_corruption_detection_evalidation() {
        let dir = tempfile::tempdir().unwrap();
        let payload = vec![0x3Cu8; 1024 * 1024 + 512];
        let img_path = dir.path().join("corrupt.img");
        std::fs::write(&img_path, &payload).unwrap();
        let meta = probe_source_metadata(img_path.to_str().unwrap(), SourceType::File, None).unwrap();

        let t1 = tempfile::NamedTempFile::new().unwrap();
        t1.as_file().set_len(2 * 1024 * 1024).unwrap();
        let corrupt_path = t1.path().to_path_buf();

        let opts = WriteOptions {
            image: meta,
            destinations: vec![dummy_drive(t1.path(), 2 * 1024 * 1024)],
            auto_blockmapping: false,
            decompress_first: false,
            verify: true,
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let skip = Arc::new(AtomicBool::new(false));
        let corrupted_once = Arc::new(AtomicBool::new(false));
        let corrupted_flag = corrupted_once.clone();

        let res = write_and_validate(
            &opts,
            cancel,
            skip,
            |p| {
                // Corrupt target right as verification begins
                if p.phase == FlashPhase::Flashing
                    && p.percentage == Some(100)
                    && !corrupted_flag.swap(true, Ordering::SeqCst)
                {
                    let mut f = OpenOptions::new().write(true).open(&corrupt_path).unwrap();
                    f.seek(SeekFrom::Start(0)).unwrap();
                    f.write_all(b"CORRUPTED!").unwrap();
                    f.sync_all().unwrap();
                }
            },
            |_| {},
            |_| {},
        )
        .unwrap();

        assert_eq!(res.successful_devices, 0);
        assert_eq!(res.failed_devices, 1);
        assert_eq!(res.errors[0].code.as_deref(), Some("EVALIDATION"));
    }

    #[test]
    fn test_sparse_ext2_trimmed_write() {
        let dir = tempfile::tempdir().unwrap();
        let total = 4 * 1024 * 1024usize;
        let part_offset = 1024 * 1024u64;
        let part_size = 3 * 1024 * 1024u64;
        let mut img = vec![0u8; total];

        // MBR
        img[510] = 0x55;
        img[511] = 0xAA;
        img[446 + 4] = 0x83;
        img[446 + 8..446 + 12].copy_from_slice(&((part_offset / 512) as u32).to_le_bytes());
        img[446 + 12..446 + 16].copy_from_slice(&((part_size / 512) as u32).to_le_bytes());

        // Ext2 superblock
        let sb_off = part_offset as usize + 1024;
        img[sb_off + 4..sb_off + 8].copy_from_slice(&3072u32.to_le_bytes());
        img[sb_off + 20..sb_off + 24].copy_from_slice(&1u32.to_le_bytes());
        img[sb_off + 24..sb_off + 28].copy_from_slice(&0u32.to_le_bytes());
        img[sb_off + 32..sb_off + 36].copy_from_slice(&8192u32.to_le_bytes());
        img[sb_off + 56..sb_off + 58].copy_from_slice(&0xEF53u16.to_le_bytes());
        let gdt_off = part_offset as usize + 2048;
        img[gdt_off..gdt_off + 4].copy_from_slice(&3u32.to_le_bytes());
        let bmp_off = part_offset as usize + 3072;
        img[bmp_off] = 0xFF;

        let img_path = dir.path().join("ext2.img");
        std::fs::write(&img_path, &img).unwrap();
        let meta = probe_source_metadata(img_path.to_str().unwrap(), SourceType::File, None).unwrap();

        let t1 = tempfile::NamedTempFile::new().unwrap();
        t1.as_file().set_len(total as u64).unwrap();

        let opts = WriteOptions {
            image: meta,
            destinations: vec![dummy_drive(t1.path(), total as u64)],
            auto_blockmapping: true,
            decompress_first: true,
            verify: true,
        };

        let res = write_and_validate(
            &opts,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            |_| {},
            |e| panic!("unexpected fail: {e:?}"),
            |_| {},
        )
        .unwrap();

        assert_eq!(res.successful_devices, 1);
        assert!(res.blockmapped_size.unwrap() < total as u64);
        assert!(res.bytes_written < total as u64);
    }
}
