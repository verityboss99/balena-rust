use iced::widget::{button, column, container, progress_bar, row, svg, text};
use iced::{Alignment, Color, Element, Length};

use etcher_core::i18n::{Locale, flash_eta, flash_speed_short, messages, progress_failed, progress_from_state};
use etcher_core::types::FlashPhase;
use etcher_core::units::format_seconds;

use crate::app::{EtcherApp, Message};
use crate::icons;
use crate::theme::{
    self, FAIL_RED, FLASHING_PURPLE, FONT_SEMIBOLD, PRIMARY, SUCCESS, TEXT_HOVER_MUTED, TEXT_MUTED,
};

pub fn view(app: &EtcherApp, locale: Locale) -> Element<'_, Message> {
    let m = messages(locale);
    let disabled = !app.store.has_image() || !app.store.has_drive();
    let is_flashing = app.store.is_flashing;
    let has_system_warning = app.store.selected_drives().iter().any(|d| d.is_system);

    let icon_color = if disabled {
        Color::from_rgba(1.0, 1.0, 1.0, 0.4)
    } else {
        Color::WHITE
    };

    let top_icon = container(
        svg(icons::flash_icon())
            .width(40)
            .height(40)
            .style(theme::svg_tint(icon_color)),
    )
    .padding(iced::Padding {
        bottom: 24.0,
        ..Default::default()
    });

    let body: Element<'_, Message> = if is_flashing {
        let state = &app.store.flash_state;
        let disp = progress_from_state(locale, state.phase, state.percentage, state.position);
        let phase_color = match state.phase {
            FlashPhase::Decompressing | FlashPhase::Starting => PRIMARY,
            FlashPhase::Flashing | FlashPhase::Finishing => FLASHING_PURPLE,
            FlashPhase::Verifying => SUCCESS,
        };
        let action_label = if state.phase == FlashPhase::Verifying {
            m.skip
        } else {
            m.cancel
        };
        let action_msg = if state.phase == FlashPhase::Verifying {
            Message::SkipVerification
        } else {
            Message::CancelFlash
        };

        let header_row = row![
            row![
                text(disp.status).font(FONT_SEMIBOLD).size(15).color(Color::WHITE),
                text(disp.position.unwrap_or_default())
                    .font(FONT_SEMIBOLD)
                    .size(15)
                    .color(phase_color),
            ]
            .spacing(4)
            .width(Length::Fill),
            button(text(action_label).font(FONT_SEMIBOLD).size(13))
                .padding([0, 4])
                .style(theme::plain_button(PRIMARY, TEXT_HOVER_MUTED))
                .on_press(action_msg),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill);

        let pct = state.percentage.unwrap_or(0) as f32;
        let bar = progress_bar(0.0..=100.0, pct)
            .length(Length::Fill)
            .girth(Length::Fixed(12.0))
            .style(theme::flash_progress_bar(phase_color));

        let mut col = column![header_row, bar]
            .spacing(8)
            .width(Length::Fixed(210.0));

        if state.percentage != Some(100) && state.speed > 0.0 {
            let mut stats_row = row![
                text(flash_speed_short(locale, state.speed))
                    .size(13)
                    .color(TEXT_MUTED)
                    .width(Length::Fill),
            ];
            if let Some(eta) = state.eta
                && eta.is_finite()
            {
                let eta_str = format_seconds(eta);
                if !eta_str.is_empty() {
                    stats_row = stats_row.push(
                        text(flash_eta(locale, &eta_str))
                            .size(13)
                            .color(TEXT_MUTED),
                    );
                }
            }
            col = col.push(stats_row);
        }

        if state.failed > 0 {
            col = col.push(
                row![
                    svg(icons::circle_dot_icon())
                        .width(12)
                        .height(12)
                        .style(theme::svg_tint(FAIL_RED)),
                    text(format!("{} {}", state.failed, progress_failed(locale, state.failed)))
                        .size(13)
                        .color(Color::WHITE),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }

        col.into()
    } else {
        let mut btn = button(
            container(text(m.flash_flash_now).font(FONT_SEMIBOLD).size(15))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .padding(0)
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(48.0))
        .style(if has_system_warning && !disabled {
            theme::warning_pill_button
        } else {
            theme::primary_pill_button
        });

        if !disabled {
            btn = btn.on_press(Message::TryFlash);
        }
        btn.into()
    };

    column![top_icon, body]
        .width(Length::Fixed(210.0))
        .align_x(Alignment::Center)
        .into()
}
