use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use etcher_core::errors::EtcherError;
use etcher_core::ipc::{GuiToUtil, ProbeRequest, UtilToGui};
use etcher_core::source::probe_source_metadata;
use etcher_core::types::SourceMetadata;
use etcher_core::writer::{cleanup_tmp_files, write_and_validate};

struct Args {
    host: String,
    port: u16,
    token: String,
    timeout_ms: u64,
}

fn parse_args() -> Result<Args, String> {
    let mut host = "127.0.0.1".to_string();
    let mut port: Option<u16> = None;
    let mut token: Option<String> = None;
    let mut timeout_ms: u64 = 15_000;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--host=") {
            host = v.to_string();
        } else if let Some(v) = arg.strip_prefix("--port=") {
            port = Some(v.parse().map_err(|e| format!("Invalid port: {e}"))?);
        } else if let Some(v) = arg.strip_prefix("--token=") {
            token = Some(v.to_string());
        } else if let Some(v) = arg.strip_prefix("--timeout-ms=") {
            timeout_ms = v.parse().map_err(|e| format!("Invalid timeout: {e}"))?;
        }
    }

    Ok(Args {
        host,
        port: port.ok_or_else(|| "Missing --port=<PORT>".to_string())?,
        token: token.ok_or_else(|| "Missing --token=<TOKEN>".to_string())?,
        timeout_ms,
    })
}

fn send_msg(writer: &Arc<Mutex<TcpStream>>, msg: &UtilToGui) {
    if let Ok(line) = serde_json::to_string(msg)
        && let Ok(mut stream) = writer.lock()
    {
        let _ = writeln!(stream, "{line}");
        let _ = stream.flush();
    }
}

fn run_probe_mode() -> ExitCode {
    let mut input = String::new();
    let result: Result<SourceMetadata, EtcherError> = match std::io::stdin().read_to_string(&mut input) {
        Ok(_) => match serde_json::from_str::<ProbeRequest>(&input) {
            Ok(req) => probe_source_metadata(&req.selected, req.source_type, req.auth),
            Err(e) => Err(EtcherError::new("Invalid probe request", e.to_string())),
        },
        Err(e) => Err(EtcherError::new("Failed to read probe request", e.to_string())),
    };
    let Ok(json) = serde_json::to_string(&result) else {
        return ExitCode::from(1);
    };
    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "{json}").is_err() || stdout.flush().is_err() {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    if std::env::args().any(|a| a == "--probe") {
        return run_probe_mode();
    }

    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("etcher-util argument error: {e}");
            return ExitCode::from(2);
        }
    };

    let addr = format!("{}:{}", args.host, args.port);
    let stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("etcher-util failed to connect to {addr}: {e}");
            return ExitCode::from(1);
        }
    };

    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("etcher-util failed to clone stream: {e}");
            return ExitCode::from(1);
        }
    };

    let writer = Arc::new(Mutex::new(stream));

    // Authenticate first
    if let Ok(auth_line) = serde_json::to_string(&GuiToUtil::Auth {
        token: args.token.clone(),
    }) && let Ok(mut s) = writer.lock()
        && (writeln!(s, "{auth_line}").is_err() || s.flush().is_err())
    {
        return ExitCode::from(1);
    }

    send_msg(&writer, &UtilToGui::Ready);

    let cancel = Arc::new(AtomicBool::new(false));
    let skip_verify = Arc::new(AtomicBool::new(false));
    let terminate = Arc::new(AtomicBool::new(false));
    let last_heartbeat = Arc::new(Mutex::new(Instant::now()));

    let (write_tx, write_rx) = std::sync::mpsc::channel();

    // Reader thread for GUI commands & heartbeats
    let cancel_r = cancel.clone();
    let skip_r = skip_verify.clone();
    let term_r = terminate.clone();
    let hb_r = last_heartbeat.clone();
    std::thread::spawn(move || {
        let mut br = BufReader::new(reader_stream);
        let mut line = String::new();
        loop {
            line.clear();
            match br.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    cancel_r.store(true, Ordering::Relaxed);
                    term_r.store(true, Ordering::Relaxed);
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(cmd) = serde_json::from_str::<GuiToUtil>(trimmed) {
                        match cmd {
                            GuiToUtil::Auth { .. } => {}
                            GuiToUtil::Heartbeat => {
                                if let Ok(mut t) = hb_r.lock() {
                                    *t = Instant::now();
                                }
                            }
                            GuiToUtil::Cancel => {
                                cancel_r.store(true, Ordering::Relaxed);
                            }
                            GuiToUtil::Skip => {
                                skip_r.store(true, Ordering::Relaxed);
                            }
                            GuiToUtil::Terminate => {
                                cancel_r.store(true, Ordering::Relaxed);
                                term_r.store(true, Ordering::Relaxed);
                                break;
                            }
                            GuiToUtil::Write(opts) => {
                                let _ = write_tx.send(*opts);
                            }
                        }
                    }
                }
            }
        }
    });

    // Heartbeat watchdog
    let timeout = Duration::from_millis(args.timeout_ms);
    let cancel_w = cancel.clone();
    let term_w = terminate.clone();
    let hb_w = last_heartbeat;
    std::thread::spawn(move || {
        while !term_w.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(500));
            if let Ok(last) = hb_w.lock()
                && last.elapsed() > timeout
            {
                cancel_w.store(true, Ordering::Relaxed);
                term_w.store(true, Ordering::Relaxed);
                break;
            }
        }
    });

    let options = match write_rx.recv_timeout(timeout) {
        Ok(opts) => opts,
        Err(_) => {
            send_msg(
                &writer,
                &UtilToGui::Error(EtcherError::new(
                    "Timed out waiting for write command",
                    "No write options received from GUI",
                )),
            );
            cleanup_tmp_files(None);
            return ExitCode::from(1);
        }
    };

    let writer_prog = writer.clone();
    let writer_fail = writer.clone();
    let writer_log = writer.clone();

    let result = write_and_validate(
        &options,
        cancel.clone(),
        skip_verify.clone(),
        |state| send_msg(&writer_prog, &UtilToGui::State(state)),
        |err| send_msg(&writer_fail, &UtilToGui::Fail(err)),
        |msg| {
            send_msg(
                &writer_log,
                &UtilToGui::Log {
                    message: msg.to_string(),
                },
            )
        },
    );

    terminate.store(true, Ordering::Relaxed);
    cleanup_tmp_files(Some(Duration::from_secs(0)));

    match result {
        Ok(res) if res.cancelled => {
            send_msg(&writer, &UtilToGui::Abort);
            ExitCode::from(1)
        }
        Ok(res) if res.skip => {
            send_msg(&writer, &UtilToGui::Done(res));
            ExitCode::SUCCESS
        }
        Ok(res) => {
            let has_errors = !res.errors.is_empty();
            send_msg(&writer, &UtilToGui::Done(res));
            if has_errors {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            send_msg(&writer, &UtilToGui::Error(e));
            ExitCode::from(1)
        }
    }
}
