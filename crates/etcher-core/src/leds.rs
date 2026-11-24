use std::collections::HashMap;
use std::path::PathBuf;

use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedStep {
    Main,
    Flashing,
    Verifying,
    Finish,
}

#[derive(Debug, Clone)]
pub struct RgbLed {
    pub r_path: PathBuf,
    pub g_path: PathBuf,
    pub b_path: PathBuf,
}

impl RgbLed {
    pub fn new(names: &[String; 3]) -> Self {
        let base = PathBuf::from("/sys/class/leds");
        Self {
            r_path: base.join(&names[0]).join("brightness"),
            g_path: base.join(&names[1]).join("brightness"),
            b_path: base.join(&names[2]).join("brightness"),
        }
    }

    pub fn set_rgb(&self, r: u8, g: u8, b: u8) {
        let _ = std::fs::write(&self.r_path, r.to_string());
        let _ = std::fs::write(&self.g_path, g.to_string());
        let _ = std::fs::write(&self.b_path, b.to_string());
    }
}

#[derive(Debug, Default)]
pub struct LedController {
    leds: HashMap<String, RgbLed>,
}

impl LedController {
    pub fn from_settings(settings: &Settings) -> Self {
        let mut leds = HashMap::new();
        for (drive_path, names) in &settings.leds_mapping {
            leds.insert(
                format!("/dev/disk/by-path/{drive_path}"),
                RgbLed::new(names),
            );
        }
        Self { leds }
    }

    pub fn is_active(&self) -> bool {
        !self.leds.is_empty()
    }

    pub fn update(
        &self,
        step: LedStep,
        source_drive_path: Option<&str>,
        selected_paths: &[String],
        failed_paths: &[String],
    ) {
        if self.leds.is_empty() {
            return;
        }
        for (path, led) in &self.leds {
            if source_drive_path == Some(path.as_str()) {
                led.set_rgb(0, 0, 255);
                continue;
            }
            let is_failed = failed_paths.iter().any(|p| p == path);
            let is_selected = selected_paths.iter().any(|p| p == path);
            if !is_selected && !is_failed {
                led.set_rgb(0, 0, 0);
                continue;
            }
            match step {
                LedStep::Main => led.set_rgb(255, 255, 255),
                LedStep::Flashing => {
                    if is_failed {
                        led.set_rgb(255, 0, 0);
                    } else {
                        led.set_rgb(218, 96, 255);
                    }
                }
                LedStep::Verifying => {
                    if is_failed {
                        led.set_rgb(255, 0, 0);
                    } else {
                        led.set_rgb(26, 193, 53);
                    }
                }
                LedStep::Finish => {
                    if is_failed {
                        led.set_rgb(255, 0, 0);
                    } else {
                        led.set_rgb(26, 193, 53);
                    }
                }
            }
        }
    }
}
