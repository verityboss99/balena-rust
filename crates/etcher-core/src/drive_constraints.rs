use crate::i18n::{Locale, messages};
use crate::types::{Drive, SourceMetadata};

pub const UNKNOWN_SIZE: u64 = 0;
pub const LARGE_DRIVE_SIZE: u64 = 128_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityStatusKind {
    Warning = 1,
    Error = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityStatusCode {
    Locked,
    TooSmall,
    System,
    Large,
    ContainsImage,
    SizeNotRecommended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveStatus {
    pub kind: CompatibilityStatusKind,
    pub code: CompatibilityStatusCode,
    pub message: String,
}

pub fn is_system_drive(drive: &Drive) -> bool {
    drive.is_system
}

/// Checks if `child` path is strictly inside `parent` directory/mountpoint.
/// Handles both Unix `/` and Windows `\` separators and root `/`.
pub fn path_is_inside(child: &str, parent: &str) -> bool {
    if parent.is_empty() || child.is_empty() {
        return false;
    }
    let norm = |p: &str| -> String { p.replace('\\', "/") };
    let c = norm(child);
    let mut p = norm(parent);
    if p == "/" {
        return c.starts_with('/');
    }
    while p.ends_with('/') {
        p.pop();
    }
    if p.is_empty() {
        return false;
    }
    if c == p {
        return false;
    }
    c.starts_with(&format!("{p}/"))
}

pub fn is_source_drive(drive: &Drive, selection: Option<&SourceMetadata>) -> bool {
    let Some(sel) = selection else {
        return false;
    };
    if let Some(sel_drive) = &sel.drive {
        return sel_drive.device == drive.device;
    }
    if !sel.path.is_empty() {
        return drive
            .mountpoints
            .iter()
            .any(|mp| path_is_inside(&sel.path, &mp.path));
    }
    false
}

pub fn is_drive_large_enough(drive: &Drive, image: Option<&SourceMetadata>) -> bool {
    let drive_size = drive.size.unwrap_or(UNKNOWN_SIZE);
    let Some(img) = image else {
        return true;
    };
    if img.is_size_estimated {
        if drive_size < img.compressed_size.unwrap_or(UNKNOWN_SIZE) {
            return false;
        }
        return true;
    }
    drive_size >= img.size.unwrap_or(UNKNOWN_SIZE)
}

pub fn is_drive_valid(drive: &Drive, image: Option<&SourceMetadata>, write: bool) -> bool {
    !write
        || (!drive.disabled
            && is_drive_large_enough(drive, image)
            && !is_source_drive(drive, image))
}

pub fn is_drive_size_recommended(drive: &Drive, image: Option<&SourceMetadata>) -> bool {
    let drive_size = drive.size.unwrap_or(UNKNOWN_SIZE);
    drive_size
        >= image
            .and_then(|i| i.recommended_drive_size)
            .unwrap_or(UNKNOWN_SIZE)
}

pub fn is_drive_size_large(drive: &Drive) -> bool {
    drive.size.unwrap_or(UNKNOWN_SIZE) > LARGE_DRIVE_SIZE
}

pub fn get_drive_image_compatibility_statuses(
    drive: &Drive,
    image: Option<&SourceMetadata>,
    write: bool,
    locale: Locale,
) -> Vec<DriveStatus> {
    let m = messages(locale);
    let mut status_list = Vec::new();

    if drive.is_read_only && write {
        status_list.push(DriveStatus {
            kind: CompatibilityStatusKind::Error,
            code: CompatibilityStatusCode::Locked,
            message: m.compat_locked.to_string(),
        });
    }

    if drive.size.is_some() && !is_drive_large_enough(drive, image) {
        status_list.push(DriveStatus {
            kind: CompatibilityStatusKind::Error,
            code: CompatibilityStatusCode::TooSmall,
            message: m.compat_too_small.to_string(),
        });
    } else {
        if is_system_drive(drive) {
            status_list.push(DriveStatus {
                kind: CompatibilityStatusKind::Warning,
                code: CompatibilityStatusCode::System,
                message: m.compat_system.to_string(),
            });
        } else if is_drive_size_large(drive) {
            status_list.push(DriveStatus {
                kind: CompatibilityStatusKind::Warning,
                code: CompatibilityStatusCode::Large,
                message: m.compat_large_drive.to_string(),
            });
        }

        if is_source_drive(drive, image) {
            status_list.push(DriveStatus {
                kind: CompatibilityStatusKind::Error,
                code: CompatibilityStatusCode::ContainsImage,
                message: m.compat_contains_image.to_string(),
            });
        }

        if image.is_some() && !is_drive_size_recommended(drive, image) {
            status_list.push(DriveStatus {
                kind: CompatibilityStatusKind::Warning,
                code: CompatibilityStatusCode::SizeNotRecommended,
                message: m.compat_size_not_recommended.to_string(),
            });
        }
    }

    status_list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Mountpoint, SourceType};

    fn base_drive() -> Drive {
        Drive {
            device: "/dev/disk2".to_string(),
            raw: "/dev/rdisk2".to_string(),
            device_path: None,
            display_name: "/dev/disk2".to_string(),
            description: "USB Drive".to_string(),
            size: Some(4_000_000_000),
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
            kind: Default::default(),
        }
    }

    fn base_image(path: &str, size: u64) -> SourceMetadata {
        SourceMetadata {
            path: path.to_string(),
            name: None,
            display_name: "rpi.img".to_string(),
            description: "rpi.img".to_string(),
            size: Some(size),
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
        }
    }

    #[test]
    fn test_is_source_drive_unix_and_windows() {
        let mut d = base_drive();
        d.mountpoints = vec![Mountpoint {
            path: "/Volumes/fo".to_string(),
            label: None,
        }];
        let img = base_image("/Volumes/foo/image.img", 100);
        assert!(!is_source_drive(&d, Some(&img)));

        d.mountpoints = vec![Mountpoint {
            path: "/".to_string(),
            label: None,
        }];
        assert!(is_source_drive(&d, Some(&base_image("/image.img", 100))));

        d.mountpoints = vec![Mountpoint {
            path: "E:".to_string(),
            label: Some("label".to_string()),
        }];
        assert!(is_source_drive(
            &d,
            Some(&base_image("E:\\foo\\bar\\image.img", 100))
        ));
        assert!(!is_source_drive(
            &d,
            Some(&base_image("G:\\image.img", 100))
        ));
    }

    #[test]
    fn test_is_drive_large_enough_estimated() {
        let mut d = base_drive();
        d.size = Some(1_000_000_000);

        let mut img = base_image("/tmp/rpi.img", 1_000_000_001);
        assert!(!is_drive_large_enough(&d, Some(&img)));

        img.is_size_estimated = true;
        img.compressed_size = Some(999_999_999);
        assert!(is_drive_large_enough(&d, Some(&img)));

        img.compressed_size = Some(1_000_000_001);
        assert!(!is_drive_large_enough(&d, Some(&img)));
    }

    #[test]
    fn test_compatibility_statuses_order() {
        let mut d = base_drive();
        d.is_read_only = true;
        d.is_system = true;
        d.size = Some(200_000_000_000); // > 128 GB
        d.mountpoints = vec![Mountpoint {
            path: "/mnt/usb".to_string(),
            label: None,
        }];

        let mut img = base_image("/mnt/usb/test.img", 1_000_000_000);
        img.recommended_drive_size = Some(300_000_000_000);

        let statuses = get_drive_image_compatibility_statuses(&d, Some(&img), true, Locale::En);
        let codes: Vec<_> = statuses.iter().map(|s| s.code).collect();
        assert_eq!(
            codes,
            vec![
                CompatibilityStatusCode::Locked,
                CompatibilityStatusCode::System,
                CompatibilityStatusCode::ContainsImage,
                CompatibilityStatusCode::SizeNotRecommended,
            ]
        );
    }
}
