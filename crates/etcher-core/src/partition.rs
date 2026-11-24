use std::io::{Read, Seek, SeekFrom};

use crate::types::PartitionInfo;

const SECTOR_SIZE: u64 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    pub offset: u64,
    pub length: u64,
}

/// Inspects the first bytes (ideally >= 32 KiB) of an image or disk to detect MBR or GPT partition tables.
pub fn inspect_partition_table(header: &[u8]) -> (bool, Vec<PartitionInfo>) {
    if header.len() < 512 {
        return (false, Vec::new());
    }

    // Check GPT signature "EFI PART" at LBA 1 (offset 512)
    if header.len() >= 1024 && &header[512..520] == b"EFI PART" {
        let entry_lba = u64::from_le_bytes(header[584..592].try_into().unwrap_or([0; 8]));
        let num_entries = u32::from_le_bytes(header[592..596].try_into().unwrap_or([0; 4])) as usize;
        let entry_size = u32::from_le_bytes(header[596..600].try_into().unwrap_or([0; 4])) as usize;

        if entry_size >= 128 && num_entries > 0 && num_entries <= 1024 {
            let table_offset = (entry_lba as usize).saturating_mul(512);
            let mut partitions = Vec::new();
            for i in 0..num_entries {
                let start = table_offset.saturating_add(i * entry_size);
                let end = start.saturating_add(entry_size);
                if end > header.len() {
                    break;
                }
                let entry = &header[start..end];
                // Empty GUID means unused entry
                if entry[0..16].iter().all(|&b| b == 0) {
                    continue;
                }
                let first_lba = u64::from_le_bytes(entry[32..40].try_into().unwrap_or([0; 8]));
                let last_lba = u64::from_le_bytes(entry[40..48].try_into().unwrap_or([0; 8]));
                if last_lba >= first_lba {
                    partitions.push(PartitionInfo {
                        index: (i + 1) as u32,
                        offset: first_lba.saturating_mul(SECTOR_SIZE),
                        size: (last_lba - first_lba + 1).saturating_mul(SECTOR_SIZE),
                        fs_type: None,
                        part_type: 0xEE,
                    });
                }
            }
            return (true, partitions);
        }
        return (true, Vec::new());
    }

    // Check MBR signature 0x55 0xAA at 510..512
    if header[510] == 0x55 && header[511] == 0xAA {
        let mut partitions = Vec::new();
        for i in 0..4 {
            let base = 446 + i * 16;
            let ptype = header[base + 4];
            let lba_start =
                u32::from_le_bytes(header[base + 8..base + 12].try_into().unwrap_or([0; 4]))
                    as u64;
            let num_sectors =
                u32::from_le_bytes(header[base + 12..base + 16].try_into().unwrap_or([0; 4]))
                    as u64;
            if ptype != 0 && num_sectors > 0 {
                partitions.push(PartitionInfo {
                    index: (i + 1) as u32,
                    offset: lba_start.saturating_mul(SECTOR_SIZE),
                    size: num_sectors.saturating_mul(SECTOR_SIZE),
                    fs_type: None,
                    part_type: ptype,
                });
            }
        }
        // Only treat as valid partition table if at least one entry is non-empty
        // or the boot signature is present with MBR entries
        return (!partitions.is_empty(), partitions);
    }

    (false, Vec::new())
}

/// Computes sparse extents for raw images containing ext2/ext3/ext4 partitions (`auto_blockmapping`).
/// Non-partition regions (headers, bootloaders, backup GPT) and non-ext partitions are preserved in full.
/// Returns `Some(extents)` only if blockmapping skips at least 1 MiB.
pub fn compute_sparse_extents<R: Read + Seek>(
    reader: &mut R,
    total_size: u64,
    partitions: &[PartitionInfo],
) -> Option<Vec<Extent>> {
    if total_size == 0 || partitions.is_empty() {
        return None;
    }

    let mut sorted = partitions.to_vec();
    sorted.sort_by_key(|p| p.offset);

    let mut raw_extents: Vec<Extent> = Vec::new();
    let mut cursor: u64 = 0;

    for part in &sorted {
        if part.offset >= total_size || part.size == 0 {
            continue;
        }
        let part_end = part.offset.saturating_add(part.size).min(total_size);
        if part.offset > cursor {
            raw_extents.push(Extent {
                offset: cursor,
                length: part.offset - cursor,
            });
        }
        if part_end > cursor {
            let start = part.offset.max(cursor);
            let len = part_end - start;
            if let Some(ext_extents) = scan_ext_partition(reader, start, len) {
                raw_extents.extend(ext_extents);
            } else {
                raw_extents.push(Extent {
                    offset: start,
                    length: len,
                });
            }
            cursor = part_end;
        }
    }

    if cursor < total_size {
        raw_extents.push(Extent {
            offset: cursor,
            length: total_size - cursor,
        });
    }

    let merged = merge_extents(raw_extents);
    let mapped_bytes: u64 = merged.iter().map(|e| e.length).sum();
    if total_size.saturating_sub(mapped_bytes) >= 1024 * 1024 {
        Some(merged)
    } else {
        None
    }
}

fn scan_ext_partition<R: Read + Seek>(
    reader: &mut R,
    part_offset: u64,
    part_size: u64,
) -> Option<Vec<Extent>> {
    if part_size < 2048 {
        return None;
    }
    reader.seek(SeekFrom::Start(part_offset + 1024)).ok()?;
    let mut sb = [0u8; 1024];
    reader.read_exact(&mut sb).ok()?;

    let magic = u16::from_le_bytes([sb[56], sb[57]]);
    if magic != 0xEF53 {
        return None;
    }

    let s_blocks_count_lo = u32::from_le_bytes(sb[4..8].try_into().ok()?) as u64;
    let s_first_data_block = u32::from_le_bytes(sb[20..24].try_into().ok()?) as u64;
    let s_log_block_size = u32::from_le_bytes(sb[24..28].try_into().ok()?);
    if s_log_block_size > 6 {
        return None;
    }
    let block_size = 1024u64 << s_log_block_size;
    let s_blocks_per_group = u32::from_le_bytes(sb[32..36].try_into().ok()?) as u64;
    if s_blocks_per_group == 0 || s_blocks_per_group > block_size * 8 {
        return None;
    }
    let s_incompat_feature = u32::from_le_bytes(sb[96..100].try_into().ok()?);
    let is_64bit = (s_incompat_feature & 0x0080) != 0;
    let s_desc_size = u16::from_le_bytes(sb[254..256].try_into().ok()?) as u64;
    let desc_size = if is_64bit && s_desc_size >= 64 {
        s_desc_size
    } else {
        32
    };
    let s_blocks_count_hi = if is_64bit {
        u32::from_le_bytes(sb[0x150..0x154].try_into().ok()?) as u64
    } else {
        0
    };
    let total_blocks = (s_blocks_count_hi << 32) | s_blocks_count_lo;
    if total_blocks == 0 || total_blocks.saturating_mul(block_size) > part_size {
        return None;
    }

    // Group descriptor table sits in the block immediately following the superblock
    let gdt_block = s_first_data_block + 1;
    let gdt_offset = part_offset + gdt_block * block_size;
    let num_groups = total_blocks.div_ceil(s_blocks_per_group);
    if num_groups == 0 || num_groups > 1_000_000 {
        return None;
    }

    let mut gdt = vec![0u8; (num_groups * desc_size) as usize];
    reader.seek(SeekFrom::Start(gdt_offset)).ok()?;
    reader.read_exact(&mut gdt).ok()?;

    let mut extents = Vec::new();
    // Always preserve anything before the first data block (e.g., 1024-byte boot block for block_size > 1024)
    if s_first_data_block == 0 {
        extents.push(Extent {
            offset: part_offset,
            length: block_size,
        });
    } else {
        extents.push(Extent {
            offset: part_offset,
            length: s_first_data_block * block_size,
        });
    }

    let mut bitmap_buf = vec![0u8; block_size as usize];

    for g in 0..num_groups {
        let gd = &gdt[(g * desc_size) as usize..((g + 1) * desc_size) as usize];
        let bmp_lo = u32::from_le_bytes(gd[0..4].try_into().ok()?) as u64;
        let bmp_hi = if is_64bit && desc_size >= 64 {
            u32::from_le_bytes(gd[32..36].try_into().ok()?) as u64
        } else {
            0
        };
        let bmp_block = (bmp_hi << 32) | bmp_lo;
        if bmp_block == 0 || bmp_block >= total_blocks {
            return None;
        }
        let bmp_byte_offset = part_offset + bmp_block * block_size;
        reader.seek(SeekFrom::Start(bmp_byte_offset)).ok()?;
        reader.read_exact(&mut bitmap_buf).ok()?;

        let group_start_block = s_first_data_block + g * s_blocks_per_group;
        let remaining_blocks = total_blocks.saturating_sub(g * s_blocks_per_group);
        let group_blocks = remaining_blocks.min(s_blocks_per_group);

        let mut run_start: Option<u64> = None;
        for b in 0..group_blocks {
            let byte_idx = (b / 8) as usize;
            let bit_idx = (b % 8) as u8;
            let allocated = (bitmap_buf[byte_idx] & (1 << bit_idx)) != 0;
            let abs_block = group_start_block + b;
            if allocated {
                if run_start.is_none() {
                    run_start = Some(abs_block);
                }
            } else if let Some(start_b) = run_start.take() {
                extents.push(Extent {
                    offset: part_offset + start_b * block_size,
                    length: (abs_block - start_b) * block_size,
                });
            }
        }
        if let Some(start_b) = run_start {
            let end_block = group_start_block + group_blocks;
            extents.push(Extent {
                offset: part_offset + start_b * block_size,
                length: (end_block - start_b) * block_size,
            });
        }
    }

    // Preserve any trailing partition bytes beyond total_blocks * block_size
    let fs_end = part_offset + total_blocks * block_size;
    let part_end = part_offset + part_size;
    if part_end > fs_end {
        extents.push(Extent {
            offset: fs_end,
            length: part_end - fs_end,
        });
    }

    Some(merge_extents(extents))
}

fn merge_extents(mut extents: Vec<Extent>) -> Vec<Extent> {
    extents.retain(|e| e.length > 0);
    extents.sort_by_key(|e| e.offset);
    let mut merged: Vec<Extent> = Vec::with_capacity(extents.len());
    for ext in extents {
        if let Some(last) = merged.last_mut() {
            let last_end = last.offset.saturating_add(last.length);
            if ext.offset <= last_end {
                let new_end = last_end.max(ext.offset.saturating_add(ext.length));
                last.length = new_end - last.offset;
                continue;
            }
        }
        merged.push(ext);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_inspect_mbr() {
        let mut buf = vec![0u8; 4096];
        buf[510] = 0x55;
        buf[511] = 0xAA;
        // Partition 1: type 0x83, start 2048, size 4096 sectors
        buf[446 + 4] = 0x83;
        buf[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());
        buf[446 + 12..446 + 16].copy_from_slice(&4096u32.to_le_bytes());

        let (has_tbl, parts) = inspect_partition_table(&buf);
        assert!(has_tbl);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].offset, 2048 * 512);
        assert_eq!(parts[0].size, 4096 * 512);
    }

    #[test]
    fn test_inspect_gpt() {
        let mut buf = vec![0u8; 32 * 1024];
        buf[512..520].copy_from_slice(b"EFI PART");
        buf[584..592].copy_from_slice(&2u64.to_le_bytes()); // entry LBA 2 -> offset 1024
        buf[592..596].copy_from_slice(&128u32.to_le_bytes());
        buf[596..600].copy_from_slice(&128u32.to_le_bytes());

        // Entry 0 at offset 1024
        buf[1024] = 0xAF; // non-zero GUID
        buf[1024 + 32..1024 + 40].copy_from_slice(&2048u64.to_le_bytes());
        buf[1024 + 40..1024 + 48].copy_from_slice(&6143u64.to_le_bytes());

        let (has_tbl, parts) = inspect_partition_table(&buf);
        assert!(has_tbl);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].offset, 2048 * 512);
        assert_eq!(parts[0].size, 4096 * 512);
    }

    #[test]
    fn test_ext2_sparse_trimming() {
        // Create 4 MiB synthetic raw image: 1 MiB header + 3 MiB ext2 partition (3072 x 1KiB blocks, mostly free)
        let total = 4 * 1024 * 1024;
        let part_offset = 1024 * 1024u64;
        let part_size = 3 * 1024 * 1024u64;
        let mut img = vec![0u8; total];

        // MBR
        img[510] = 0x55;
        img[511] = 0xAA;
        img[446 + 4] = 0x83;
        img[446 + 8..446 + 12].copy_from_slice(&((part_offset / 512) as u32).to_le_bytes());
        img[446 + 12..446 + 16].copy_from_slice(&((part_size / 512) as u32).to_le_bytes());

        // Ext2 superblock at part_offset + 1024 (block_size = 1024, first_data_block = 1)
        let sb_off = part_offset as usize + 1024;
        let total_blocks = 3072u32;
        img[sb_off + 4..sb_off + 8].copy_from_slice(&total_blocks.to_le_bytes());
        img[sb_off + 20..sb_off + 24].copy_from_slice(&1u32.to_le_bytes()); // s_first_data_block = 1
        img[sb_off + 24..sb_off + 28].copy_from_slice(&0u32.to_le_bytes()); // 1024-byte blocks
        img[sb_off + 32..sb_off + 36].copy_from_slice(&8192u32.to_le_bytes()); // blocks per group
        img[sb_off + 56..sb_off + 58].copy_from_slice(&0xEF53u16.to_le_bytes());

        // GDT at block 2 (part_offset + 2048): block bitmap at block 3
        let gdt_off = part_offset as usize + 2048;
        img[gdt_off..gdt_off + 4].copy_from_slice(&3u32.to_le_bytes());

        // Block bitmap at block 3 (part_offset + 3072): mark only first 16 blocks allocated
        let bmp_off = part_offset as usize + 3072;
        img[bmp_off] = 0xFF;
        img[bmp_off + 1] = 0xFF;

        let (has_tbl, parts) = inspect_partition_table(&img[..4096]);
        assert!(has_tbl);
        let mut cursor = Cursor::new(&img);
        let extents = compute_sparse_extents(&mut cursor, total as u64, &parts).unwrap();
        let mapped: u64 = extents.iter().map(|e| e.length).sum();
        assert!(mapped < 1_100_000, "mapped bytes should be ~1 MiB + 17 KiB, got {mapped}");
    }
}
