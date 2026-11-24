use iced::widget::{button, column, container, row, svg, text, tooltip};
use iced::{Alignment, Color, Element, Length};

use etcher_core::drive_constraints::{
    CompatibilityStatusCode, get_drive_image_compatibility_statuses,
};
use etcher_core::i18n::{Locale, messages};
use etcher_core::units::{middle_ellipsis, pretty_bytes};

use crate::app::{EtcherApp, Message};
use crate::icons;
use crate::theme::{
    self, FG_DISABLED, FONT_SEMIBOLD, PRIMARY, TEXT_HOVER_MUTED, WARNING,
};

pub fn view(app: &EtcherApp, locale: Locale) -> Element<'_, Message> {
    let m = messages(locale);
    let disabled = !app.store.has_image();
    let is_flashing = app.store.is_flashing;
    let targets = app.store.selected_drives();
    let image = app.store.selected_image.as_ref();

    let icon_color = if disabled {
        Color::from_rgba(1.0, 1.0, 1.0, 0.4)
    } else {
        Color::WHITE
    };

    let top_icon = container(
        svg(icons::drive_icon())
            .width(40)
            .height(40)
            .style(theme::svg_tint(icon_color)),
    )
    .padding(iced::Padding {
        bottom: 24.0,
        ..Default::default()
    });

    let body: Element<'_, Message> = if targets.len() == 1 {
        let target = &targets[0];
        let statuses = get_drive_image_compatibility_statuses(target, image, true, locale);
        let warn_msg: Vec<String> = statuses
            .iter()
            .filter_map(|s| match s.code {
                CompatibilityStatusCode::ContainsImage => Some(m.msg_source_drive.to_string()),
                CompatibilityStatusCode::Large => Some(m.msg_large_drive_size.to_string()),
                CompatibilityStatusCode::System => Some(m.msg_system_drive.to_string()),
                _ => None,
            })
            .collect();
        let has_system = statuses
            .iter()
            .any(|s| s.code == CompatibilityStatusCode::System);

        let short = middle_ellipsis(&target.description, 20)
            .unwrap_or_else(|_| target.description.clone());
        let tip = format!("{} ({})", target.description, target.display_name);

        let mut name_row = row![].spacing(6).align_y(Alignment::Center);
        if !warn_msg.is_empty() {
            name_row = name_row.push(tooltip(
                svg(icons::warning_triangle_icon())
                    .width(14)
                    .height(14)
                    .style(theme::svg_tint(if has_system {
                        WARNING
                    } else {
                        TEXT_HOVER_MUTED
                    })),
                container(text(warn_msg.join(", ")).size(12))
                    .padding([4, 8])
                    .style(theme::tooltip_container),
                tooltip::Position::Top,
            ));
        }
        name_row = name_row.push(text(short).font(FONT_SEMIBOLD).size(16).color(Color::WHITE));

        let name_widget = tooltip(
            container(name_row).padding([6, 4]),
            container(text(tip).size(12))
                .padding([4, 8])
                .style(theme::tooltip_container),
            tooltip::Position::Bottom,
        );

        let mut col = column![name_widget].align_x(Alignment::Center).spacing(4);
        if !is_flashing {
            col = col.push(
                button(text(m.target_change).size(14))
                    .padding([2, 8])
                    .style(theme::plain_button(PRIMARY, TEXT_HOVER_MUTED))
                    .on_press(Message::OpenTargetSelector),
            );
        }
        if let Some(sz) = target.size {
            col = col.push(text(pretty_bytes(sz)).size(13).color(FG_DISABLED));
        }
        col.into()
    } else if targets.len() > 1 {
        let mut col = column![
            text(format!("{} {}", targets.len(), m.target_targets))
                .font(FONT_SEMIBOLD)
                .size(16)
                .color(Color::WHITE)
        ]
        .align_x(Alignment::Center)
        .spacing(4);

        if !is_flashing {
            col = col.push(
                button(text(m.target_change).size(14))
                    .padding([2, 8])
                    .style(theme::plain_button(PRIMARY, TEXT_HOVER_MUTED))
                    .on_press(Message::OpenTargetSelector),
            );
        }

        for target in &targets {
            let short = middle_ellipsis(&target.description, 14)
                .unwrap_or_else(|_| target.description.clone());
            let sz_txt = target.size.map(pretty_bytes).unwrap_or_default();
            col = col.push(
                row![
                    text(short).size(12).color(FG_DISABLED),
                    text(sz_txt).size(12).color(FG_DISABLED),
                ]
                .spacing(6),
            );
        }
        col.into()
    } else {
        let mut btn = button(
            container(text(m.target_select_target).font(FONT_SEMIBOLD).size(15))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .padding(0)
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(48.0))
        .style(theme::primary_pill_button);

        if !disabled {
            btn = btn.on_press(Message::OpenTargetSelector);
        }
        btn.into()
    };

    column![top_icon, body]
        .width(Length::Fixed(200.0))
        .align_x(Alignment::Center)
        .into()
}
