#[cfg(target_os = "linux")]
use std::path::Path;

use crate::errors::EtcherError;
use crate::settings::Settings;
use crate::types::{Drive, DriveKind, Mountpoint};

pub fn scan_drives(settings: &Settings) -> Result<Vec<Drive>, EtcherError> {
    #[cfg(target_os = "linux")]
    {
        scan_drives_linux(settings)
    }
    #[cfg(target_os = "macos")]
    {
        scan_drives_macos(settings)
    }
    #[cfg(target_os = "windows")]
    {
        scan_drives_windows(settings)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = settings;
        Ok(Vec::new())
    }
}

pub fn unmount_drive_partitions(device: &str) -> Result<(), EtcherError> {
    #[cfg(target_os = "linux")]
    {
        unmount_drive_partitions_linux(device)
    }
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("diskutil")
            .args(["unmountDisk", device])
            .status()
            .map_err(|e| EtcherError::from_io_error(&e, Some(device)))?;
        if !status.success() {
            return Err(EtcherError::new("Unmount failed", format!("diskutil failed for {device}")));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        unmount_drive_partitions_windows(device)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = device;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn scan_drives_linux(settings: &Settings) -> Result<Vec<Drive>, EtcherError> {
    use std::collections::HashMap;
    use std::fs;

    let mounts = parse_proc_mounts();
    let by_path_map = build_by_path_map();
    let mut drives = Vec::new();

    let entries = fs::read_dir("/sys/block")
        .map_err(|e| EtcherError::from_io_error(&e, Some("/sys/block")))?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_ignored_block_name(&name) {
            continue;
        }

        let sys_dir = entry.path();
        let sectors: u64 = read_trimmed(&sys_dir.join("size"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if sectors == 0 {
            continue;
        }
        let size = sectors.saturating_mul(512);

        let is_read_only = read_trimmed(&sys_dir.join("ro")).as_deref() == Some("1");
        let is_removable = read_trimmed(&sys_dir.join("removable")).as_deref() == Some("1");

        let canonical_sys = fs::canonicalize(&sys_dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_usb = canonical_sys.contains("/usb");
        let is_card = canonical_sys.contains("/mmc") || name.starts_with("mmcblk");
        let bus_type = if is_usb {
            Some("USB".to_string())
        } else if is_card {
            Some("MMC".to_string())
        } else if name.starts_with("nvme") {
            Some("NVMe".to_string())
        } else if canonical_sys.contains("/ata") {
            Some("ATA".to_string())
        } else {
            None
        };

        let mut udev_props: HashMap<String, String> = HashMap::new();
        if let Some(dev_str) = read_trimmed(&sys_dir.join("dev")) {
            let udev_path = format!("/run/udev/data/b{dev_str}");
            if let Ok(contents) = fs::read_to_string(udev_path) {
                for line in contents.lines() {
                    if let Some(rest) = line.strip_prefix("E:")
                        && let Some((k, v)) = rest.split_once('=')
                    {
                        udev_props.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }

        let vendor = read_trimmed(&sys_dir.join("device/vendor"))
            .or_else(|| udev_props.get("ID_VENDOR").cloned())
            .unwrap_or_default()
            .replace('_', " ");
        let model = read_trimmed(&sys_dir.join("device/model"))
            .or_else(|| udev_props.get("ID_MODEL").cloned())
            .unwrap_or_default()
            .replace('_', " ");

        let description = match (vendor.trim(), model.trim()) {
            ("", "") => name.clone(),
            ("", m) => m.to_string(),
            (v, "") => v.to_string(),
            (v, m) if m.to_ascii_lowercase().starts_with(&v.to_ascii_lowercase()) => m.to_string(),
            (v, m) => format!("{v} {m}"),
        };

        let device = format!("/dev/{name}");
        let device_path = by_path_map.get(&device).cloned();

        if settings.drive_blacklist.iter().any(|b| {
            b == &device || device_path.as_deref() == Some(b.as_str())
        }) {
            continue;
        }

        let mut drive_mounts = Vec::new();
        for (mp_dev, mp) in &mounts {
            if mp_dev == &device || is_partition_of(mp_dev, &device) {
                drive_mounts.push(mp.clone());
            }
        }

        let has_system_mount = drive_mounts.iter().any(|m| {
            matches!(
                m.path.as_str(),
                "/" | "/boot" | "/boot/efi" | "/home" | "/var" | "/usr" | "/etc" | "/opt" | "/nix"
            )
        });
        let is_system = has_system_mount || (!is_removable && !is_usb && !is_card);

        let partition_table_type = udev_props.get("ID_PART_TABLE_TYPE").cloned();

        drives.push(Drive {
            device: device.clone(),
            raw: device.clone(),
            device_path,
            display_name: device,
            description,
            size: Some(size),
            is_removable,
            is_system,
            is_read_only,
            is_usb,
            is_card,
            is_virtual: canonical_sys.contains("/devices/virtual/"),
            bus_type,
            mountpoints: drive_mounts,
            partition_table_type,
            disabled: false,
            kind: DriveKind::BlockDevice,
        });
    }

    Ok(drives)
}

#[cfg(target_os = "linux")]
fn is_ignored_block_name(name: &str) -> bool {
    name.starts_with("loop")
        || name.starts_with("ram")
        || name.starts_with("zram")
        || name.starts_with("dm-")
        || name.starts_with("sr")
        || name.starts_with("fd")
        || name.starts_with("nbd")
        || name.starts_with("md")
}

#[cfg(target_os = "linux")]
fn is_partition_of(part_dev: &str, disk_dev: &str) -> bool {
    let Some(suffix) = part_dev.strip_prefix(disk_dev) else {
        return false;
    };
    if suffix.is_empty() {
        return false;
    }
    if let Some(rest) = suffix.strip_prefix('p') {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
    } else {
        suffix.chars().all(|c| c.is_ascii_digit())
    }
}

#[cfg(target_os = "linux")]
fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(target_os = "linux")]
fn build_by_path_map() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let Ok(entries) = std::fs::read_dir("/dev/disk/by-path") else {
        return map;
    };
    for entry in entries.flatten() {
        let link_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.contains("-part") {
            continue;
        }
        if let Ok(target) = std::fs::canonicalize(&link_path) {
            let target_str = target.to_string_lossy().into_owned();
            map.insert(target_str, link_path.to_string_lossy().into_owned());
        }
    }
    map
}

#[cfg(target_os = "linux")]
fn parse_proc_mounts() -> Vec<(String, Mountpoint)> {
    let Ok(contents) = std::fs::read_to_string("/proc/mounts") else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for line in contents.lines() {
        let mut parts = line.split_whitespace();
        let Some(dev) = parts.next() else {
            continue;
        };
        let Some(mnt) = parts.next() else {
            continue;
        };
        if !dev.starts_with("/dev/") {
            continue;
        }
        result.push((
            unescape_mount_field(dev),
            Mountpoint {
                path: unescape_mount_field(mnt),
                label: None,
            },
        ));
    }
    result
}

#[cfg(target_os = "linux")]
fn unescape_mount_field(s: &str) -> String {
    s.replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

#[cfg(target_os = "linux")]
fn unmount_drive_partitions_linux(device: &str) -> Result<(), EtcherError> {
    // Only attempt unmount if device is actually in /dev/ and mounted
    if !device.starts_with("/dev/") {
        return Ok(());
    }
    let mounts = parse_proc_mounts();
    let mut targets: Vec<String> = mounts
        .into_iter()
        .filter(|(d, _)| d == device || is_partition_of(d, device))
        .map(|(d, _)| d)
        .collect();
    targets.sort();
    targets.dedup();

    for part in targets {
        let status = std::process::Command::new("umount")
            .arg(&part)
            .status();
        let ok = status.map(|s| s.success()).unwrap_or(false);
        if !ok {
            let udisks = std::process::Command::new("udisksctl")
                .args(["unmount", "-b", &part])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !udisks {
                return Err(EtcherError::new(
                    format!("Failed to unmount {part}"),
                    "Close any programs using this drive and try again.",
                )
                .with_device(device));
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn scan_drives_macos(settings: &Settings) -> Result<Vec<Drive>, EtcherError> {
    use std::process::Command;

    let list_out = Command::new("diskutil")
        .args(["list"])
        .output()
        .map_err(|e| EtcherError::from_io_error(&e, Some("diskutil")))?;
    if !list_out.status.success() {
        return Ok(Vec::new());
    }

    let list_text = String::from_utf8_lossy(&list_out.stdout);
    let mut disk_devs = Vec::new();
    for line in list_text.lines() {
        let trimmed = line.trim();
        if let Some(dev) = trimmed.split_whitespace().next()
            && dev.starts_with("/dev/disk")
            && !disk_devs.iter().any(|d| d == dev)
        {
            disk_devs.push(dev.to_string());
        }
    }

    let mut drives = Vec::new();
    for dev in disk_devs {
        if settings.drive_blacklist.iter().any(|b| b == &dev) {
            continue;
        }
        let Ok(info_out) = Command::new("diskutil").args(["info", &dev]).output() else {
            continue;
        };
        if !info_out.status.success() {
            continue;
        }
        let info = String::from_utf8_lossy(&info_out.stdout);

        let field = |key: &str| -> Option<String> {
            info.lines().find_map(|l| {
                let (k, v) = l.split_once(':')?;
                if k.trim().eq_ignore_ascii_case(key) {
                    Some(v.trim().to_string())
                } else {
                    None
                }
            })
        };

        let is_whole = field("Whole")
            .map(|v| v.eq_ignore_ascii_case("Yes"))
            .unwrap_or(false);
        if !is_whole {
            continue;
        }

        let size = field("Disk Size").and_then(|v| {
            let start = v.find('(')?;
            let rest = &v[start + 1..];
            let num_str = rest.split_whitespace().next()?;
            num_str.parse::<u64>().ok()
        });
        if size.unwrap_or(0) == 0 {
            continue;
        }

        let protocol = field("Protocol").filter(|s| !s.is_empty());
        let proto_upper = protocol.as_deref().unwrap_or_default().to_ascii_uppercase();
        let is_usb = proto_upper == "USB";
        let is_card = proto_upper.contains("SECURE DIGITAL") || proto_upper == "SD";
        let is_removable = field("Removable Media")
            .map(|v| v.eq_ignore_ascii_case("Removable") || v.eq_ignore_ascii_case("Yes"))
            .unwrap_or(is_usb || is_card);
        let is_virtual = field("Virtual")
            .map(|v| v.eq_ignore_ascii_case("Yes"))
            .unwrap_or(false);
        let is_read_only = field("Read-Only Media")
            .map(|v| v.eq_ignore_ascii_case("Yes"))
            .unwrap_or(false);
        let is_internal = field("Device Location")
            .map(|v| v.eq_ignore_ascii_case("Internal"))
            .unwrap_or(false);
        let is_system = is_internal || (!is_removable && !is_virtual);

        let partition_table_type = match field("Content (IOContent)").as_deref() {
            Some("GUID_partition_scheme") => Some("gpt".to_string()),
            Some("FDisk_partition_scheme") => Some("mbr".to_string()),
            _ => None,
        };

        let description = field("Device / Media Name")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| dev.clone());
        let raw = dev.replacen("/dev/disk", "/dev/rdisk", 1);

        drives.push(Drive {
            device: dev.clone(),
            raw,
            device_path: field("Device Tree Path"),
            display_name: dev,
            description,
            size,
            is_removable,
            is_system,
            is_read_only,
            is_usb,
            is_card,
            is_virtual,
            bus_type: protocol,
            mountpoints: Vec::new(),
            partition_table_type,
            disabled: is_read_only,
            kind: DriveKind::BlockDevice,
        });
    }

    Ok(drives)
}

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(target_os = "windows")]
pub(crate) fn parse_windows_disk_number(device: &str) -> Option<u32> {
    let lower = device.trim().to_ascii_lowercase();
    lower.strip_prefix(r"\\.\physicaldrive")?.parse::<u32>().ok()
}

#[cfg(target_os = "windows")]
fn unmount_drive_partitions_windows(device: &str) -> Result<(), EtcherError> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let Some(disk_num) = parse_windows_disk_number(device) else {
        return Ok(());
    };

    let mut child = Command::new("diskpart.exe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| EtcherError::from_io_error(&e, Some("diskpart.exe")))?;

    if let Some(mut stdin) = child.stdin.take() {
        let script =
            format!("select disk {disk_num}\r\nattributes disk clear readonly\r\nclean\r\nexit\r\n");
        let _ = stdin.write_all(script.as_bytes());
    }

    let output = child
        .wait_with_output()
        .map_err(|e| EtcherError::from_io_error(&e, Some("diskpart.exe")))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        return Err(EtcherError::new(
            format!("Failed to clean disk {device}"),
            detail,
        )
        .with_device(device));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn scan_drives_windows(settings: &Settings) -> Result<Vec<Drive>, EtcherError> {
    use serde::Deserialize;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PsMount {
        path: String,
        label: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PsDisk {
        number: u32,
        friendly_name: Option<String>,
        size: Option<u64>,
        bus_type: Option<String>,
        is_system: Option<bool>,
        is_boot: Option<bool>,
        is_read_only: Option<bool>,
        partition_style: Option<String>,
        location: Option<String>,
        mounts: Option<Vec<PsMount>>,
    }

    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$disks = @(Get-Disk | ForEach-Object {
    $d = $_
    $mounts = @(Get-Partition -DiskNumber $d.Number | ForEach-Object {
        $v = Get-Volume -Partition $_
        if ($_.DriveLetter) {
            [PSCustomObject]@{
                Path = "$($_.DriveLetter):\"
                Label = if ($v -and $v.FileSystemLabel) { [string]$v.FileSystemLabel } else { $null }
            }
        }
    })
    [PSCustomObject]@{
        Number = [uint32]$d.Number
        FriendlyName = [string]$d.FriendlyName
        Size = [uint64]$d.Size
        BusType = [string]$d.BusType
        IsSystem = [bool]$d.IsSystem
        IsBoot = [bool]$d.IsBoot
        IsReadOnly = [bool]$d.IsReadOnly
        PartitionStyle = [string]$d.PartitionStyle
        Location = [string]$d.Location
        Mounts = $mounts
    }
})
ConvertTo-Json -InputObject $disks -Compress -Depth 4
"#;

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| EtcherError::from_io_error(&e, Some("powershell.exe")))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let raw_json = String::from_utf8_lossy(&output.stdout);
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let parsed: Vec<PsDisk> = serde_json::from_str(trimmed).unwrap_or_default();
    let mut drives = Vec::new();

    for d in parsed {
        let size = d.size.unwrap_or(0);
        if size == 0 {
            continue;
        }
        let device = format!(r"\\.\PhysicalDrive{}", d.number);
        if settings
            .drive_blacklist
            .iter()
            .any(|b| b.eq_ignore_ascii_case(&device))
        {
            continue;
        }

        let bus_type = d
            .bus_type
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("Unknown"))
            .map(ToString::to_string);
        let bus_upper = bus_type
            .as_deref()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let is_usb = bus_upper == "USB";
        let is_card = bus_upper == "SD" || bus_upper == "MMC";
        let is_removable = is_usb || is_card;
        let is_virtual = matches!(
            bus_upper.as_str(),
            "FILE BACKED VIRTUAL" | "SPACES" | "ISCSI" | "VIRTUAL"
        );
        let is_system =
            d.is_system.unwrap_or(false) || d.is_boot.unwrap_or(false) || (!is_removable && !is_virtual);
        let is_read_only = d.is_read_only.unwrap_or(false);
        let partition_table_type = match d.partition_style.as_deref().map(str::trim) {
            Some(s) if s.eq_ignore_ascii_case("MBR") => Some("mbr".to_string()),
            Some(s) if s.eq_ignore_ascii_case("GPT") => Some("gpt".to_string()),
            _ => None,
        };
        let description = d
            .friendly_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("PhysicalDrive {}", d.number));
        let device_path = d
            .location
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);
        let mountpoints = d
            .mounts
            .unwrap_or_default()
            .into_iter()
            .map(|m| Mountpoint {
                path: m.path,
                label: m.label,
            })
            .collect();

        drives.push(Drive {
            device: device.clone(),
            raw: device.clone(),
            device_path,
            display_name: device,
            description,
            size: Some(size),
            is_removable,
            is_system,
            is_read_only,
            is_usb,
            is_card,
            is_virtual,
            bus_type,
            mountpoints,
            partition_table_type,
            disabled: is_read_only,
            kind: DriveKind::BlockDevice,
        });
    }

    Ok(drives)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_drives_runs_cleanly() {
        let settings = Settings::default();
        let drives = scan_drives(&settings).expect("scan_drives should succeed on Linux");
        for d in &drives {
            assert!(!d.device.is_empty());
            assert!(d.size.unwrap_or(0) > 0);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_partition_matching() {
        assert!(is_partition_of("/dev/sda1", "/dev/sda"));
        assert!(is_partition_of("/dev/nvme0n1p2", "/dev/nvme0n1"));
        assert!(is_partition_of("/dev/mmcblk0p1", "/dev/mmcblk0"));
        assert!(!is_partition_of("/dev/sda", "/dev/sda"));
        assert!(!is_partition_of("/dev/sdb1", "/dev/sda"));
    }
}
