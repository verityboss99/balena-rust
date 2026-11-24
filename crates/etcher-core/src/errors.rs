use serde::{Deserialize, Serialize};
use std::fmt;

use crate::i18n::{Locale, messages};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    Enoent,
    Eperm,
    Eacces,
    Enomem,
    Evalidation,
    Eunplugged,
    Eio,
    Enospc,
    Echilddied,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enoent => "ENOENT",
            Self::Eperm => "EPERM",
            Self::Eacces => "EACCES",
            Self::Enomem => "ENOMEM",
            Self::Evalidation => "EVALIDATION",
            Self::Eunplugged => "EUNPLUGGED",
            Self::Eio => "EIO",
            Self::Enospc => "ENOSPC",
            Self::Echilddied => "ECHILDDIED",
        }
    }

    pub fn from_code_str(code: &str) -> Option<Self> {
        match code {
            "ENOENT" => Some(Self::Enoent),
            "EPERM" => Some(Self::Eperm),
            "EACCES" => Some(Self::Eacces),
            "ENOMEM" => Some(Self::Enomem),
            "EVALIDATION" => Some(Self::Evalidation),
            "EUNPLUGGED" => Some(Self::Eunplugged),
            "EIO" => Some(Self::Eio),
            "ENOSPC" => Some(Self::Enospc),
            "ECHILDDIED" => Some(Self::Echilddied),
            _ => None,
        }
    }

    pub fn from_raw_os_error(errno: i32) -> Option<Self> {
        #[cfg(unix)]
        {
            match errno {
                libc::ENOENT => Some(Self::Enoent),
                libc::EPERM => Some(Self::Eperm),
                libc::EACCES => Some(Self::Eacces),
                libc::ENOMEM => Some(Self::Enomem),
                libc::ENOSPC => Some(Self::Enospc),
                libc::ENODEV | libc::ENXIO | libc::ESHUTDOWN => Some(Self::Eunplugged),
                libc::EIO => Some(Self::Eio),
                _ => None,
            }
        }
        #[cfg(windows)]
        {
            match errno {
                2 | 3 | libc::ENOENT => Some(Self::Enoent),
                libc::EPERM => Some(Self::Eperm),
                5 | libc::EACCES => Some(Self::Eacces),
                8 | 14 | libc::ENOMEM => Some(Self::Enomem),
                39 | 112 | libc::ENOSPC => Some(Self::Enospc),
                21 | 1167 | libc::ENODEV | libc::ENXIO => Some(Self::Eunplugged),
                23 | 1117 | libc::EIO => Some(Self::Eio),
                _ => None,
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            match errno {
                libc::ENOENT => Some(Self::Enoent),
                libc::EPERM => Some(Self::Eperm),
                libc::EACCES => Some(Self::Eacces),
                libc::ENOMEM => Some(Self::Enomem),
                libc::ENOSPC => Some(Self::Enospc),
                libc::ENODEV | libc::ENXIO => Some(Self::Eunplugged),
                libc::EIO => Some(Self::Eio),
                _ => None,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EtcherErrorInner {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
    pub report: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EtcherError(pub Box<EtcherErrorInner>);

impl std::ops::Deref for EtcherError {
    type Target = EtcherErrorInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for EtcherError {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl EtcherError {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self(Box::new(EtcherErrorInner {
            title: title.into(),
            description: description.into(),
            code: None,
            path: None,
            device: None,
            report: true,
        }))
    }

    pub fn user_error(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self(Box::new(EtcherErrorInner {
            title: title.into(),
            description: description.into(),
            code: None,
            path: None,
            device: None,
            report: false,
        }))
    }

    pub fn with_code(mut self, code: ErrorCode) -> Self {
        self.0.code = Some(code.as_str().to_string());
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.0.path = Some(path.into());
        self
    }

    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.0.device = Some(device.into());
        self
    }

    pub fn from_io_error(err: &std::io::Error, path: Option<&str>) -> Self {
        let code = err.raw_os_error().and_then(ErrorCode::from_raw_os_error);
        let title = match (code, path) {
            (Some(ErrorCode::Enoent), Some(p)) => format!("No such file or directory: {p}"),
            (Some(ErrorCode::Eperm), _) => {
                "You're not authorized to perform this operation".to_string()
            }
            (Some(ErrorCode::Eacces), _) => {
                "You don't have access to this resource".to_string()
            }
            (Some(ErrorCode::Enomem), _) => "Your system ran out of memory".to_string(),
            _ => err.to_string(),
        };
        let description = match code {
            Some(ErrorCode::Enoent) => "The file you're trying to access doesn't exist".to_string(),
            Some(ErrorCode::Eperm) => {
                "Please ensure you have necessary permissions for this task".to_string()
            }
            Some(ErrorCode::Eacces) => {
                "Please ensure you have necessary permissions to access this resource".to_string()
            }
            Some(ErrorCode::Enomem) => {
                "Please make sure your system has enough available memory for this task"
                    .to_string()
            }
            _ => err.to_string(),
        };
        Self(Box::new(EtcherErrorInner {
            title,
            description,
            code: code.map(|c| c.as_str().to_string()),
            path: path.map(ToString::to_string),
            device: None,
            report: !matches!(
                code,
                Some(ErrorCode::Enoent | ErrorCode::Eperm | ErrorCode::Eacces | ErrorCode::Enospc)
            ),
        }))
    }

    pub fn title(&self) -> &str {
        if !self.0.title.trim().is_empty() {
            &self.0.title
        } else if let Some(code) = &self.0.code {
            code
        } else {
            "An error occurred"
        }
    }

    pub fn description(&self) -> &str {
        if !self.0.description.trim().is_empty() {
            &self.0.description
        } else {
            self.title()
        }
    }
}

impl fmt::Display for EtcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.description.is_empty() || self.description == self.title {
            write!(f, "{}", self.title)
        } else {
            write!(f, "{}: {}", self.title, self.description)
        }
    }
}

impl std::error::Error for EtcherError {}

impl From<std::io::Error> for EtcherError {
    fn from(value: std::io::Error) -> Self {
        Self::from_io_error(&value, None)
    }
}

pub fn error_message_from_code(code: &str, locale: Locale) -> Option<String> {
    let m = messages(locale);
    match ErrorCode::from_code_str(code)? {
        ErrorCode::Evalidation => Some(m.msg_validation.to_string()),
        ErrorCode::Eunplugged => Some(m.msg_unplug.to_string()),
        ErrorCode::Eio => Some(m.msg_cannot_write.to_string()),
        ErrorCode::Enospc => Some(m.msg_no_space.to_string()),
        ErrorCode::Echilddied => Some(m.msg_child_writer_died.to_string()),
        _ => None,
    }
}
