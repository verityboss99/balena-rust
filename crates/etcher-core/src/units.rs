use crate::errors::EtcherError;

pub const MEGABYTE_TO_BYTE_RATIO: f64 = 1_000_000.0;

pub fn bytes_to_megabytes(bytes: f64) -> f64 {
    bytes / MEGABYTE_TO_BYTE_RATIO
}

pub fn is_valid_percentage(percentage: f64) -> bool {
    percentage.is_finite() && (0.0..=100.0).contains(&percentage)
}

pub fn percentage_to_float(percentage: f64) -> Result<f64, EtcherError> {
    if !is_valid_percentage(percentage) {
        return Err(EtcherError::new(
            format!("Invalid percentage: {percentage}"),
            "Percentage must be between 0 and 100",
        ));
    }
    Ok(percentage / 100.0)
}

/// Formats bytes using SI decimal units (1000-based) matching npm `pretty-bytes`.
pub fn pretty_bytes(bytes: u64) -> String {
    const UNITS: [&str; 9] = ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let b = bytes as f64;
    let exp = (b.log10() / 3.0).floor() as usize;
    let exp = exp.min(UNITS.len() - 1);
    let val = b / 1000f64.powi(exp as i32);
    // Match pretty-bytes precision (~3 significant digits, trimming trailing zeros)
    let rounded = if val >= 100.0 {
        format!("{val:.0}")
    } else if val >= 10.0 {
        trim_decimal(format!("{val:.1}"))
    } else {
        trim_decimal(format!("{val:.2}"))
    };
    format!("{rounded} {}", UNITS[exp])
}

fn trim_decimal(s: String) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// Truncate text from the middle with an ellipsis (`…`), char-safe for Unicode.
pub fn middle_ellipsis(input: &str, limit: usize) -> Result<String, &'static str> {
    if limit < 3 {
        return Err("middleEllipsis: Limit should be at least 3");
    }
    let chars: Vec<char> = input.chars().collect();
    if chars.len() <= limit {
        return Ok(input.to_string());
    }
    let side = (limit - 1) / 2;
    let left: String = chars[..side].iter().collect();
    let right: String = chars[chars.len() - side..].iter().collect();
    Ok(format!("{left}…{right}"))
}

pub fn format_seconds(total_seconds: f64) -> String {
    if !total_seconds.is_finite() || total_seconds < 0.0 {
        return String::new();
    }
    let total = total_seconds.floor() as u64;
    let minutes = total / 60;
    let seconds = total % 60;
    format!("{minutes}m{seconds}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_megabytes() {
        assert!((bytes_to_megabytes(1_000_000.0) - 1.0).abs() < f64::EPSILON);
        assert!((bytes_to_megabytes(2_500_000.0) - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pretty_bytes() {
        assert_eq!(pretty_bytes(0), "0 B");
        assert_eq!(pretty_bytes(999), "999 B");
        assert_eq!(pretty_bytes(1000), "1 kB");
        assert_eq!(pretty_bytes(1_500_000), "1.5 MB");
        assert_eq!(pretty_bytes(32_000_000_000), "32 GB");
    }

    #[test]
    fn test_middle_ellipsis_parity() {
        assert_eq!(
            middle_ellipsis("No", 2).unwrap_err(),
            "middleEllipsis: Limit should be at least 3"
        );
        let alphabet = "abcdefghijklmnopqrstuvwxyz";
        assert_eq!(middle_ellipsis(alphabet, 3).unwrap().chars().count(), 3);
        assert_eq!(middle_ellipsis(alphabet, 4).unwrap().chars().count(), 3);
        assert_eq!(middle_ellipsis(alphabet, 5).unwrap().chars().count(), 5);
        assert_eq!(middle_ellipsis(alphabet, 6).unwrap().chars().count(), 5);
        assert_eq!(middle_ellipsis("Hello", 10).unwrap(), "Hello");
        // Unicode safety
        assert_eq!(middle_ellipsis("你好世界测试文件名.img", 7).unwrap(), "你好世…img");
    }

    #[test]
    fn test_format_seconds() {
        assert_eq!(format_seconds(0.0), "0m0s");
        assert_eq!(format_seconds(75.9), "1m15s");
        assert_eq!(format_seconds(f64::NAN), "");
    }
}
