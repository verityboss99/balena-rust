use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::errors::EtcherError;

pub const DEFAULT_WIDTH: f32 = 800.0;
pub const DEFAULT_HEIGHT: f32 = 480.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub error_reporting: bool,
    pub updates_enabled: bool,
    pub desktop_notifications: bool,
    pub auto_blockmapping: bool,
    pub decompress_first: bool,
    pub fullscreen: bool,
    pub disable_external_links: bool,
    pub auto_select_all_drives: bool,
    pub drives_order: Vec<String>,
    pub drive_blacklist: Vec<String>,
    pub analytics_alert_visible: bool,
    pub recent_url_images: Vec<String>,
    pub language: Option<String>,
    pub leds_mapping: HashMap<String, [String; 3]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            error_reporting: true,
            updates_enabled: false,
            desktop_notifications: true,
            auto_blockmapping: true,
            decompress_first: true,
            fullscreen: false,
            disable_external_links: false,
            auto_select_all_drives: false,
            drives_order: Vec::new(),
            drive_blacklist: Vec::new(),
            analytics_alert_visible: true,
            recent_url_images: Vec::new(),
            language: None,
            leds_mapping: HashMap::new(),
        }
    }
}

impl Settings {
    pub fn config_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("balena-etcher").join("config.json")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::config_path())
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), EtcherError> {
        self.save_to(&Self::config_path())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), EtcherError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| EtcherError::from_io_error(&e, parent.to_str()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| EtcherError::new("Failed to serialize settings", e.to_string()))?;
        std::fs::write(path, json).map_err(|e| EtcherError::from_io_error(&e, path.to_str()))
    }

    pub fn push_recent_url(&mut self, new_url: &str) {
        let mut urls = self.recent_url_images.clone();
        urls.push(new_url.to_string());
        self.recent_url_images = normalize_recent_urls(&urls);
    }
}

pub fn normalize_recent_urls(urls: &[String]) -> Vec<String> {
    let mut valid = Vec::new();
    for raw in urls {
        let trimmed = raw.trim();
        let (scheme, rest) = if trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("http://") {
            ("http://", &trimmed[7..])
        } else if trimmed.len() >= 8 && trimmed[..8].eq_ignore_ascii_case("https://") {
            ("https://", &trimmed[8..])
        } else {
            continue;
        };
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        if host.is_empty()
            || trimmed
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            continue;
        }
        let href = format!("{scheme}{rest}");
        if !valid.contains(&href) {
            valid.push(href);
        }
    }
    if valid.len() > 5 {
        valid.split_off(valid.len() - 5)
    } else {
        valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut s = Settings {
            auto_blockmapping: false,
            ..Settings::default()
        };
        s.push_recent_url("https://example.com/a.img");
        s.push_recent_url("ftp://ignored.example.com/b.img");
        s.save_to(&path).unwrap();

        let loaded = Settings::load_from(&path);
        assert!(!loaded.auto_blockmapping);
        assert_eq!(
            loaded.recent_url_images,
            vec!["https://example.com/a.img".to_string()]
        );
    }

    #[test]
    fn test_normalize_recent_urls_limit_and_dedup() {
        let input: Vec<String> = (1..=7)
            .map(|i| format!("https://example.com/img{i}.iso"))
            .chain(std::iter::once("https://example.com/img3.iso".to_string()))
            .collect();
        let norm = normalize_recent_urls(&input);
        assert_eq!(norm.len(), 5);
        assert_eq!(norm[0], "https://example.com/img3.iso");
        assert_eq!(norm[4], "https://example.com/img7.iso");
    }
}
