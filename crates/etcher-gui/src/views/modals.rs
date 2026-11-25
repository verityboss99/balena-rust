use iced::widget::{
    button, checkbox, column, container, mouse_area, row, scrollable, space, svg, text,
    text_input, toggler, tooltip,
};
use iced::{Alignment, Color, Element, Length};

use etcher_core::drive_constraints::{
    CompatibilityStatusCode, CompatibilityStatusKind, get_drive_image_compatibility_statuses,
    is_drive_size_large, is_drive_valid,
};
use etcher_core::i18n::{
    Locale, drives_found, drives_select, drives_show_hidden, messages, msg_too_small,
};
use etcher_core::types::{Drive, DriveKind, FlashError};
use etcher_core::units::{middle_ellipsis, pretty_bytes};

use crate::app::{ActiveModal, DriveModalState, EtcherApp, Message, UrlModalState};
use crate::icons;
use crate::theme::{
    self, COUNT_BLUE, FAIL_RED, FG_DISABLED, FONT_SEMIBOLD, MODAL_TEXT, PRIMARY, TEXT_HOVER_MUTED,
    TEXT_MUTED, WARNING,
};

pub fn view<'a>(app: &'a EtcherApp, modal: &'a ActiveModal, locale: Locale) -> Element<'a, Message> {
    let dialog: Element<'a, Message> = match modal {
        ActiveModal::DriveSelector(st) => view_drive_selector(app, st, locale),
        ActiveModal::UrlSelector(st) => view_url_selector(app, st, locale),
        ActiveModal::ImageDetails => view_image_details(app, locale),
        ActiveModal::SourceWarning { title, message } => {
            view_source_warning(title, message, locale)
        }
        ActiveModal::DriveStatusWarning { is_system, drives } => {
            view_drive_status_warning(*is_system, drives, locale)
        }
        ActiveModal::FlashError(msg) => view_flash_error(msg, locale),
        ActiveModal::FailedTargets(errors) => view_failed_targets(errors, locale),
        ActiveModal::Settings => view_settings(app, locale),
        ActiveModal::ExitConfirm => view_exit_confirm(locale),
    };

    mouse_area(
        container(dialog)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(16)
            .style(theme::modal_backdrop),
    )
    .on_press(Message::BackdropClicked)
    .into()
}

fn modal_shell<'a>(
    width: f32,
    height: f32,
    header: Element<'a, Message>,
    body: Element<'a, Message>,
    footer: Element<'a, Message>,
) -> Element<'a, Message> {
    // Stop clicks inside card from closing via backdrop
    mouse_area(
        container(
            column![
                container(header)
                    .padding(iced::Padding {
                        top: 20.0,
                        right: 28.0,
                        bottom: 12.0,
                        left: 28.0,
                    })
                    .width(Length::Fill),
                container(body)
                    .padding(iced::Padding {
                        top: 0.0,
                        right: 28.0,
                        bottom: 12.0,
                        left: 28.0,
                    })
                    .width(Length::Fill)
                    .height(Length::Fill),
                container(footer)
                    .padding([14, 28])
                    .width(Length::Fill)
                    .style(theme::modal_footer),
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(theme::modal_card),
    )
    .on_press(Message::Noop)
    .into()
}

fn view_drive_selector<'a>(
    app: &'a EtcherApp,
    st: &'a DriveModalState,
    locale: Locale,
) -> Element<'a, Message> {
    let m = messages(locale);
    let image = app.store.selected_image.as_ref();
    let all_drives = &app.store.available_drives;

    let displayed: Vec<&Drive> = all_drives
        .iter()
        .filter(|d| {
            d.is_usbboot()
                || d.is_driverless()
                || st.draft_selected.iter().any(|sel| sel == &d.device)
                || st.show_system_drives
                || !d.is_system
        })
        .collect();

    let total_system = all_drives.iter().filter(|d| d.is_system).count();
    let shown_system = displayed.iter().filter(|d| d.is_system).count();
    let hidden_system = total_system.saturating_sub(shown_system);

    let selected_has_system = st.write
        && st
            .draft_selected
            .iter()
            .any(|dev| all_drives.iter().any(|d| &d.device == dev && d.is_system));

    let title_label = if st.write {
        m.target_select_target
    } else {
        m.source_select_source
    };

    let header = row![
        text(title_label).font(FONT_SEMIBOLD).size(22).color(MODAL_TEXT),
        text(drives_found(locale, all_drives.len()))
            .font(FONT_SEMIBOLD)
            .size(12)
            .color(COUNT_BLUE),
    ]
    .spacing(12)
    .align_y(Alignment::End)
    .into();

    let body: Element<'a, Message> = if all_drives.is_empty() {
        let empty_svg = if st.write {
            icons::tgt_empty_icon()
        } else {
            icons::src_empty_icon()
        };
        let empty_label = if st.write {
            m.target_plug_target
        } else {
            m.source_plug_source
        };
        container(
            column![
                svg(empty_svg).width(44).height(88),
                text(empty_label).font(FONT_SEMIBOLD).size(15).color(MODAL_TEXT),
            ]
            .spacing(14)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else {
        let table_hdr = container(
            row![
                space().width(Length::Fixed(34.0)),
                text(m.drives_name)
                    .font(FONT_SEMIBOLD)
                    .size(13)
                    .color(MODAL_TEXT)
                    .width(Length::FillPortion(32)),
                text(m.drives_size)
                    .font(FONT_SEMIBOLD)
                    .size(13)
                    .color(MODAL_TEXT)
                    .width(Length::FillPortion(15)),
                text(m.drives_location)
                    .font(FONT_SEMIBOLD)
                    .size(13)
                    .color(MODAL_TEXT)
                    .width(Length::FillPortion(18)),
                space().width(Length::FillPortion(30)),
            ]
            .padding([8, 12])
            .align_y(Alignment::Center),
        )
        .style(theme::table_header_container)
        .width(Length::Fill);

        let mut rows_col = column![].width(Length::Fill);
        for drive in displayed {
            let is_selected = st.draft_selected.iter().any(|s| s == &drive.device);
            let is_disabled = !drive.is_block_device()
                || !is_drive_valid(drive, image, st.write)
                || (st.write && drive.is_read_only);

            let has_warn = st.write && (is_drive_size_large(drive) || drive.is_system);
            let short_desc = middle_ellipsis(&drive.description, 32)
                .unwrap_or_else(|_| drive.description.clone());

            let mut name_cell = row![].spacing(6).align_y(Alignment::Center);
            if has_warn && drive.is_block_device() {
                name_cell = name_cell.push(
                    svg(icons::warning_triangle_icon())
                        .width(14)
                        .height(14)
                        .style(theme::svg_tint(if drive.is_system {
                            WARNING
                        } else {
                            TEXT_HOVER_MUTED
                        })),
                );
            }
            name_cell = name_cell.push(
                text(short_desc)
                    .size(14)
                    .color(if is_disabled { FG_DISABLED } else { MODAL_TEXT }),
            );

            let size_txt = drive.size.map(pretty_bytes).unwrap_or_default();

            let extra_cell: Element<'a, Message> = match &drive.kind {
                DriveKind::Usbboot { progress } => column![
                    text(m.drives_initializing).size(11).color(MODAL_TEXT),
                    iced::widget::progress_bar(0.0..=100.0, *progress as f32)
                        .length(Length::Fixed(120.0))
                        .girth(Length::Fixed(4.0))
                        .style(theme::flash_progress_bar(PRIMARY)),
                ]
                .spacing(4)
                .into(),
                DriveKind::Driverless { link_cta, .. } => {
                    button(text(link_cta).size(13).font(FONT_SEMIBOLD))
                        .padding([2, 6])
                        .style(theme::plain_button(PRIMARY, MODAL_TEXT))
                        .on_press(Message::OpenDriveDriverLink(drive.device.clone()))
                        .into()
                }
                DriveKind::BlockDevice => {
                    let statuses =
                        get_drive_image_compatibility_statuses(drive, image, st.write, locale);
                    let mut badges = row![].spacing(6).align_y(Alignment::Center);
                    for status in statuses.into_iter().take(2) {
                        let is_err = status.kind == CompatibilityStatusKind::Error;
                        let is_warn = status.kind == CompatibilityStatusKind::Warning;
                        let tip_text = match status.code {
                            CompatibilityStatusCode::ContainsImage => m.msg_source_drive.to_string(),
                            CompatibilityStatusCode::Large => m.msg_large_drive_size.to_string(),
                            CompatibilityStatusCode::System => m.msg_system_drive.to_string(),
                            CompatibilityStatusCode::TooSmall => {
                                let req = image
                                    .and_then(|i| i.recommended_drive_size.or(i.size))
                                    .unwrap_or(0);
                                msg_too_small(locale, req, drive.size.unwrap_or(0))
                            }
                            _ => status.message.clone(),
                        };
                        let badge = container(text(status.message).size(11).font(FONT_SEMIBOLD))
                            .padding([3, 7])
                            .style(theme::badge_container(is_err, is_warn));
                        if st.write {
                            badges = badges.push(tooltip(
                                badge,
                                container(text(tip_text).size(12))
                                    .padding([4, 8])
                                    .style(theme::tooltip_container),
                                tooltip::Position::Top,
                            ));
                        } else {
                            badges = badges.push(badge);
                        }
                    }
                    badges.into()
                }
            };

            let cb = checkbox(is_selected)
                .size(18)
                .style(theme::modal_checkbox);

            let row_content = row![
                container(cb).width(Length::Fixed(34.0)).center_y(Length::Fill),
                container(name_cell).width(Length::FillPortion(32)),
                text(size_txt)
                    .size(13)
                    .color(if is_disabled { FG_DISABLED } else { MODAL_TEXT })
                    .width(Length::FillPortion(15)),
                text(&drive.display_name)
                    .size(13)
                    .color(if is_disabled { FG_DISABLED } else { MODAL_TEXT })
                    .width(Length::FillPortion(18)),
                container(extra_cell).width(Length::FillPortion(30)),
            ]
            .padding([8, 12])
            .align_y(Alignment::Center);

            let mut row_btn = button(row_content)
                .padding(0)
                .width(Length::Fill)
                .style(theme::drive_row_button(is_selected, drive.is_system, is_disabled));

            if !is_disabled {
                row_btn = row_btn.on_press(Message::ToggleDraftDrive(drive.device.clone()));
            }
            rows_col = rows_col.push(row_btn);
        }

        let mut list_col = column![table_hdr, scrollable(rows_col).height(Length::Fill)]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

        if hidden_system > 0 {
            list_col = list_col.push(
                button(
                    row![
                        svg(icons::chevron_down_icon())
                            .width(12)
                            .height(12)
                            .style(theme::svg_tint(PRIMARY)),
                        text(drives_show_hidden(locale, hidden_system))
                            .size(13)
                            .color(PRIMARY),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([8, 4])
                .style(theme::plain_button(PRIMARY, MODAL_TEXT))
                .on_press(Message::ShowHiddenSystemDrives),
            );
        }

        if selected_has_system {
            list_col = list_col.push(
                container(
                    text(m.drives_system_drive_danger)
                        .size(13)
                        .font(FONT_SEMIBOLD)
                        .color(Color::WHITE),
                )
                .padding([6, 12])
                .width(Length::Fill)
                .style(theme::system_danger_banner),
            );
        }

        list_col.into()
    };

    let mut select_btn = button(
        container(
            text(drives_select(locale, st.draft_selected.len()))
                .font(FONT_SEMIBOLD)
                .size(14),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .width(Length::Fixed(180.0))
    .height(Length::Fixed(42.0))
    .style(if selected_has_system {
        theme::warning_pill_button
    } else {
        theme::primary_pill_button
    });
    if !all_drives.is_empty() {
        select_btn = select_btn.on_press(Message::ConfirmDriveSelection);
    }

    let cancel_btn = button(
        container(text(m.cancel).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(150.0))
    .height(Length::Fixed(42.0))
    .style(theme::outline_pill_button)
    .on_press(Message::CloseModal);

    let footer = row![cancel_btn, select_btn]
        .spacing(20)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    modal_shell(750.0, 420.0, header, body, container(footer).center_x(Length::Fill).into())
}

fn view_url_selector<'a>(
    app: &'a EtcherApp,
    st: &'a UrlModalState,
    locale: Locale,
) -> Element<'a, Message> {
    let m = messages(locale);
    let header = text(m.source_use_source_url)
        .font(FONT_SEMIBOLD)
        .size(22)
        .color(MODAL_TEXT)
        .into();

    let url_input = text_input(m.source_enter_valid_url, &st.url)
        .on_input(Message::UrlInputChanged)
        .on_submit(Message::SubmitUrl)
        .padding([10, 12])
        .size(14)
        .style(theme::modal_text_input);

    let auth_chevron = if st.show_auth {
        icons::chevron_down_icon()
    } else {
        icons::chevron_right_icon()
    };

    let auth_toggle = button(
        row![
            svg(auth_chevron)
                .width(12)
                .height(12)
                .style(theme::svg_tint(PRIMARY)),
            text(m.source_auth).size(14).color(PRIMARY),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 0])
    .style(theme::plain_button(PRIMARY, MODAL_TEXT))
    .on_press(Message::ToggleUrlAuth);

    let mut col = column![url_input, auth_toggle].spacing(10).width(Length::Fill);

    if st.show_auth {
        col = col
            .push(
                text_input(m.source_username, &st.username)
                    .on_input(Message::UrlUsernameChanged)
                    .padding([8, 12])
                    .size(14)
                    .style(theme::modal_text_input),
            )
            .push(
                text_input(m.source_password, &st.password)
                    .secure(true)
                    .on_input(Message::UrlPasswordChanged)
                    .padding([8, 12])
                    .size(14)
                    .style(theme::modal_text_input),
            );
    }

    if !app.settings.recent_url_images.is_empty() {
        let mut recent_list = column![].spacing(6);
        for recent in app.settings.recent_url_images.iter().rev() {
            let file_part = recent.rsplit('/').next().unwrap_or(recent);
            recent_list = recent_list.push(
                button(
                    text(format!("{file_part} - {recent}"))
                        .size(13)
                        .color(MODAL_TEXT),
                )
                .padding([6, 8])
                .width(Length::Fill)
                .style(theme::drive_row_button(false, false, false))
                .on_press(Message::UrlInputChanged(recent.clone())),
            );
        }
        col = col
            .push(text(m.source_recent).font(FONT_SEMIBOLD).size(16).color(MODAL_TEXT))
            .push(scrollable(recent_list).height(Length::Fill));
    }

    let cancel_btn = button(
        container(text(m.cancel).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(150.0))
    .height(Length::Fixed(42.0))
    .style(theme::outline_pill_button)
    .on_press(Message::CloseModal);

    let ok_label = if st.loading { "Loading..." } else { m.ok };
    let mut ok_btn = button(
        container(text(ok_label).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(180.0))
    .height(Length::Fixed(42.0))
    .style(theme::primary_pill_button);

    if !st.loading && !st.url.trim().is_empty() {
        ok_btn = ok_btn.on_press(Message::SubmitUrl);
    }

    let footer = container(row![cancel_btn, ok_btn].spacing(20))
        .center_x(Length::Fill)
        .into();

    modal_shell(660.0, 410.0, header, col.into(), footer)
}

fn view_image_details<'a>(app: &'a EtcherApp, locale: Locale) -> Element<'a, Message> {
    let m = messages(locale);
    let img = app.store.selected_image.as_ref();
    let name = img
        .and_then(|i| i.name.clone().or_else(|| Some(i.display_name.clone())))
        .unwrap_or_default();
    let path = img.map(|i| i.path.clone()).unwrap_or_default();

    let header = text(m.source_image)
        .font(FONT_SEMIBOLD)
        .size(22)
        .color(MODAL_TEXT)
        .into();

    let body = column![
        row![
            text(m.source_name).font(FONT_SEMIBOLD).size(14).color(MODAL_TEXT),
            text(name).size(14).color(MODAL_TEXT),
        ]
        .spacing(6),
        row![
            text(m.source_path).font(FONT_SEMIBOLD).size(14).color(MODAL_TEXT),
            text(path).size(14).color(MODAL_TEXT),
        ]
        .spacing(6),
    ]
    .spacing(12)
    .into();

    let ok_btn = button(
        container(text(m.ok).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(180.0))
    .height(Length::Fixed(42.0))
    .style(theme::primary_pill_button)
    .on_press(Message::CloseModal);

    modal_shell(560.0, 260.0, header, body, container(ok_btn).center_x(Length::Fill).into())
}

fn view_source_warning<'a>(
    title: &'a str,
    message: &'a str,
    locale: Locale,
) -> Element<'a, Message> {
    let m = messages(locale);
    let header = row![
        svg(icons::warning_triangle_icon())
            .width(20)
            .height(20)
            .style(theme::svg_tint(WARNING)),
        text(title).font(FONT_SEMIBOLD).size(20).color(MODAL_TEXT),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into();

    let body = scrollable(text(message).size(14).color(MODAL_TEXT))
        .height(Length::Fill)
        .into();

    let cancel_btn = button(
        container(text(m.cancel).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(150.0))
    .height(Length::Fixed(42.0))
    .style(theme::outline_pill_button)
    .on_press(Message::DismissSourceWarningAndClear);

    let cont_btn = button(
        container(text(m.cont).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(180.0))
    .height(Length::Fixed(42.0))
    .style(theme::warning_pill_button)
    .on_press(Message::CloseModal);

    let footer = container(row![cancel_btn, cont_btn].spacing(20))
        .center_x(Length::Fill)
        .into();

    modal_shell(560.0, 300.0, header, body, footer)
}

fn view_drive_status_warning<'a>(
    is_system: bool,
    drives: &'a [Drive],
    locale: Locale,
) -> Element<'a, Message> {
    let m = messages(locale);
    let subtitle = if is_system {
        m.drives_system_drive_warning
    } else {
        m.drives_large_drive_warning
    };
    let cta = if is_system {
        m.drives_system_drive_warning_msg
    } else {
        m.drives_large_drive_warning_msg
    };

    let header = column![
        svg(icons::warning_triangle_icon())
            .width(28)
            .height(28)
            .style(theme::svg_tint(WARNING)),
        text(m.warning).font(FONT_SEMIBOLD).size(22).color(WARNING),
    ]
    .spacing(6)
    .align_x(Alignment::Center)
    .width(Length::Fill)
    .into();

    let mut list_col = column![].spacing(6);
    for drive in drives {
        let badge_txt = if drive.is_system {
            m.compat_system
        } else {
            m.compat_large_drive
        };
        list_col = list_col.push(
            row![
                text(middle_ellipsis(&drive.description, 26).unwrap_or_else(|_| drive.description.clone()))
                    .font(FONT_SEMIBOLD)
                    .size(13)
                    .color(MODAL_TEXT)
                    .width(Length::Fill),
                text(drive.size.map(pretty_bytes).unwrap_or_default())
                    .size(13)
                    .color(MODAL_TEXT),
                container(text(badge_txt).size(11).font(FONT_SEMIBOLD))
                    .padding([2, 6])
                    .style(theme::badge_container(true, false)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }

    let body = column![
        text(subtitle).size(18).color(MODAL_TEXT),
        container(scrollable(list_col).height(Length::Fixed(84.0)))
            .padding([10, 16])
            .width(Length::Fixed(460.0))
            .style(theme::warning_box_container),
        text(cta).font(FONT_SEMIBOLD).size(15).color(MODAL_TEXT),
    ]
    .spacing(12)
    .align_x(Alignment::Center)
    .width(Length::Fill)
    .into();

    let change_btn = button(
        container(text(m.drives_change_target).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(180.0))
    .height(Length::Fixed(42.0))
    .style(theme::warning_pill_button)
    .on_press(Message::ChangeTargetFromWarning);

    let sure_btn = button(
        container(text(m.sure).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(160.0))
    .height(Length::Fixed(42.0))
    .style(theme::outline_pill_button)
    .on_press(Message::ConfirmWarningAndFlash);

    let footer = container(row![sure_btn, change_btn].spacing(20))
        .center_x(Length::Fill)
        .into();

    modal_shell(580.0, 380.0, header, body, footer)
}

fn view_flash_error<'a>(msg: &'a str, locale: Locale) -> Element<'a, Message> {
    let m = messages(locale);
    let header = text(m.attention)
        .font(FONT_SEMIBOLD)
        .size(22)
        .color(MODAL_TEXT)
        .into();

    let body = scrollable(text(msg).size(14).color(MODAL_TEXT))
        .height(Length::Fill)
        .into();

    let cancel_btn = button(
        container(text(m.cancel).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(140.0))
    .height(Length::Fixed(42.0))
    .style(theme::outline_pill_button)
    .on_press(Message::ErrorDismissAndClear);

    let retry_btn = button(
        container(text(m.retry).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(140.0))
    .height(Length::Fixed(42.0))
    .style(theme::primary_pill_button)
    .on_press(Message::ErrorRetry);

    let footer = container(row![cancel_btn, retry_btn].spacing(18))
        .center_x(Length::Fill)
        .into();

    modal_shell(460.0, 270.0, header, body, footer)
}

fn view_failed_targets<'a>(errors: &'a [FlashError], locale: Locale) -> Element<'a, Message> {
    let m = messages(locale);
    let header = text(m.flash_failed_target)
        .font(FONT_SEMIBOLD)
        .size(22)
        .color(MODAL_TEXT)
        .into();

    let table_hdr = container(
        row![
            text(m.flash_target)
                .font(FONT_SEMIBOLD)
                .size(13)
                .color(MODAL_TEXT)
                .width(Length::FillPortion(3)),
            text(m.flash_location)
                .font(FONT_SEMIBOLD)
                .size(13)
                .color(MODAL_TEXT)
                .width(Length::FillPortion(2)),
            text(m.flash_error)
                .font(FONT_SEMIBOLD)
                .size(13)
                .color(MODAL_TEXT)
                .width(Length::FillPortion(5)),
        ]
        .padding([8, 12]),
    )
    .style(theme::table_header_container)
    .width(Length::Fill);

    let mut rows = column![].width(Length::Fill);
    for err in errors {
        let err_txt = if err.message.is_empty() {
            err.code.as_deref().unwrap_or("Unknown error")
        } else {
            err.message.as_str()
        };
        rows = rows.push(
            container(
                row![
                    text(&err.description).size(13).color(MODAL_TEXT).width(Length::FillPortion(3)),
                    text(&err.device).size(13).color(MODAL_TEXT).width(Length::FillPortion(2)),
                    text(err_txt).size(13).color(MODAL_TEXT).width(Length::FillPortion(5)),
                ]
                .padding([8, 12]),
            )
            .width(Length::Fill),
        );
    }

    let body = column![table_hdr, scrollable(rows).height(Length::Fill)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    let cancel_btn = button(
        container(text(m.cancel).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(150.0))
    .height(Length::Fixed(42.0))
    .style(theme::outline_pill_button)
    .on_press(Message::CloseModal);

    let retry_btn = button(
        container(text(m.flash_failed_retry).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(42.0))
    .style(theme::primary_pill_button)
    .on_press(Message::RetryFailedTargets);

    let footer = container(row![cancel_btn, retry_btn].spacing(20))
        .center_x(Length::Fill)
        .into();

    modal_shell(720.0, 390.0, header, body, footer)
}

fn view_settings<'a>(app: &'a EtcherApp, locale: Locale) -> Element<'a, Message> {
    let m = messages(locale);
    let header = text(m.settings_settings)
        .font(FONT_SEMIBOLD)
        .size(22)
        .color(MODAL_TEXT)
        .into();

    let t1 = toggler(app.settings.error_reporting)
        .label(m.settings_error_reporting)
        .on_toggle(Message::ToggleErrorReporting)
        .size(20)
        .text_size(14)
        .style(theme::modal_toggler);

    let t2 = toggler(app.settings.auto_blockmapping)
        .label(m.settings_trim_ext_partitions)
        .on_toggle(Message::ToggleAutoBlockmapping)
        .size(20)
        .text_size(14)
        .style(theme::modal_toggler);

    let t3 = toggler(app.settings.desktop_notifications)
        .label(m.settings_desktop_notifications)
        .on_toggle(Message::ToggleDesktopNotifications)
        .size(20)
        .text_size(14)
        .style(theme::modal_toggler);

    let t4 = toggler(app.settings.updates_enabled)
        .label(m.settings_auto_update)
        .on_toggle(Message::ToggleAutoUpdates)
        .size(20)
        .text_size(14)
        .style(theme::modal_toggler);

    let lang_row = row![
        text(format!("{}:", m.settings_language))
            .font(FONT_SEMIBOLD)
            .size(14)
            .color(MODAL_TEXT),
        lang_btn(Locale::En, locale),
        lang_btn(Locale::ZhCn, locale),
        lang_btn(Locale::ZhTw, locale),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mut body_col = column![t1, t2, t3, t4, lang_row].spacing(12);

    if let (Ok(addr), Ok(_key)) = (
        std::env::var("BALENA_SUPERVISOR_ADDRESS"),
        std::env::var("BALENA_SUPERVISOR_API_KEY"),
    ) && !addr.is_empty()
    {
        let uuid = std::env::var("BALENA_DEVICE_UUID")
            .unwrap_or_else(|_| "NO-UUID".to_string());
        let short_uuid: String = uuid.chars().take(7).collect();
        body_col = body_col.push(
            column![
                text(m.settings_system_information)
                    .font(FONT_SEMIBOLD)
                    .size(18)
                    .color(MODAL_TEXT),
                text(format!("UUID: {short_uuid}")).size(13).color(MODAL_TEXT),
            ]
            .spacing(4),
        );
    }

    let version_link = button(
        row![
            svg(icons::github_icon())
                .width(14)
                .height(14)
                .style(theme::svg_tint(PRIMARY)),
            text(env!("CARGO_PKG_VERSION")).size(14).color(PRIMARY),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([4, 0])
    .style(theme::plain_button(PRIMARY, MODAL_TEXT))
    .on_press(Message::OpenExternal(
        "https://github.com/verityboss99/balena-rust/releases".to_string(),
    ));

    let check_btn = button(
        text(if app.checking_updates {
            "Checking..."
        } else {
            "Check for updates"
        })
        .size(12)
        .font(FONT_SEMIBOLD),
    )
    .padding([4, 10])
    .style(theme::outline_pill_button)
    .on_press_maybe((!app.checking_updates).then_some(Message::CheckForUpdates));

    let mut version_row = row![version_link, check_btn]
        .spacing(12)
        .align_y(Alignment::Center);

    if let Some(info) = &app.update_info {
        if info.update_available {
            version_row = version_row.push(
                button(
                    text(format!("v{} available", info.latest_version))
                        .size(12)
                        .font(FONT_SEMIBOLD)
                        .color(Color::WHITE),
                )
                .padding([4, 10])
                .style(theme::primary_pill_button)
                .on_press(Message::OpenExternal(info.html_url.clone())),
            );
        } else {
            version_row = version_row.push(text("Up to date").size(12).color(TEXT_MUTED));
        }
    } else if let Some(err) = &app.update_error {
        version_row = version_row.push(text(err.as_str()).size(12).color(FAIL_RED));
    }

    body_col = body_col.push(version_row);

    let ok_btn = button(
        container(text(m.ok).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(180.0))
    .height(Length::Fixed(42.0))
    .style(theme::primary_pill_button)
    .on_press(Message::CloseModal);

    modal_shell(
        680.0,
        390.0,
        header,
        scrollable(body_col).height(Length::Fill).into(),
        container(ok_btn).center_x(Length::Fill).into(),
    )
}

fn lang_btn<'a>(target: Locale, active: Locale) -> Element<'a, Message> {
    let is_active = target == active;
    button(text(target.display_name()).size(13).font(FONT_SEMIBOLD))
        .padding([5, 12])
        .style(if is_active {
            theme::primary_pill_button
        } else {
            theme::outline_pill_button
        })
        .on_press(Message::SetLocale(target))
        .into()
}

fn view_exit_confirm<'a>(locale: Locale) -> Element<'a, Message> {
    let m = messages(locale);
    let header = row![
        svg(icons::warning_triangle_icon())
            .width(20)
            .height(20)
            .style(theme::svg_tint(WARNING)),
        text(m.really_exit)
            .font(FONT_SEMIBOLD)
            .size(19)
            .color(MODAL_TEXT),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into();

    let body = text(m.msg_exit_while_flashing)
        .size(14)
        .color(MODAL_TEXT)
        .into();

    let cancel_btn = button(
        container(text(m.cancel).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(150.0))
    .height(Length::Fixed(42.0))
    .style(theme::outline_pill_button)
    .on_press(Message::CloseModal);

    let quit_btn = button(
        container(text(m.yes_exit).font(FONT_SEMIBOLD).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(160.0))
    .height(Length::Fixed(42.0))
    .style(theme::danger_pill_button)
    .on_press(Message::ConfirmForceExit);

    let footer = container(row![cancel_btn, quit_btn].spacing(20))
        .center_x(Length::Fill)
        .into();

    modal_shell(500.0, 240.0, header, body, footer)
}
