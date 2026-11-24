use std::path::Path;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "bin",
    "bz2",
    "dmg",
    "dsk",
    "etch",
    "gz",
    "hddimg",
    "img",
    "iso",
    "raw",
    "rpi-sdimg",
    "sdcard",
    "vhd",
    "wic",
    "xz",
    "zip",
    "zst",
];

pub fn looks_like_windows_image(image_path: &str) -> bool {
    let base = Path::new(image_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(image_path)
        .to_ascii_lowercase();
    base.contains("windows")
        || base.contains("win7")
        || base.contains("win8")
        || base.contains("win10")
        || base.contains("win11")
        || base.contains("winxp")
}

pub fn normalize_image_url(img_path: &str) -> String {
    let trimmed = img_path.trim().trim_end_matches('/');
    let without_scheme = trimmed.strip_prefix("etcher://").unwrap_or(trimmed);
    let decoded = percent_decode(without_scheme);
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&decoded)
        && let Some(url) = val.get("url").and_then(|v| v.as_str())
    {
        return url.to_string();
    }
    decoded
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(val) = u8::from_str_radix(hex, 16)
        {
            out.push(val);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn is_http_url(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_image_detection() {
        assert!(looks_like_windows_image("/tmp/Win10_22H2_English_x64.iso"));
        assert!(looks_like_windows_image("C:\\ISOs\\windows_server.iso"));
        assert!(looks_like_windows_image("winxp-sp3.img"));
        assert!(!looks_like_windows_image("/tmp/ubuntu-24.04-desktop-amd64.iso"));
        assert!(!looks_like_windows_image("/windows/ubuntu.img"));
    }
}
