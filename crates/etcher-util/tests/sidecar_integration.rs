use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use flate2::Compression;
use flate2::write::GzEncoder;

use etcher_core::ipc::{GuiToUtil, ProbeRequest, UtilToGui, generate_auth_token};
use etcher_core::source::probe_source_metadata;
use etcher_core::types::{Drive, DriveKind, SourceMetadata, SourceType, WriteOptions};

fn find_built_util() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_etcher-util"))
}

#[test]
fn test_etcher_util_sidecar_flashes_and_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let payload: Vec<u8> = (0..(1024 * 1024 + 4096))
        .map(|i| ((i * 17 + 5) % 253) as u8)
        .collect();
    let gz_path = dir.path().join("sidecar-test.img.gz");
    {
        let mut enc = GzEncoder::new(std::fs::File::create(&gz_path).unwrap(), Compression::fast());
        enc.write_all(&payload).unwrap();
        enc.finish().unwrap();
    }

    let meta = probe_source_metadata(gz_path.to_str().unwrap(), SourceType::File, None).unwrap();

    let t1 = tempfile::NamedTempFile::new().unwrap();
    let t2 = tempfile::NamedTempFile::new().unwrap();
    t1.as_file().set_len(2 * 1024 * 1024).unwrap();
    t2.as_file().set_len(2 * 1024 * 1024).unwrap();

    let make_drive = |p: &std::path::Path| Drive {
        device: p.to_string_lossy().into_owned(),
        raw: p.to_string_lossy().into_owned(),
        device_path: None,
        display_name: p.to_string_lossy().into_owned(),
        description: "Sidecar Test Target".to_string(),
        size: Some(2 * 1024 * 1024),
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
        kind: DriveKind::BlockDevice,
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let token = generate_auth_token();

    let util_bin = find_built_util();
    assert!(
        util_bin.exists(),
        "etcher-util binary must exist at {}",
        util_bin.display()
    );

    let mut child = Command::new(&util_bin)
        .arg("--host=127.0.0.1")
        .arg(format!("--port={port}"))
        .arg(format!("--token={token}"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let (stream, _) = listener.accept().unwrap();
    let mut writer_stream = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);

    // First message must be Auth
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let auth_msg: GuiToUtil = serde_json::from_str(line.trim()).unwrap();
    match auth_msg {
        GuiToUtil::Auth { token: got } => assert_eq!(got, token),
        other => panic!("expected Auth, got {other:?}"),
    }

    // Second message must be Ready
    line.clear();
    reader.read_line(&mut line).unwrap();
    let ready_msg: UtilToGui = serde_json::from_str(line.trim()).unwrap();
    assert!(matches!(ready_msg, UtilToGui::Ready));

    // Send Write command
    let opts = WriteOptions {
        image: meta,
        destinations: vec![make_drive(t1.path()), make_drive(t2.path())],
        auto_blockmapping: false,
        decompress_first: true,
        verify: true,
    };
    writeln!(
        writer_stream,
        "{}",
        serde_json::to_string(&GuiToUtil::Write(Box::new(opts))).unwrap()
    )
    .unwrap();
    writer_stream.flush().unwrap();

    let mut saw_state = false;
    let mut final_done = None;
    loop {
        line.clear();
        let n = reader.read_line(&mut line).unwrap();
        if n == 0 {
            break;
        }
        let msg: UtilToGui = serde_json::from_str(line.trim()).unwrap();
        match msg {
            UtilToGui::State(_) => saw_state = true,
            UtilToGui::Done(res) => {
                final_done = Some(res);
                break;
            }
            UtilToGui::Error(e) => panic!("sidecar error: {e}"),
            UtilToGui::Fail(f) => panic!("sidecar target fail: {f:?}"),
            _ => {}
        }
    }

    let status = child.wait().unwrap();
    assert!(status.success(), "etcher-util exited with {status}");
    assert!(saw_state, "expected at least one State message");
    let done = final_done.expect("expected Done message");
    assert_eq!(done.successful_devices, 2);
    assert_eq!(done.failed_devices, 0);

    let mut out1 = vec![0u8; payload.len()];
    let mut out2 = vec![0u8; payload.len()];
    t1.reopen().unwrap().read_exact(&mut out1).unwrap();
    t2.reopen().unwrap().read_exact(&mut out2).unwrap();
    assert_eq!(out1, payload);
    assert_eq!(out2, payload);
}

#[test]
fn test_etcher_util_probe_mode() {
    let dir = tempfile::tempdir().unwrap();
    let payload: Vec<u8> = (0..(128 * 1024))
        .map(|i| ((i * 31 + 7) % 251) as u8)
        .collect();
    let gz_path = dir.path().join("probe-test.img.gz");
    {
        let mut enc = GzEncoder::new(std::fs::File::create(&gz_path).unwrap(), Compression::fast());
        enc.write_all(&payload).unwrap();
        enc.finish().unwrap();
    }

    let util_bin = find_built_util();
    let mut child = Command::new(&util_bin)
        .arg("--probe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let req = ProbeRequest {
        selected: gz_path.to_string_lossy().into_owned(),
        source_type: SourceType::File,
        auth: None,
    };
    {
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(serde_json::to_string(&req).unwrap().as_bytes())
            .unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "etcher-util --probe exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let res: Result<SourceMetadata, etcher_core::errors::EtcherError> =
        serde_json::from_slice(&output.stdout).unwrap();
    let meta = res.unwrap();
    assert!(meta.is_compressed);
    assert_eq!(meta.archive_extension.as_deref(), Some("gz"));
    assert_eq!(meta.size, Some(payload.len() as u64));
}
