#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod icons;
mod theme;
mod views;

use std::process::ExitCode;

use iced::{Size, window};

use etcher_core::settings::{DEFAULT_HEIGHT, DEFAULT_WIDTH, Settings};
use etcher_core::supported_formats::{is_http_url, normalize_image_url};

use crate::app::EtcherApp;

struct LowMemTokioExecutor(tokio::runtime::Runtime);

impl iced::Executor for LowMemTokioExecutor {
    fn new() -> Result<Self, std::io::Error> {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(4)
            .thread_stack_size(512 * 1024)
            .enable_all()
            .build()
            .map(Self)
    }

    #[allow(clippy::let_underscore_future)]
    fn spawn(&self, future: impl std::future::Future<Output = ()> + Send + 'static) {
        let _ = self.0.spawn(future);
    }

    fn enter<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = self.0.enter();
        f()
    }

    fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        self.0.block_on(future)
    }
}

fn load_window_icon() -> Option<window::Icon> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(icons::APP_ICON_PNG));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    window::icon::from_rgba(buf, info.width, info.height).ok()
}

fn parse_initial_source_arg() -> Option<String> {
    for arg in std::env::args().skip(1) {
        if arg.starts_with("--") {
            continue;
        }
        let norm = normalize_image_url(&arg);
        if is_http_url(&norm) || std::path::Path::new(&norm).is_file() {
            return Some(arg);
        }
    }
    None
}

fn run_smoke_test() -> ExitCode {
    assert!(
        load_window_icon().is_some(),
        "APP_ICON_PNG failed to decode"
    );
    assert!(!theme::SOURCE_SANS_REGULAR_BYTES.is_empty());
    assert!(!theme::SOURCE_SANS_SEMIBOLD_BYTES.is_empty());

    let (mut app, _) = EtcherApp::new();
    let _ = app.title();
    let _ = app.view();

    // Exercise modals & finish screen view construction
    let all_modals = [
        app::ActiveModal::ImageDetails,
        app::ActiveModal::Settings,
        app::ActiveModal::ExitConfirm,
        app::ActiveModal::FlashError("Sample error".to_string()),
        app::ActiveModal::SourceWarning {
            title: "Warning".to_string(),
            message: "Sample warning".to_string(),
        },
        app::ActiveModal::UrlSelector(app::UrlModalState {
            url: "https://example.com/os.img.xz".to_string(),
            show_auth: true,
            username: "u".to_string(),
            password: "p".to_string(),
            loading: false,
        }),
        app::ActiveModal::DriveSelector(app::DriveModalState {
            write: true,
            multiple: true,
            draft_selected: Vec::new(),
            show_system_drives: false,
        }),
        app::ActiveModal::DriveStatusWarning {
            is_system: true,
            drives: Vec::new(),
        },
        app::ActiveModal::FailedTargets(Vec::new()),
    ];
    for m in all_modals {
        app.modal = Some(m);
        let _ = app.view();
    }
    app.modal = None;
    app.screen = app::Screen::Finish;
    let _ = app.view();

    println!(
        "balena-etcher Rust GUI smoke test passed (iced 0.14, locale={:?}, drives={})",
        app.locale,
        app.store.available_drives.len()
    );
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    if std::env::args().any(|a| a == "--smoke-test") {
        return run_smoke_test();
    }

    let settings = Settings::load();
    let fullscreen = settings.fullscreen;
    let initial_source = parse_initial_source_arg();
    let icon = load_window_icon();

    let res = iced::application(
        move || EtcherApp::new_with_initial_source(initial_source.clone()),
        EtcherApp::update,
        EtcherApp::view,
    )
    .executor::<LowMemTokioExecutor>()
    .title(EtcherApp::title)
    .subscription(EtcherApp::subscription)
    .theme(EtcherApp::theme)
    .font(theme::SOURCE_SANS_REGULAR_BYTES)
    .font(theme::SOURCE_SANS_SEMIBOLD_BYTES)
    .default_font(theme::FONT_REGULAR)
    .window(window::Settings {
        size: Size::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
        resizable: false,
        decorations: !fullscreen,
        icon,
        exit_on_close_request: false,
        ..Default::default()
    })
    .centered()
    .run();

    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("GUI error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use etcher_core::types::{FlashPhase, ProgressState};

    #[test]
    fn test_smoke_and_title_updates() {
        assert_eq!(run_smoke_test(), ExitCode::SUCCESS);
        let (mut app, _) = EtcherApp::new();
        assert_eq!(app.title(), "balenaEtcher");
        app.store.set_flashing_flag();
        app.store.set_progress_state(ProgressState {
            phase: FlashPhase::Flashing,
            percentage: Some(42),
            ..Default::default()
        });
        assert_eq!(app.title(), "balenaEtcher – 42% Flashing...");
    }
}
