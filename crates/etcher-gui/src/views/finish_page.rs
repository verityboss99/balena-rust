use iced::widget::{button, column, container, row, space, stack, svg, text, tooltip};
use iced::{Alignment, Color, Element, Length};

use etcher_core::i18n::{
    Locale, flash_speed, messages, progress_failed, progress_successful,
};
use etcher_core::units::middle_ellipsis;

use crate::app::{EtcherApp, Message};
use crate::icons;
use crate::theme::{self, FAIL_RED, FONT_SEMIBOLD, PRIMARY, SUCCESS, TEXT_MUTED};

pub fn view(app: &EtcherApp, locale: Locale) -> Element<'_, Message> {
    let m = messages(locale);
    let results = app.store.flash_results.as_ref();
    let skip = results.map(|r| r.skip).unwrap_or(false);
    let successful = results.map(|r| r.successful_devices).unwrap_or(0);
    let failed_count = results.map(|r| r.failed_devices).unwrap_or(0);
    let errors_len = results.map(|r| r.errors.len()).unwrap_or(0);
    let all_failed = !skip && successful == 0;
    let some_failed = failed_count != 0 || errors_len != 0;

    let badge_color = if all_failed || some_failed {
        Color::from_rgb(0xc6 as f32 / 255.0, 0xc8 as f32 / 255.0, 0xc9 as f32 / 255.0)
    } else {
        SUCCESS
    };

    let done_badge = svg(if all_failed && !skip {
        icons::times_circle_icon()
    } else {
        icons::check_circle_icon()
    })
    .width(24)
    .height(24)
    .style(theme::svg_tint(badge_color));

    let flash_glyph = stack![
        container(
            svg(icons::flash_icon())
                .width(40)
                .height(40)
                .style(theme::svg_tint(Color::from_rgba(1.0, 1.0, 1.0, 0.4))),
        )
        .padding([0, 12]),
        container(done_badge)
            .align_right(Length::Fill)
            .align_bottom(Length::Fill),
    ]
    .width(Length::Fixed(64.0))
    .height(Length::Fixed(48.0));

    let img_name = app
        .store
        .selected_image
        .as_ref()
        .and_then(|i| i.name.clone().or_else(|| Some(i.display_name.clone())))
        .unwrap_or_default();
    let short_img = middle_ellipsis(&img_name, 24).unwrap_or(img_name);

    let title = if all_failed {
        m.flash_failed
    } else {
        m.flash_completed
    };

    let mut summary_col = column![
        flash_glyph,
        text(short_img).size(14).color(TEXT_MUTED),
        text(title).font(FONT_SEMIBOLD).size(24).color(Color::WHITE),
    ]
    .spacing(10)
    .align_x(Alignment::Center);

    if skip {
        summary_col = summary_col.push(text(m.flash_skip).size(14).color(TEXT_MUTED));
    }

    let mut counts_col = column![].spacing(6).align_x(Alignment::Center);

    if successful > 0 {
        counts_col = counts_col.push(
            row![
                svg(icons::circle_dot_icon())
                    .width(12)
                    .height(12)
                    .style(theme::svg_tint(SUCCESS)),
                text(successful.to_string()).size(14).color(Color::WHITE),
                text(progress_successful(locale, successful))
                    .size(14)
                    .color(TEXT_MUTED),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }

    if errors_len > 0 {
        counts_col = counts_col.push(
            row![
                svg(icons::circle_dot_icon())
                    .width(12)
                    .height(12)
                    .style(theme::svg_tint(FAIL_RED)),
                text(errors_len.to_string()).size(14).color(Color::WHITE),
                text(progress_failed(locale, errors_len))
                    .size(14)
                    .color(TEXT_MUTED),
                button(text(m.flash_more_info).size(14))
                    .padding([0, 4])
                    .style(theme::plain_button(PRIMARY, Color::WHITE))
                    .on_press(Message::ShowFailedTargetsModal),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }

    if !all_failed {
        let eff_mb = app.store.effective_speed_mb_per_sec();
        counts_col = counts_col.push(tooltip(
            text(flash_speed(locale, eff_mb)).size(12).color(TEXT_MUTED),
            container(text(m.flash_speed_tip).size(12))
                .padding([6, 10])
                .style(theme::tooltip_container),
            tooltip::Position::Bottom,
        ));
    }

    let flash_another_btn = button(
        container(text(m.flash_another).font(FONT_SEMIBOLD).size(15))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(48.0))
    .style(theme::primary_pill_button)
    .on_press(Message::FlashAnother);

    column![
        space().height(Length::FillPortion(1)),
        summary_col,
        counts_col,
        container(flash_another_btn).padding(iced::Padding {
            top: 20.0,
            ..Default::default()
        }),
        space().height(Length::FillPortion(1)),
    ]
    .spacing(14)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .into()
}
