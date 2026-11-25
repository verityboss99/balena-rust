use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use iced::widget::{container, stack};
use iced::{Element, Length, Subscription, Task, Theme, keyboard, window};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{Mutex as AsyncMutex, mpsc};

use etcher_core::drive_constraints::{
    CompatibilityStatusCode, get_drive_image_compatibility_statuses,
};
use etcher_core::drives::scan_drives;
use etcher_core::errors::{EtcherError, error_message_from_code};
use etcher_core::i18n::{
    Locale, messages, msg_flash_complete, msg_flash_failure, msg_generic_flash_error,
    msg_missing_partition_table, msg_open_source, title_from_flash_state,
};
use etcher_core::ipc::{
    GuiToUtil, ProbeRequest, SpawnOutcome, UpdateCheckResult, UtilToGui, find_util_binary,
    generate_auth_token, spawn_util_process,
};
use etcher_core::leds::{LedController, LedStep};
use etcher_core::settings::Settings;
use etcher_core::store::AppStore;
use etcher_core::supported_formats::{
    SUPPORTED_EXTENSIONS, is_http_url, looks_like_windows_image, normalize_image_url,
};
use etcher_core::types::{
    Authentication, Drive, DriveKind, FlashError, FlashPhase, FlashResults, ProgressState,
    SourceMetadata, SourceType, WriteOptions,
};

use crate::theme;
use crate::views::{finish_page, header, main_page, modals};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    Finish,
}

#[derive(Debug, Clone)]
pub struct DriveModalState {
    pub write: bool,
    pub multiple: bool,
    pub draft_selected: Vec<String>,
    pub show_system_drives: bool,
}

#[derive(Debug, Clone)]
pub struct UrlModalState {
    pub url: String,
    pub show_auth: bool,
    pub username: String,
    pub password: String,
    pub loading: bool,
}

#[derive(Debug, Clone)]
pub enum ActiveModal {
    DriveSelector(DriveModalState),
    UrlSelector(UrlModalState),
    ImageDetails,
    SourceWarning { title: String, message: String },
    DriveStatusWarning { is_system: bool, drives: Vec<Drive> },
    FlashError(String),
    FailedTargets(Vec<FlashError>),
    Settings,
    ExitConfirm,
}

#[derive(Debug, Clone)]
pub enum FlashWorkerEvent {
    Progress(ProgressState),
    TargetFailed(FlashError),
    Done(FlashResults),
    Cancelled,
    Error(EtcherError),
}

#[derive(Clone)]
pub struct FlashSubscriptionHandle {
    pub id: u64,
    pub rx: Arc<AsyncMutex<Option<mpsc::UnboundedReceiver<FlashWorkerEvent>>>>,
}

impl Hash for FlashSubscriptionHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    TickDrives,
    DrivesUpdated(Vec<Drive>),
    SourceFlowHover(Option<usize>),
    OpenFileSelector,
    FilePicked(Option<PathBuf>),
    OpenUrlSelector,
    UrlInputChanged(String),
    ToggleUrlAuth,
    UrlUsernameChanged(String),
    UrlPasswordChanged(String),
    SubmitUrl,
    OpenSourceDriveSelector,
    SourceProbed(Box<Result<SourceMetadata, EtcherError>>),
    ClearSource,
    ShowImageDetails,
    DismissSourceWarningAndClear,

    OpenTargetSelector,
    ToggleDraftDrive(String),
    ShowHiddenSystemDrives,
    ConfirmDriveSelection,
    OpenDriveDriverLink(String),

    TryFlash,
    ConfirmWarningAndFlash,
    ChangeTargetFromWarning,
    FlashWorkerStarted,
    FlashEvent(FlashWorkerEvent),
    CancelFlash,
    SkipVerification,
    FlashAnother,

    ShowFailedTargetsModal,
    RetryFailedTargets,
    ErrorRetry,
    ErrorDismissAndClear,

    OpenSettings,
    ToggleErrorReporting(bool),
    ToggleAutoBlockmapping(bool),
    ToggleDesktopNotifications(bool),
    ToggleAutoUpdates(bool),
    CheckForUpdates,
    UpdateChecked(Result<UpdateCheckResult, EtcherError>),
    SetLocale(Locale),
    OpenSupportUrl,
    OpenExternal(String),
    DismissAnalyticsAlert,
    CloseModal,
    BackdropClicked,

    WindowEvent(window::Id, window::Event),
    CloseRequested(window::Id),
    ConfirmForceExit,
    EscapePressed,
}

pub struct EtcherApp {
    pub store: AppStore,
    pub settings: Settings,
    pub locale: Locale,
    pub screen: Screen,
    pub modal: Option<ActiveModal>,
    pub hovered_flow: Option<usize>,
    pub image_loading: bool,
    pub is_file_hovered: bool,
    pub main_window_id: Option<window::Id>,
    pub leds: LedController,
    pub active_flash_cmd_tx: Option<mpsc::UnboundedSender<GuiToUtil>>,
    pub active_flash_sub: Option<FlashSubscriptionHandle>,
    pub update_info: Option<UpdateCheckResult>,
    pub checking_updates: bool,
    pub update_error: Option<String>,
    next_flash_id: u64,
}

impl EtcherApp {
    pub fn new_with_initial_source(initial_source: Option<String>) -> (Self, Task<Message>) {
        let settings = Settings::load();
        let locale = settings
            .language
            .as_deref()
            .map(Locale::from_code)
            .unwrap_or_else(Locale::detect_system);
        let leds = LedController::from_settings(&settings);

        let mut store = AppStore::new();
        if let Ok(drives) = scan_drives(&settings) {
            store.set_available_drives(drives, &settings);
        }

        let checking_updates = settings.updates_enabled;
        let app = Self {
            store,
            settings,
            locale,
            screen: Screen::Main,
            modal: None,
            hovered_flow: None,
            image_loading: initial_source.is_some(),
            is_file_hovered: false,
            main_window_id: None,
            leds,
            active_flash_cmd_tx: None,
            active_flash_sub: None,
            update_info: None,
            checking_updates,
            update_error: None,
            next_flash_id: 1,
        };

        let mut tasks = vec![Task::perform(
            {
                let s = app.settings.clone();
                async move { scan_drives(&s).unwrap_or_default() }
            },
            Message::DrivesUpdated,
        )];

        if app.settings.updates_enabled {
            tasks.push(Task::perform(
                async move {
                    tokio::task::spawn_blocking(check_update_via_util)
                        .await
                        .map_err(|e| EtcherError::new("Task join error", e.to_string()))?
                },
                Message::UpdateChecked,
            ));
        }

        if let Some(src) = initial_source {
            let normalized = normalize_image_url(&src);
            let source_type = if is_http_url(&normalized) {
                SourceType::Http
            } else {
                SourceType::File
            };
            tasks.push(Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        probe_via_util(&normalized, source_type, None)
                    })
                    .await
                    .map_err(|e| EtcherError::new("Task join error", e.to_string()))?
                },
                |r| Message::SourceProbed(Box::new(r)),
            ));
        }

        (app, Task::batch(tasks))
    }

    pub fn new() -> (Self, Task<Message>) {
        Self::new_with_initial_source(None)
    }

    pub fn title(&self) -> String {
        if self.store.is_flashing {
            format!(
                "balenaEtcher – {}",
                title_from_flash_state(self.locale, &self.store.flash_state)
            )
        } else {
            "balenaEtcher".to_string()
        }
    }

    pub fn theme(&self) -> Theme {
        theme::app_theme()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),

            Message::TickDrives => {
                if self.store.is_flashing {
                    return Task::none();
                }
                let s = self.settings.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || scan_drives(&s).unwrap_or_default())
                            .await
                            .unwrap_or_default()
                    },
                    Message::DrivesUpdated,
                )
            }

            Message::DrivesUpdated(drives) => {
                if !self.store.is_flashing {
                    self.store.set_available_drives(drives, &self.settings);
                    self.sync_leds();
                }
                Task::none()
            }

            Message::SourceFlowHover(idx) => {
                self.hovered_flow = idx;
                Task::none()
            }

            Message::OpenFileSelector => {
                self.dismiss_analytics_alert_if_visible();
                let os_images_label = messages(self.locale).source_os_images.to_string();
                let all_files_label = messages(self.locale).source_all_files.to_string();
                Task::perform(
                    async move {
                        let dialog = rfd::AsyncFileDialog::new()
                            .add_filter(os_images_label, SUPPORTED_EXTENSIONS)
                            .add_filter(all_files_label, &["*"]);
                        dialog.pick_file().await.map(|f| f.path().to_path_buf())
                    },
                    Message::FilePicked,
                )
            }

            Message::FilePicked(Some(path)) => self.probe_path(path.to_string_lossy().into_owned(), SourceType::File, None),
            Message::FilePicked(None) => Task::none(),

            Message::OpenUrlSelector => {
                self.dismiss_analytics_alert_if_visible();
                self.modal = Some(ActiveModal::UrlSelector(UrlModalState {
                    url: String::new(),
                    show_auth: false,
                    username: String::new(),
                    password: String::new(),
                    loading: false,
                }));
                Task::none()
            }

            Message::UrlInputChanged(val) => {
                if let Some(ActiveModal::UrlSelector(st)) = &mut self.modal {
                    st.url = val;
                }
                Task::none()
            }

            Message::ToggleUrlAuth => {
                if let Some(ActiveModal::UrlSelector(st)) = &mut self.modal {
                    st.show_auth = !st.show_auth;
                    if !st.show_auth {
                        st.username.clear();
                        st.password.clear();
                    }
                }
                Task::none()
            }

            Message::UrlUsernameChanged(val) => {
                if let Some(ActiveModal::UrlSelector(st)) = &mut self.modal {
                    st.username = val;
                }
                Task::none()
            }

            Message::UrlPasswordChanged(val) => {
                if let Some(ActiveModal::UrlSelector(st)) = &mut self.modal {
                    st.password = val;
                }
                Task::none()
            }

            Message::SubmitUrl => {
                let (url, auth) = if let Some(ActiveModal::UrlSelector(st)) = &mut self.modal {
                    let u = st.url.trim().to_string();
                    if u.is_empty() {
                        return Task::none();
                    }
                    st.loading = true;
                    let auth = if !st.username.is_empty() {
                        Some(Authentication {
                            username: st.username.clone(),
                            password: st.password.clone(),
                        })
                    } else {
                        None
                    };
                    (u, auth)
                } else {
                    return Task::none();
                };

                if !is_http_url(&normalize_image_url(&url)) {
                    let m = messages(self.locale);
                    self.modal = Some(ActiveModal::FlashError(format!(
                        "{}\n{}",
                        m.source_unsupported_protocol, m.msg_bad_protocol
                    )));
                    return Task::none();
                }

                self.settings.push_recent_url(&url);
                let _ = self.settings.save();
                self.probe_path(url, SourceType::Http, auth)
            }

            Message::OpenSourceDriveSelector => {
                self.dismiss_analytics_alert_if_visible();
                let current = self
                    .store
                    .selected_image
                    .as_ref()
                    .and_then(|i| i.drive.as_ref().map(|d| d.device.clone()))
                    .into_iter()
                    .collect();
                self.modal = Some(ActiveModal::DriveSelector(DriveModalState {
                    write: false,
                    multiple: false,
                    draft_selected: current,
                    show_system_drives: false,
                }));
                Task::none()
            }

            Message::SourceProbed(res) => match *res {
                Ok(meta) => {
                    self.image_loading = false;
                    if matches!(self.modal, Some(ActiveModal::UrlSelector(_))) {
                        self.modal = None;
                    }
                    let m = messages(self.locale);
                    if looks_like_windows_image(&meta.path) {
                        self.modal = Some(ActiveModal::SourceWarning {
                            title: m.source_windows_image.to_string(),
                            message: m.msg_looks_like_windows_image.to_string(),
                        });
                    } else if !meta.has_mbr {
                        self.modal = Some(ActiveModal::SourceWarning {
                            title: m.source_partition_table.to_string(),
                            message: msg_missing_partition_table(self.locale, false),
                        });
                    }
                    let _ = self.store.select_source(meta);
                    self.sync_leds();
                    Task::none()
                }
                Err(err) => {
                    self.image_loading = false;
                    let m = messages(self.locale);
                    let msg = msg_open_source(
                        self.locale,
                        err.path.as_deref().unwrap_or("source"),
                        err.description(),
                    );
                    self.modal = Some(ActiveModal::FlashError(format!(
                        "{}\n\n{msg}",
                        m.source_error_open
                    )));
                    Task::none()
                }
            }

            Message::ClearSource => {
                self.dismiss_analytics_alert_if_visible();
                self.store.deselect_source();
                self.sync_leds();
                Task::none()
            }

            Message::ShowImageDetails => {
                self.dismiss_analytics_alert_if_visible();
                self.modal = Some(ActiveModal::ImageDetails);
                Task::none()
            }

            Message::DismissSourceWarningAndClear => {
                self.store.deselect_source();
                self.modal = None;
                self.sync_leds();
                Task::none()
            }

            Message::OpenTargetSelector => {
                self.dismiss_analytics_alert_if_visible();
                self.modal = Some(ActiveModal::DriveSelector(DriveModalState {
                    write: true,
                    multiple: true,
                    draft_selected: self.store.selected_devices.clone(),
                    show_system_drives: false,
                }));
                Task::none()
            }

            Message::ToggleDraftDrive(device) => {
                if let Some(ActiveModal::DriveSelector(st)) = &mut self.modal {
                    if st.multiple {
                        if let Some(pos) = st.draft_selected.iter().position(|d| d == &device) {
                            st.draft_selected.remove(pos);
                        } else {
                            st.draft_selected.push(device);
                        }
                    } else if st.draft_selected.first() == Some(&device) {
                        st.draft_selected.clear();
                    } else {
                        st.draft_selected = vec![device];
                    }
                }
                Task::none()
            }

            Message::ShowHiddenSystemDrives => {
                if let Some(ActiveModal::DriveSelector(st)) = &mut self.modal {
                    st.show_system_drives = true;
                }
                Task::none()
            }

            Message::ConfirmDriveSelection => {
                let Some(ActiveModal::DriveSelector(st)) = self.modal.take() else {
                    return Task::none();
                };
                if st.write {
                    self.store.deselect_all_targets();
                    for dev in st.draft_selected {
                        let _ = self.store.select_target(&dev);
                    }
                } else if let Some(dev) = st.draft_selected.first() {
                    if let Some(drive) = self
                        .store
                        .available_drives
                        .iter()
                        .find(|d| &d.device == dev)
                        .cloned()
                    {
                        if drive.partition_table_type.is_none() {
                            let m = messages(self.locale);
                            self.modal = Some(ActiveModal::SourceWarning {
                                title: m.source_partition_table.to_string(),
                                message: msg_missing_partition_table(self.locale, true),
                            });
                        }
                        let meta = SourceMetadata {
                            path: drive.device.clone(),
                            name: Some(drive.description.clone()),
                            display_name: drive.display_name.clone(),
                            description: drive.description.clone(),
                            size: drive.size,
                            compressed_size: None,
                            is_size_estimated: false,
                            recommended_drive_size: None,
                            has_mbr: drive.partition_table_type.is_some(),
                            partitions: Vec::new(),
                            source_type: SourceType::BlockDevice,
                            extension: None,
                            archive_extension: None,
                            drive: Some(drive),
                            auth: None,
                            logo: None,
                            support_url: None,
                            is_compressed: false,
                            blockmapped_size: None,
                        };
                        let _ = self.store.select_source(meta);
                    }
                } else {
                    self.store.deselect_source();
                }
                self.sync_leds();
                Task::none()
            }

            Message::OpenDriveDriverLink(device) => {
                if let Some(d) = self
                    .store
                    .available_drives
                    .iter()
                    .find(|d| d.device == device)
                    && let DriveKind::Driverless { link, .. } = &d.kind
                {
                    self.open_url_if_allowed(link);
                }
                Task::none()
            }

            Message::TryFlash => {
                if self.store.is_flashing || !self.store.has_image() || !self.store.has_drive() {
                    return Task::none();
                }
                let selected = self.store.selected_drives();
                let mut warned: Vec<Drive> = Vec::new();
                let mut has_system = false;
                for drive in &selected {
                    let statuses =
                        get_drive_image_compatibility_statuses(drive, None, true, self.locale);
                    if !statuses.is_empty() {
                        if statuses
                            .iter()
                            .any(|s| s.code == CompatibilityStatusCode::System)
                        {
                            has_system = true;
                        }
                        warned.push(drive.clone());
                    }
                }
                if !warned.is_empty() {
                    let filtered: Vec<Drive> = warned
                        .into_iter()
                        .filter(|d| d.is_system || !has_system)
                        .collect();
                    self.modal = Some(ActiveModal::DriveStatusWarning {
                        is_system: has_system,
                        drives: filtered,
                    });
                    return Task::none();
                }
                self.start_flash()
            }

            Message::ConfirmWarningAndFlash => {
                self.modal = None;
                self.start_flash()
            }

            Message::ChangeTargetFromWarning => {
                self.modal = Some(ActiveModal::DriveSelector(DriveModalState {
                    write: true,
                    multiple: true,
                    draft_selected: self.store.selected_devices.clone(),
                    show_system_drives: false,
                }));
                Task::none()
            }

            Message::FlashWorkerStarted => Task::none(),

            Message::FlashEvent(ev) => match ev {
                FlashWorkerEvent::Progress(state) => {
                    self.store.set_progress_state(state);
                    self.sync_leds();
                    Task::none()
                }
                FlashWorkerEvent::TargetFailed(err) => {
                    self.store.add_failed_device_error(err);
                    self.sync_leds();
                    Task::none()
                }
                FlashWorkerEvent::Done(results) => {
                    let skip = results.skip;
                    let succ = results.successful_devices;
                    let fail = results.failed_devices;
                    self.store.unset_flashing_flag(results);
                    self.active_flash_cmd_tx = None;
                    self.active_flash_sub = None;
                    self.sync_leds();

                    if !skip {
                        self.send_finish_notification(succ, fail);
                    }
                    self.screen = Screen::Finish;
                    Task::none()
                }
                FlashWorkerEvent::Cancelled => {
                    self.store.unset_flashing_flag(FlashResults {
                        cancelled: true,
                        ..Default::default()
                    });
                    self.active_flash_cmd_tx = None;
                    self.active_flash_sub = None;
                    self.sync_leds();
                    Task::none()
                }
                FlashWorkerEvent::Error(err) => {
                    self.store.unset_flashing_flag(FlashResults {
                        cancelled: false,
                        ..Default::default()
                    });
                    self.active_flash_cmd_tx = None;
                    self.active_flash_sub = None;
                    self.sync_leds();

                    let msg = err
                        .code
                        .as_deref()
                        .and_then(|c| error_message_from_code(c, self.locale))
                        .unwrap_or_else(|| msg_generic_flash_error(self.locale, &err.to_string()));
                    self.modal = Some(ActiveModal::FlashError(msg));
                    Task::none()
                }
            },

            Message::CancelFlash => {
                if let Some(tx) = &self.active_flash_cmd_tx {
                    let _ = tx.send(GuiToUtil::Cancel);
                }
                Task::none()
            }

            Message::SkipVerification => {
                if let Some(tx) = &self.active_flash_cmd_tx {
                    let _ = tx.send(GuiToUtil::Skip);
                }
                Task::none()
            }

            Message::FlashAnother => {
                self.store.deselect_all_targets();
                self.store.reset_flash_state();
                self.store.new_workflow_uuid();
                self.screen = Screen::Main;
                self.sync_leds();
                Task::none()
            }

            Message::ShowFailedTargetsModal => {
                let errs = self
                    .store
                    .flash_results
                    .as_ref()
                    .map(|r| r.errors.clone())
                    .unwrap_or_default();
                self.modal = Some(ActiveModal::FailedTargets(errs));
                Task::none()
            }

            Message::RetryFailedTargets => {
                let failed_devs: Vec<String> = self
                    .store
                    .flash_results
                    .as_ref()
                    .map(|r| r.errors.iter().map(|e| e.device.clone()).collect())
                    .unwrap_or_default();
                self.modal = None;
                self.store.reset_flash_state();
                self.store.deselect_all_targets();
                for dev in failed_devs {
                    let _ = self.store.select_target(&dev);
                }
                self.screen = Screen::Main;
                self.sync_leds();
                Task::none()
            }

            Message::ErrorRetry => {
                self.modal = None;
                self.store.reset_flash_state();
                Task::none()
            }

            Message::ErrorDismissAndClear => {
                self.modal = None;
                self.store.reset_flash_state();
                self.store.clear_selection();
                Task::none()
            }

            Message::OpenSettings => {
                self.dismiss_analytics_alert_if_visible();
                self.modal = Some(ActiveModal::Settings);
                Task::none()
            }

            Message::ToggleErrorReporting(val) => {
                self.settings.error_reporting = val;
                let _ = self.settings.save();
                Task::none()
            }

            Message::ToggleAutoBlockmapping(val) => {
                self.settings.auto_blockmapping = val;
                let _ = self.settings.save();
                Task::none()
            }

            Message::ToggleDesktopNotifications(val) => {
                self.settings.desktop_notifications = val;
                let _ = self.settings.save();
                Task::none()
            }

            Message::ToggleAutoUpdates(val) => {
                self.settings.updates_enabled = val;
                let _ = self.settings.save();
                if val && self.update_info.is_none() && !self.checking_updates {
                    self.checking_updates = true;
                    self.update_error = None;
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(check_update_via_util)
                                .await
                                .map_err(|e| EtcherError::new("Task join error", e.to_string()))?
                        },
                        Message::UpdateChecked,
                    );
                }
                Task::none()
            }

            Message::CheckForUpdates => {
                if self.checking_updates {
                    return Task::none();
                }
                self.checking_updates = true;
                self.update_error = None;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(check_update_via_util)
                            .await
                            .map_err(|e| EtcherError::new("Task join error", e.to_string()))?
                    },
                    Message::UpdateChecked,
                )
            }

            Message::UpdateChecked(res) => {
                self.checking_updates = false;
                match res {
                    Ok(info) => {
                        self.update_error = None;
                        self.update_info = Some(info);
                    }
                    Err(e) => {
                        self.update_error = Some(e.0.title);
                    }
                }
                Task::none()
            }

            Message::SetLocale(loc) => {
                self.locale = loc;
                self.settings.language = Some(loc.code().to_string());
                let _ = self.settings.save();
                Task::none()
            }

            Message::OpenSupportUrl => {
                let url = self
                    .store
                    .selected_image
                    .as_ref()
                    .and_then(|i| i.support_url.clone())
                    .unwrap_or_else(|| {
                        "https://github.com/balena-io/etcher/blob/master/docs/SUPPORT.md"
                            .to_string()
                    });
                self.open_url_if_allowed(&url);
                Task::none()
            }

            Message::OpenExternal(url) => {
                self.open_url_if_allowed(&url);
                Task::none()
            }

            Message::DismissAnalyticsAlert => {
                self.settings.analytics_alert_visible = false;
                let _ = self.settings.save();
                Task::none()
            }

            Message::CloseModal | Message::BackdropClicked => {
                self.modal = None;
                Task::none()
            }

            Message::WindowEvent(id, event) => {
                self.main_window_id = Some(id);
                match event {
                    window::Event::FileHovered(_) => {
                        if !self.store.is_flashing && self.modal.is_none() && self.screen == Screen::Main {
                            self.is_file_hovered = true;
                        }
                        Task::none()
                    }
                    window::Event::FilesHoveredLeft => {
                        self.is_file_hovered = false;
                        Task::none()
                    }
                    window::Event::FileDropped(path) => {
                        self.is_file_hovered = false;
                        if !self.store.is_flashing && self.modal.is_none() && self.screen == Screen::Main {
                            return self.probe_path(
                                path.to_string_lossy().into_owned(),
                                SourceType::File,
                                None,
                            );
                        }
                        Task::none()
                    }
                    _ => Task::none(),
                }
            }

            Message::CloseRequested(id) => {
                self.main_window_id = Some(id);
                if self.store.is_flashing {
                    self.modal = Some(ActiveModal::ExitConfirm);
                    Task::none()
                } else {
                    window::close(id)
                }
            }

            Message::ConfirmForceExit => {
                if let Some(tx) = &self.active_flash_cmd_tx {
                    let _ = tx.send(GuiToUtil::Terminate);
                }
                if let Some(id) = self.main_window_id {
                    window::close(id)
                } else {
                    iced::exit()
                }
            }

            Message::EscapePressed => {
                if self.modal.is_some() {
                    self.modal = None;
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = match self.screen {
            Screen::Main => main_page::view(self, self.locale),
            Screen::Finish => finish_page::view(self, self.locale),
        };

        let base = container(
            iced::widget::column![
                header::view(self.settings.disable_external_links, self.update_info.as_ref()),
                content,
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::dark_root_container);

        if let Some(modal) = &self.modal {
            stack![base, modals::view(self, modal, self.locale)].into()
        } else {
            base.into()
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            iced::time::every(Duration::from_secs(2)).map(|_| Message::TickDrives),
            window::events().map(|(id, ev)| Message::WindowEvent(id, ev)),
            window::close_requests().map(Message::CloseRequested),
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                } => Some(Message::EscapePressed),
                _ => None,
            }),
        ];

        if let Some(handle) = &self.active_flash_sub {
            subs.push(
                Subscription::run_with(handle.clone(), flash_event_stream).map(Message::FlashEvent),
            );
        }

        Subscription::batch(subs)
    }

    fn probe_path(
        &mut self,
        path: String,
        source_type: SourceType,
        auth: Option<Authentication>,
    ) -> Task<Message> {
        self.image_loading = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || probe_via_util(&path, source_type, auth))
                    .await
                    .map_err(|e| EtcherError::new("Task join error", e.to_string()))?
            },
            |r| Message::SourceProbed(Box::new(r)),
        )
    }

    fn start_flash(&mut self) -> Task<Message> {
        let Some(image) = self.store.selected_image.clone() else {
            return Task::none();
        };
        let destinations = self.store.selected_drives();
        if destinations.is_empty() {
            return Task::none();
        }

        self.store.set_flashing_flag();
        self.store.set_device_paths(
            destinations
                .iter()
                .filter_map(|d| d.device_path.clone())
                .collect(),
        );
        self.sync_leds();

        let opts = WriteOptions {
            image,
            destinations,
            auto_blockmapping: self.settings.auto_blockmapping,
            decompress_first: self.settings.decompress_first,
            verify: true,
        };

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        self.active_flash_cmd_tx = Some(cmd_tx);
        let flash_id = self.next_flash_id;
        self.next_flash_id += 1;
        self.active_flash_sub = Some(FlashSubscriptionHandle {
            id: flash_id,
            rx: Arc::new(AsyncMutex::new(Some(ev_rx))),
        });

        Task::perform(
            async move {
                run_flash_ipc_session(opts, cmd_rx, ev_tx).await;
            },
            |()| Message::FlashWorkerStarted,
        )
    }

    fn dismiss_analytics_alert_if_visible(&mut self) {
        if self.settings.analytics_alert_visible {
            self.settings.analytics_alert_visible = false;
            let _ = self.settings.save();
        }
    }

    fn open_url_if_allowed(&self, url: &str) {
        if !self.settings.disable_external_links && !url.is_empty() {
            let _ = open::that_detached(url);
        }
    }

    fn sync_leds(&self) {
        if !self.leds.is_active() {
            return;
        }
        let step = if self.store.is_flashing {
            match self.store.flash_state.phase {
                FlashPhase::Verifying => LedStep::Verifying,
                _ => LedStep::Flashing,
            }
        } else if self.screen == Screen::Finish {
            LedStep::Finish
        } else {
            LedStep::Main
        };
        let selected: Vec<String> = if step == LedStep::Main {
            self.store
                .selected_drives()
                .into_iter()
                .filter_map(|d| d.device_path)
                .collect()
        } else {
            self.store.device_paths.clone()
        };
        let failed: Vec<String> = self
            .store
            .failed_device_errors
            .iter()
            .filter_map(|e| e.device_path.clone())
            .collect();
        self.leds.update(step, None, &selected, &failed);
    }

    fn send_finish_notification(&self, successful: usize, failed: usize) {
        if !self.settings.desktop_notifications {
            return;
        }
        let img_base = self
            .store
            .selected_image
            .as_ref()
            .and_then(|i| {
                Path::new(&i.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "image".to_string());
        let drives = self.store.selected_drives();
        let first_drive = drives
            .first()
            .map(|d| (d.description.as_str(), d.display_name.as_str()));

        let (summary, body) = if successful > 0 {
            (
                "Flash complete!",
                msg_flash_complete(self.locale, &img_base, first_drive, successful, failed),
            )
        } else {
            let pairs: Vec<(&str, &str)> = drives
                .iter()
                .map(|d| (d.description.as_str(), d.display_name.as_str()))
                .collect();
            (
                "Oops! Looks like the flash failed.",
                msg_flash_failure(self.locale, &img_base, &pairs),
            )
        };
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("notify-send")
                .args(["-a", "balenaEtcher", "--", summary, &body])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!(
                "display notification \"{}\" with title \"{}\"",
                esc(&body),
                esc(summary)
            );
            let _ = std::process::Command::new("osascript")
                .args(["-e", &script])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (summary, body);
        }
    }
}

fn probe_via_util(
    selected: &str,
    source_type: SourceType,
    auth: Option<Authentication>,
) -> Result<SourceMetadata, EtcherError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let util_path = find_util_binary()?;
    let req = ProbeRequest {
        selected: selected.to_string(),
        source_type,
        auth,
    };
    let req_json = serde_json::to_string(&req)
        .map_err(|e| EtcherError::new("Failed to encode probe request", e.to_string()))?;

    let mut cmd = Command::new(&util_path);
    cmd.arg("--probe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| EtcherError::from_io_error(&e, util_path.to_str()))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(req_json.as_bytes())
            .map_err(|e| EtcherError::new("Failed to send probe request", e.to_string()))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| EtcherError::new("Failed waiting for probe helper", e.to_string()))?;

    if output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EtcherError::new(
            "Source probe failed",
            if stderr.trim().is_empty() {
                format!("etcher-util --probe exited with {}", output.status)
            } else {
                stderr.trim().to_string()
            },
        ));
    }

    serde_json::from_slice::<Result<SourceMetadata, EtcherError>>(&output.stdout).map_err(|e| {
        EtcherError::new(
            "Invalid probe response",
            format!("{e}: {}", String::from_utf8_lossy(&output.stdout)),
        )
    })?
}

fn check_update_via_util() -> Result<UpdateCheckResult, EtcherError> {
    use std::process::{Command, Stdio};

    let util_path = find_util_binary()?;
    let mut cmd = Command::new(&util_path);
    cmd.arg("--check-update")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let output = cmd
        .output()
        .map_err(|e| EtcherError::from_io_error(&e, util_path.to_str()))?;

    if output.stdout.is_empty() {
        return Err(EtcherError::new(
            "Update check failed",
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    serde_json::from_slice::<Result<UpdateCheckResult, EtcherError>>(&output.stdout).map_err(
        |e| EtcherError::new("Invalid update response", e.to_string()),
    )?
}

fn flash_event_stream(
    handle: &FlashSubscriptionHandle,
) -> iced::futures::stream::BoxStream<'static, FlashWorkerEvent> {
    let rx_holder = handle.rx.clone();
    Box::pin(iced::futures::stream::unfold(
        Some(rx_holder),
        |state| async move {
            let holder = state?;
            let mut guard = holder.lock().await;
            let rx = guard.as_mut()?;
            let ev = rx.recv().await?;
            drop(guard);
            Some((ev, Some(holder)))
        },
    ))
}

async fn run_flash_ipc_session(
    options: WriteOptions,
    mut cmd_rx: mpsc::UnboundedReceiver<GuiToUtil>,
    ev_tx: mpsc::UnboundedSender<FlashWorkerEvent>,
) {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => {
            let _ = ev_tx.send(FlashWorkerEvent::Error(EtcherError::new(
                "Failed to start IPC server",
                e.to_string(),
            )));
            return;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let token = generate_auth_token();
    let expected_token = token.clone();
    let util_path = match find_util_binary() {
        Ok(p) => p,
        Err(e) => {
            let _ = ev_tx.send(FlashWorkerEvent::Error(e));
            return;
        }
    };

    // Elevate if any destination or source block device is a real OS device path
    let needs_privileges = options.image.source_type == SourceType::BlockDevice
        || options
            .destinations
            .iter()
            .any(|d| d.raw.starts_with("/dev/") || d.raw.starts_with("\\\\.\\"));

    let child_res = tokio::task::spawn_blocking(move || {
        spawn_util_process(&util_path, "127.0.0.1", port, &token, needs_privileges)
    })
    .await;

    let mut child = match child_res {
        Ok(Ok(SpawnOutcome::Spawned(c))) => c,
        Ok(Ok(SpawnOutcome::Cancelled)) => {
            let _ = ev_tx.send(FlashWorkerEvent::Cancelled);
            return;
        }
        Ok(Err(e)) => {
            let _ = ev_tx.send(FlashWorkerEvent::Error(e));
            return;
        }
        Err(e) => {
            let _ = ev_tx.send(FlashWorkerEvent::Error(EtcherError::new(
                "Spawn failed",
                e.to_string(),
            )));
            return;
        }
    };

    // Wait up to 45s for pkexec/elevation prompt + connection, polling child status every 200ms
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    let mut poll_child = tokio::time::interval(Duration::from_millis(200));
    let stream = loop {
        tokio::select! {
            accept_res = listener.accept() => {
                match accept_res {
                    Ok((s, _)) => break s,
                    Err(e) => {
                        let _ = child.kill();
                        let _ = ev_tx.send(FlashWorkerEvent::Error(EtcherError::new(
                            "IPC accept failed",
                            e.to_string(),
                        )));
                        return;
                    }
                }
            }
            _ = poll_child.tick() => {
                if let Ok(Some(status)) = child.try_wait() {
                    if status.code() == Some(126) {
                        let _ = ev_tx.send(FlashWorkerEvent::Cancelled);
                    } else {
                        let _ = ev_tx.send(FlashWorkerEvent::Error(
                            EtcherError::user_error(
                                "The elevated process died unexpectedly",
                                format!("The helper process exited with status {status}"),
                            )
                            .with_code(etcher_core::errors::ErrorCode::Echilddied),
                        ));
                    }
                    return;
                }
                if tokio::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = ev_tx.send(FlashWorkerEvent::Error(EtcherError::new(
                        "Timed out waiting for privileged flasher",
                        "Authentication was cancelled or the helper process did not respond.",
                    )));
                    return;
                }
            }
        }
    };

    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();

    // Verify exact Auth token
    match lines.next_line().await {
        Ok(Some(line)) => {
            match serde_json::from_str::<GuiToUtil>(&line) {
                Ok(GuiToUtil::Auth { token: got }) if got == expected_token => {}
                _ => {
                    let _ = child.kill();
                    let _ = ev_tx.send(FlashWorkerEvent::Error(EtcherError::new(
                        "Invalid sidecar handshake",
                        "Unexpected or mismatched authentication token",
                    )));
                    return;
                }
            }
        }
        _ => {
            let _ = child.kill();
            let _ = ev_tx.send(FlashWorkerEvent::Cancelled);
            return;
        }
    }

    // Wait for Ready, then send Write(options)
    let mut sent_write = false;
    let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
    let mut finished = false;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if let Ok(hb) = serde_json::to_string(&GuiToUtil::Heartbeat) {
                    let _ = write_half.write_all(format!("{hb}\n").as_bytes()).await;
                    let _ = write_half.flush().await;
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                if let Ok(s) = serde_json::to_string(&cmd) {
                    let _ = write_half.write_all(format!("{s}\n").as_bytes()).await;
                    let _ = write_half.flush().await;
                }
            }
            line_res = lines.next_line() => {
                match line_res {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Ok(msg) = serde_json::from_str::<UtilToGui>(trimmed) {
                            match msg {
                                UtilToGui::Ready => {
                                    if !sent_write {
                                        sent_write = true;
                                        if let Ok(w) = serde_json::to_string(&GuiToUtil::Write(Box::new(options.clone()))) {
                                            let _ = write_half.write_all(format!("{w}\n").as_bytes()).await;
                                            let _ = write_half.flush().await;
                                        }
                                    }
                                }
                                UtilToGui::Log { message } => {
                                    let _ = message;
                                }
                                UtilToGui::State(st) => {
                                    let _ = ev_tx.send(FlashWorkerEvent::Progress(st));
                                }
                                UtilToGui::Fail(fe) => {
                                    let _ = ev_tx.send(FlashWorkerEvent::TargetFailed(fe));
                                }
                                UtilToGui::Done(res) => {
                                    finished = true;
                                    let _ = ev_tx.send(FlashWorkerEvent::Done(res));
                                    break;
                                }
                                UtilToGui::Abort | UtilToGui::Skip => {
                                    finished = true;
                                    let _ = ev_tx.send(FlashWorkerEvent::Cancelled);
                                    break;
                                }
                                UtilToGui::Error(err) => {
                                    finished = true;
                                    let _ = ev_tx.send(FlashWorkerEvent::Error(err));
                                    break;
                                }
                            }
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }

    let _ = child.wait();
    if !finished {
        let _ = ev_tx.send(FlashWorkerEvent::Error(
            EtcherError::user_error(
                "The writer process ended unexpectedly",
                "Please try again, and contact the Etcher team if the problem persists.",
            )
            .with_code(etcher_core::errors::ErrorCode::Echilddied),
        ));
    }
}
