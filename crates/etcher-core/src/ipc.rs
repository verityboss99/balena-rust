use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::errors::EtcherError;
use crate::types::{
    Authentication, FlashError, FlashResults, ProgressState, SourceType, WriteOptions,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeRequest {
    pub selected: String,
    pub source_type: SourceType,
    pub auth: Option<Authentication>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum UtilToGui {
    Ready,
    Log { message: String },
    State(ProgressState),
    Fail(FlashError),
    Done(FlashResults),
    Abort,
    Skip,
    Error(EtcherError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum GuiToUtil {
    Auth { token: String },
    Write(Box<WriteOptions>),
    Cancel,
    Skip,
    Heartbeat,
    Terminate,
}

pub fn generate_auth_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

pub fn is_elevated() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("fltmc")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

pub fn find_util_binary() -> Result<PathBuf, EtcherError> {
    let exe_name = if cfg!(windows) {
        "etcher-util.exe"
    } else {
        "etcher-util"
    };
    if let Ok(curr) = std::env::current_exe()
        && let Some(dir) = curr.parent()
    {
        let sibling = dir.join(exe_name);
        if sibling.exists() {
            return Ok(sibling);
        }
        // When running cargo test binaries in target/debug/deps/, check parent dir
        if let Some(parent) = dir.parent() {
            let up = parent.join(exe_name);
            if up.exists() {
                return Ok(up);
            }
        }
    }
    // Fall back to PATH lookup
    Ok(PathBuf::from(exe_name))
}

#[derive(Debug)]
pub enum SpawnOutcome {
    Spawned(Child),
    Cancelled,
}

pub fn spawn_util_process(
    util_path: &Path,
    host: &str,
    port: u16,
    token: &str,
    with_privileges: bool,
) -> Result<SpawnOutcome, EtcherError> {
    let args = vec![
        format!("--host={host}"),
        format!("--port={port}"),
        format!("--token={token}"),
    ];

    if !with_privileges || is_elevated() {
        let mut cmd = Command::new(util_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let child = cmd
            .spawn()
            .map_err(|e| EtcherError::from_io_error(&e, util_path.to_str()))?;
        return Ok(SpawnOutcome::Spawned(child));
    }

    #[cfg(target_os = "linux")]
    {
        spawn_privileged_linux(util_path, &args)
    }
    #[cfg(target_os = "macos")]
    {
        spawn_privileged_macos(util_path, &args)
    }
    #[cfg(target_os = "windows")]
    {
        spawn_privileged_windows(util_path, &args)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(EtcherError::user_error(
            "Privilege elevation unsupported",
            "Run Etcher as root to write to block devices.",
        ))
    }
}

#[cfg(target_os = "linux")]
fn spawn_privileged_linux(util_path: &Path, args: &[String]) -> Result<SpawnOutcome, EtcherError> {
    let elevator = ["/usr/bin/pkexec", "/usr/bin/kdesudo"]
        .into_iter()
        .find(|p| Path::new(p).exists())
        .ok_or_else(|| {
            EtcherError::user_error(
                "No polkit authentication agent found",
                "Please install pkexec / a polkit authentication agent to continue.",
            )
        })?;

    let mut cmd = Command::new(elevator);
    if elevator.ends_with("pkexec") {
        cmd.arg("--disable-internal-agent");
    }
    cmd.arg(util_path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| EtcherError::from_io_error(&e, Some(elevator)))?;
    Ok(SpawnOutcome::Spawned(child))
}

#[cfg(target_os = "macos")]
fn spawn_privileged_macos(util_path: &Path, args: &[String]) -> Result<SpawnOutcome, EtcherError> {
    let joined = std::iter::once(util_path.to_string_lossy().into_owned())
        .chain(args.iter().cloned())
        .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!("do shell script \"{joined}\" with administrator privileges");
    let child = Command::new("osascript")
        .args(["-e", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| EtcherError::from_io_error(&e, Some("osascript")))?;
    Ok(SpawnOutcome::Spawned(child))
}

#[cfg(target_os = "windows")]
fn spawn_privileged_windows(util_path: &Path, args: &[String]) -> Result<SpawnOutcome, EtcherError> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let file_arg = format!("'{}'", util_path.to_string_lossy().replace('\'', "''"));
    let arg_list = format!("'{}'", args.join(" ").replace('\'', "''"));
    let script = format!(
        "try {{ $p = Start-Process -FilePath {file_arg} -ArgumentList {arg_list} -WindowStyle Hidden -Verb RunAs -Wait -PassThru; exit $p.ExitCode }} catch {{ exit 126 }}"
    );
    let child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| EtcherError::from_io_error(&e, Some("powershell.exe")))?;
    Ok(SpawnOutcome::Spawned(child))
}
