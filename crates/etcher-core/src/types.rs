/*
 * Copyright 2026 balena.io / Rust rewrite
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mountpoint {
    pub path: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DriveKind {
    #[default]
    BlockDevice,
    Usbboot {
        progress: u8,
    },
    Driverless {
        link: String,
        link_title: String,
        link_message: String,
        link_cta: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub device: String,
    pub raw: String,
    #[serde(default)]
    pub device_path: Option<String>,
    pub display_name: String,
    pub description: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub is_removable: bool,
    #[serde(default)]
    pub is_system: bool,
    #[serde(default)]
    pub is_read_only: bool,
    #[serde(default)]
    pub is_usb: bool,
    #[serde(default)]
    pub is_card: bool,
    #[serde(default)]
    pub is_virtual: bool,
    #[serde(default)]
    pub bus_type: Option<String>,
    #[serde(default)]
    pub mountpoints: Vec<Mountpoint>,
    #[serde(default)]
    pub partition_table_type: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub kind: DriveKind,
}

impl Drive {
    pub fn is_usbboot(&self) -> bool {
        matches!(self.kind, DriveKind::Usbboot { .. })
    }

    pub fn is_driverless(&self) -> bool {
        matches!(self.kind, DriveKind::Driverless { .. })
    }

    pub fn is_block_device(&self) -> bool {
        matches!(self.kind, DriveKind::BlockDevice)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    File,
    Http,
    BlockDevice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Authentication {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionInfo {
    pub index: u32,
    pub offset: u64,
    pub size: u64,
    #[serde(default)]
    pub fs_type: Option<String>,
    #[serde(default)]
    pub part_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMetadata {
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
    pub display_name: String,
    pub description: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub compressed_size: Option<u64>,
    #[serde(default)]
    pub is_size_estimated: bool,
    #[serde(default)]
    pub recommended_drive_size: Option<u64>,
    #[serde(default)]
    pub has_mbr: bool,
    #[serde(default)]
    pub partitions: Vec<PartitionInfo>,
    pub source_type: SourceType,
    #[serde(default)]
    pub extension: Option<String>,
    #[serde(default)]
    pub archive_extension: Option<String>,
    #[serde(default)]
    pub drive: Option<Drive>,
    #[serde(default)]
    pub auth: Option<Authentication>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub support_url: Option<String>,
    #[serde(default)]
    pub is_compressed: bool,
    #[serde(default)]
    pub blockmapped_size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FlashPhase {
    #[default]
    Starting,
    Decompressing,
    Flashing,
    Verifying,
    Finishing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProgressState {
    pub phase: FlashPhase,
    #[serde(default)]
    pub percentage: Option<u8>,
    pub position: u64,
    pub total: u64,
    pub bytes_written: u64,
    /// Current speed in bytes/sec (writer) or MB/s (store)
    pub speed: f64,
    /// Average speed in bytes/sec
    pub average_speed: f64,
    #[serde(default)]
    pub eta: Option<f64>,
    pub active: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashError {
    pub description: String,
    pub device: String,
    #[serde(default)]
    pub device_path: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlashResults {
    pub bytes_written: u64,
    pub source_size: u64,
    #[serde(default)]
    pub blockmapped_size: Option<u64>,
    pub average_flashing_speed: f64,
    pub successful_devices: usize,
    pub failed_devices: usize,
    #[serde(default)]
    pub errors: Vec<FlashError>,
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub skip: bool,
    #[serde(default)]
    pub source_checksum: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteOptions {
    pub image: SourceMetadata,
    pub destinations: Vec<Drive>,
    pub auto_blockmapping: bool,
    pub decompress_first: bool,
    pub verify: bool,
}
