use std::path::Path;
use uuid::Uuid;

use crate::drive_constraints::{
    is_drive_large_enough, is_drive_size_large, is_drive_size_recommended, is_drive_valid,
    is_system_drive,
};
use crate::errors::EtcherError;
use crate::settings::Settings;
use crate::types::{Drive, FlashError, FlashResults, ProgressState, SourceMetadata};
use crate::units::bytes_to_megabytes;

#[derive(Debug, Clone)]
pub struct AppStore {
    pub application_session_uuid: String,
    pub flashing_workflow_uuid: String,
    pub available_drives: Vec<Drive>,
    pub selected_devices: Vec<String>,
    pub selected_image: Option<SourceMetadata>,
    pub is_flashing: bool,
    pub flash_uuid: Option<String>,
    pub device_paths: Vec<String>,
    pub failed_device_errors: Vec<FlashError>,
    pub flash_results: Option<FlashResults>,
    pub flash_state: ProgressState,
    pub last_average_flashing_speed: Option<f64>,
}

impl Default for AppStore {
    fn default() -> Self {
        Self::new()
    }
}

impl AppStore {
    pub fn new() -> Self {
        Self {
            application_session_uuid: Uuid::new_v4().to_string(),
            flashing_workflow_uuid: Uuid::new_v4().to_string(),
            available_drives: Vec::new(),
            selected_devices: Vec::new(),
            selected_image: None,
            is_flashing: false,
            flash_uuid: None,
            device_paths: Vec::new(),
            failed_device_errors: Vec::new(),
            flash_results: None,
            flash_state: ProgressState::default(),
            last_average_flashing_speed: None,
        }
    }

    pub fn new_workflow_uuid(&mut self) {
        self.flashing_workflow_uuid = Uuid::new_v4().to_string();
    }

    pub fn set_available_drives(&mut self, mut drives: Vec<Drive>, settings: &Settings) {
        // Port of store.ts sorting:
        // 1) System drives last
        // 2) Devices with no devicePath first (usbboot)
        // 3) drivesOrder index (if present; -1 comes before >= 0 in JS indexOf, matching JS sortBy)
        // 4) devicePath || device
        let drives_order = &settings.drives_order;
        drives.sort_by(|a, b| {
            let key_a = (
                a.is_system,
                a.device_path.is_some(),
                drives_order_index(drives_order, a.device_path.as_deref()),
                a.device_path.as_deref().unwrap_or(&a.device).to_string(),
            );
            let key_b = (
                b.is_system,
                b.device_path.is_some(),
                drives_order_index(drives_order, b.device_path.as_deref()),
                b.device_path.as_deref().unwrap_or(&b.device).to_string(),
            );
            key_a.cmp(&key_b)
        });

        self.available_drives = drives;

        // Remove stale selected devices missing from available_drives
        self.selected_devices
            .retain(|dev| self.available_drives.iter().any(|d| &d.device == dev));

        let should_autoselect_all = settings.auto_select_all_drives;
        let has_selected_devices = !self.selected_devices.is_empty();
        let should_autoselect_one = self.available_drives.len() == 1 && !has_selected_devices;

        if should_autoselect_one || should_autoselect_all {
            let image = self.selected_image.clone();
            let drives_snapshot = self.available_drives.clone();
            for drive in drives_snapshot {
                let valid = is_drive_valid(&drive, image.as_ref(), true)
                    && !drive.is_read_only
                    && is_drive_size_recommended(&drive, image.as_ref())
                    && (!is_drive_size_large(&drive) || should_autoselect_all)
                    && !is_system_drive(&drive);
                if valid {
                    let _ = self.select_target(&drive.device);
                } else {
                    self.deselect_target(&drive.device);
                }
            }
        }
    }

    pub fn select_source(&mut self, source: SourceMetadata) -> Result<(), EtcherError> {
        if source.path.is_empty() && source.drive.is_none() {
            return Err(EtcherError::new("Invalid image path", "Path cannot be empty"));
        }
        let image_ref = Some(&source);
        let available = &self.available_drives;
        self.selected_devices.retain(|device| {
            if let Some(drive) = available.iter().find(|d| &d.device == device) {
                is_drive_valid(drive, image_ref, true)
                    && is_drive_size_recommended(drive, image_ref)
            } else {
                false
            }
        });
        self.selected_image = Some(source);
        Ok(())
    }

    pub fn deselect_source(&mut self) {
        self.selected_image = None;
    }

    pub fn select_target(&mut self, device: &str) -> Result<(), EtcherError> {
        if device.is_empty() {
            return Err(EtcherError::new("Missing drive", "Device string is empty"));
        }
        let Some(drive) = self.available_drives.iter().find(|d| d.device == device) else {
            return Err(EtcherError::new(
                format!("The drive is not available: {device}"),
                "Drive could not be found",
            ));
        };
        if drive.is_read_only {
            return Err(EtcherError::new(
                "The drive is write-protected",
                "Cannot select a read-only drive",
            ));
        }
        if !is_drive_large_enough(drive, self.selected_image.as_ref()) {
            return Err(EtcherError::new(
                "The drive is not large enough",
                "Select a larger drive",
            ));
        }
        if !self.selected_devices.iter().any(|d| d == device) {
            self.selected_devices.push(device.to_string());
        }
        Ok(())
    }

    pub fn deselect_target(&mut self, device: &str) {
        self.selected_devices.retain(|d| d != device);
    }

    pub fn toggle_target(&mut self, device: &str) -> Result<(), EtcherError> {
        if self.is_drive_selected(device) {
            self.deselect_target(device);
            Ok(())
        } else {
            self.select_target(device)
        }
    }

    pub fn deselect_all_targets(&mut self) {
        self.selected_devices.clear();
    }

    pub fn clear_selection(&mut self) {
        self.deselect_source();
        self.deselect_all_targets();
    }

    pub fn is_drive_selected(&self, device: &str) -> bool {
        !device.is_empty() && self.selected_devices.iter().any(|d| d == device)
    }

    pub fn selected_drives(&self) -> Vec<Drive> {
        self.available_drives
            .iter()
            .filter(|d| self.selected_devices.iter().any(|dev| dev == &d.device))
            .cloned()
            .collect()
    }

    pub fn has_drive(&self) -> bool {
        !self.selected_devices.is_empty()
    }

    pub fn has_image(&self) -> bool {
        self.selected_image.is_some()
    }

    pub fn set_flashing_flag(&mut self) {
        self.is_flashing = true;
        self.flash_uuid = Some(Uuid::new_v4().to_string());
        self.flash_results = None;
        self.failed_device_errors.clear();
        self.flash_state = ProgressState::default();
    }

    pub fn set_device_paths(&mut self, paths: Vec<String>) {
        self.device_paths = paths;
    }

    pub fn set_progress_state(&mut self, mut state: ProgressState) {
        if !self.is_flashing {
            return;
        }
        // Convert speed from bytes/sec to MB/s rounded to 1 decimal place, matching flash-state.ts
        if state.speed.is_finite() {
            state.speed = (bytes_to_megabytes(state.speed) * 10.0).round() / 10.0;
        } else {
            state.speed = 0.0;
        }
        if state.phase == crate::types::FlashPhase::Flashing && state.average_speed.is_finite() {
            self.last_average_flashing_speed = Some(state.average_speed);
        }
        self.flash_state = state;
    }

    pub fn add_failed_device_error(&mut self, err: FlashError) {
        if self
            .failed_device_errors
            .iter()
            .any(|e| e.device == err.device)
        {
            return;
        }
        self.failed_device_errors.push(err);
    }

    pub fn unset_flashing_flag(&mut self, mut results: FlashResults) {
        if let Some(avg) = self.last_average_flashing_speed
            && results.average_flashing_speed <= 0.0
        {
            results.average_flashing_speed = avg;
        }
        let skip = results.skip;
        self.is_flashing = false;
        self.flash_results = Some(results);
        if !skip {
            self.flash_state = ProgressState::default();
        }
    }

    pub fn reset_flash_state(&mut self) {
        self.is_flashing = false;
        self.flash_state = ProgressState::default();
        self.flash_results = None;
        self.device_paths.clear();
        self.failed_device_errors.clear();
        self.last_average_flashing_speed = None;
        self.flash_uuid = None;
    }

    pub fn effective_speed_mb_per_sec(&self) -> f64 {
        let Some(res) = &self.flash_results else {
            return 0.0;
        };
        let flashed_size = res.blockmapped_size.unwrap_or(res.source_size);
        let avg_speed = if res.average_flashing_speed > 0.0 {
            res.average_flashing_speed
        } else {
            self.last_average_flashing_speed.unwrap_or(0.0)
        };
        if flashed_size == 0 || avg_speed <= 0.0 || res.source_size == 0 {
            return 0.0;
        }
        let time_spent = flashed_size as f64 / avg_speed;
        let effective_bytes_per_sec = res.source_size as f64 / time_spent;
        bytes_to_megabytes(effective_bytes_per_sec)
    }
}

fn drives_order_index(order: &[String], device_path: Option<&str>) -> isize {
    let Some(dp) = device_path else {
        return -1;
    };
    let base = Path::new(dp)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(dp);
    order
        .iter()
        .position(|p| p == base)
        .map(|i| i as isize)
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceType;

    fn make_drive(dev: &str, size: u64, system: bool, dev_path: Option<&str>) -> Drive {
        Drive {
            device: dev.to_string(),
            raw: dev.to_string(),
            device_path: dev_path.map(ToString::to_string),
            display_name: dev.to_string(),
            description: format!("Drive {dev}"),
            size: Some(size),
            is_removable: !system,
            is_system: system,
            is_read_only: false,
            is_usb: !system,
            is_card: false,
            is_virtual: false,
            bus_type: Some("USB".to_string()),
            mountpoints: Vec::new(),
            partition_table_type: Some("gpt".to_string()),
            disabled: false,
            kind: Default::default(),
        }
    }

    #[test]
    fn test_store_sorting_and_autoselect() {
        let mut store = AppStore::new();
        let settings = Settings::default();

        // Single valid removable drive gets auto-selected
        let usb = make_drive("/dev/sdb", 32_000_000_000, false, Some("/dev/disk/by-path/usb-1"));
        store.set_available_drives(vec![usb.clone()], &settings);
        assert_eq!(store.selected_devices, vec!["/dev/sdb"]);

        // System drives sorted last, stale drives removed
        let sys = make_drive("/dev/sda", 512_000_000_000, true, Some("/dev/disk/by-path/sata-0"));
        let usb2 = make_drive("/dev/sdc", 16_000_000_000, false, Some("/dev/disk/by-path/usb-2"));
        store.set_available_drives(vec![sys, usb2], &settings);
        assert_eq!(store.available_drives[0].device, "/dev/sdc");
        assert_eq!(store.available_drives[1].device, "/dev/sda");
        assert!(store.selected_devices.is_empty());
    }

    #[test]
    fn test_select_source_prunes_incompatible_targets() {
        let mut store = AppStore::new();
        let settings = Settings::default();
        let small = make_drive("/dev/sdb", 1_000_000_000, false, None);
        let large = make_drive("/dev/sdc", 16_000_000_000, false, None);
        store.set_available_drives(vec![small, large], &settings);
        store.select_target("/dev/sdb").unwrap();
        store.select_target("/dev/sdc").unwrap();

        let img = SourceMetadata {
            path: "/tmp/os.img".to_string(),
            name: Some("os.img".to_string()),
            display_name: "os.img".to_string(),
            description: "os.img".to_string(),
            size: Some(4_000_000_000),
            compressed_size: None,
            is_size_estimated: false,
            recommended_drive_size: None,
            has_mbr: true,
            partitions: Vec::new(),
            source_type: SourceType::File,
            extension: Some("img".to_string()),
            archive_extension: None,
            drive: None,
            auth: None,
            logo: None,
            support_url: None,
            is_compressed: false,
            blockmapped_size: None,
        };
        store.select_source(img).unwrap();
        assert_eq!(store.selected_devices, vec!["/dev/sdc"]);
    }
}
