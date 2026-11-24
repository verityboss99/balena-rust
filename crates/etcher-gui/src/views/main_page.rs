use iced::widget::{button, column, container, row, space, svg, text};
use iced::{Alignment, Color, Element, Length};

use etcher_core::i18n::{Locale, messages};

use crate::app::{EtcherApp, Message};
use crate::icons;
use crate::theme::{self, PRIMARY, TEXT_LIGHT_SOFT, TEXT_MUTED, WARNING};
use crate::views::{flash_step, source_selector, target_selector};

pub fn view(app: &EtcherApp, locale: Locale) -> Element<'_, Message> {
    let m = messages(locale);
    let drive_line_active = app.store.has_image();
    let flash_line_active = app.store.has_image() && app.store.has_drive();
    let has_system_warning = app.store.selected_drives().iter().any(|d| d.is_system);

    let line1 = container(
        container(space().width(Length::Fixed(72.0)).height(Length::Fixed(2.0)))
            .style(theme::step_border_style(drive_line_active)),
    )
    .padding(iced::Padding {
        top: 19.0,
        ..Default::default()
    });

    let line2 = container(
        container(space().width(Length::Fixed(72.0)).height(Length::Fixed(2.0)))
            .style(theme::step_border_style(flash_line_active)),
    )
    .padding(iced::Padding {
        top: 19.0,
        ..Default::default()
    });

    let steps_row = row![
        source_selector::view(app, locale),
        line1,
        target_selector::view(app, locale),
        line2,
        flash_step::view(app, locale),
    ]
    .spacing(6)
    .align_y(Alignment::Start);

    let mut content = column![
        space().height(Length::Fixed(52.0)),
        container(steps_row).center_x(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center);

    if has_system_warning && !app.store.is_flashing {
        content = content.push(
            container(
                text(format!("Warning: {}", m.msg_system_drive))
                    .size(13)
                    .color(WARNING),
            )
            .padding([8, 0]),
        );
    }

    content = content.push(space().height(Length::Fill));

    if app.settings.analytics_alert_visible {
        let banner = container(
            row![
                column![
                    row![
                        text(m.privacy_line1_prefix).size(12).color(TEXT_LIGHT_SOFT),
                        button(text(m.privacy_settings_link).size(12))
                            .padding(0)
                            .style(theme::plain_button(PRIMARY, Color::WHITE))
                            .on_press(Message::OpenSettings),
                        text(".").size(12).color(TEXT_LIGHT_SOFT),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text(m.privacy_line2_prefix).size(12).color(TEXT_LIGHT_SOFT),
                        button(text(m.privacy_policy_link).size(12))
                            .padding(0)
                            .style(theme::plain_button(PRIMARY, Color::WHITE))
                            .on_press(Message::OpenExternal(
                                "https://www.balena.io/privacy-policy".to_string()
                            )),
                        text(".").size(12).color(TEXT_LIGHT_SOFT),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(2)
                .width(Length::Fill),
                button(
                    svg(icons::close_x_icon())
                        .width(12)
                        .height(12)
                        .style(theme::svg_tint(TEXT_LIGHT_SOFT)),
                )
                .padding(4)
                .style(theme::plain_button(TEXT_LIGHT_SOFT, TEXT_MUTED))
                .on_press(Message::DismissAnalyticsAlert),
            ]
            .align_y(Alignment::Center)
            .spacing(12),
        )
        .width(Length::Fixed(690.0))
        .padding([10, 14])
        .style(theme::analytics_banner);

        content = content.push(
            container(banner)
                .padding(iced::Padding {
                    bottom: 18.0,
                    ..Default::default()
                })
                .center_x(Length::Fill),
        );
    }

    let inner = container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([0, 24]);

    if app.is_file_hovered {
        container(inner)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .style(theme::drop_highlight_container)
            .into()
    } else {
        inner.into()
    }
}
