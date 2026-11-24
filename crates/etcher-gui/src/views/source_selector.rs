use iced::widget::{button, column, container, mouse_area, row, svg, text, tooltip};
use iced::{Alignment, Color, Element, Length};

use etcher_core::i18n::{Locale, messages};
use etcher_core::units::{middle_ellipsis, pretty_bytes};

use crate::app::{EtcherApp, Message};
use crate::icons;
use crate::theme::{
    self, FG_DISABLED, FONT_REGULAR, FONT_SEMIBOLD, PRIMARY, TEXT_HOVER_MUTED,
};

pub fn view(app: &EtcherApp, locale: Locale) -> Element<'_, Message> {
    let m = messages(locale);
    let selected_image = app.store.selected_image.as_ref();
    let is_flashing = app.store.is_flashing;

    let top_icon = container(
        svg(icons::image_icon())
            .width(40)
            .height(40)
            .style(theme::svg_tint(Color::WHITE)),
    )
    .padding(iced::Padding {
        bottom: 24.0,
        ..Default::default()
    });

    let body: Element<'_, Message> = if let Some(img) = selected_image {
        let raw_name = img
            .drive
            .as_ref()
            .map(|d| d.description.as_str())
            .or(img.name.as_deref())
            .unwrap_or(&img.display_name);
        let short_name = middle_ellipsis(raw_name, 20).unwrap_or_else(|_| raw_name.to_string());

        let name_btn = tooltip(
            button(
                text(short_name)
                    .font(FONT_SEMIBOLD)
                    .size(16)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .width(Length::Fixed(200.0))
            .padding([6, 8])
            .style(theme::plain_button(Color::WHITE, TEXT_HOVER_MUTED))
            .on_press(Message::ShowImageDetails),
            container(text(raw_name.to_string()).size(12))
                .padding([4, 8])
                .style(theme::tooltip_container),
            tooltip::Position::Bottom,
        );

        let mut col = column![name_btn].align_x(Alignment::Center).spacing(4);

        if !is_flashing {
            col = col.push(
                button(text(m.cancel).size(14))
                    .padding([2, 8])
                    .style(theme::plain_button(PRIMARY, TEXT_HOVER_MUTED))
                    .on_press(Message::ClearSource),
            );
        }

        if let Some(sz) = img.size {
            col = col.push(
                text(pretty_bytes(sz))
                    .size(13)
                    .color(FG_DISABLED),
            );
        }

        col.into()
    } else if app.image_loading {
        column![
            text("Loading...")
                .size(16)
                .color(Color::WHITE),
        ]
        .align_x(Alignment::Center)
        .into()
    } else {
        let active_flow = app.hovered_flow.unwrap_or(0);
        let from_file = flow_button(
            icons::file_icon(),
            m.source_from_file,
            active_flow == 0,
            Message::OpenFileSelector,
            0,
        );
        let from_url = flow_button(
            icons::link_icon(),
            m.source_from_url,
            active_flow == 1,
            Message::OpenUrlSelector,
            1,
        );
        let clone_drive = flow_button(
            icons::copy_icon(),
            m.source_clone,
            active_flow == 2,
            Message::OpenSourceDriveSelector,
            2,
        );

        column![from_file, from_url, clone_drive]
            .spacing(6)
            .align_x(Alignment::Center)
            .into()
    };

    column![top_icon, body]
        .width(Length::Fixed(200.0))
        .align_x(Alignment::Center)
        .into()
}

fn flow_button<'a>(
    icon_handle: svg::Handle,
    label: &'a str,
    primary: bool,
    on_press: Message,
    index: usize,
) -> Element<'a, Message> {
    let icon_color = if primary {
        Color::WHITE
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.72)
    };
    let btn = button(
        container(
            row![
                svg(icon_handle)
                    .width(14)
                    .height(14)
                    .style(theme::svg_tint(icon_color)),
                text(label)
                    .size(14)
                    .font(if primary { FONT_SEMIBOLD } else { FONT_REGULAR }),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .padding(0)
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(48.0))
    .style(theme::flow_selector_button(primary))
    .on_press(on_press);

    mouse_area(btn)
        .on_enter(Message::SourceFlowHover(Some(index)))
        .on_exit(Message::SourceFlowHover(None))
        .into()
}
