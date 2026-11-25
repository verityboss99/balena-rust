use iced::widget::{button, row, space, svg, text};
use iced::{Alignment, Color, Element, Length};

use etcher_core::ipc::UpdateCheckResult;

use crate::app::Message;
use crate::icons;
use crate::theme::{self, FONT_SEMIBOLD, TEXT_HOVER_MUTED};

pub fn view<'a>(
    disable_external_links: bool,
    update_info: Option<&'a UpdateCheckResult>,
) -> Element<'a, Message> {
    let logo_btn = button(svg(icons::etcher_logo()).width(123).height(22))
        .padding(0)
        .style(theme::plain_button(Color::WHITE, TEXT_HOVER_MUTED))
        .on_press(Message::OpenExternal(
            "https://www.balena.io/etcher?ref=etcher_footer".to_string(),
        ));

    let settings_btn = button(
        svg(icons::gear_icon())
            .width(18)
            .height(18)
            .style(theme::svg_tint(Color::WHITE)),
    )
    .padding(4)
    .style(theme::plain_button(Color::WHITE, TEXT_HOVER_MUTED))
    .on_press(Message::OpenSettings);

    let mut right = row![settings_btn]
        .spacing(12)
        .align_y(Alignment::Center);

    if !disable_external_links {
        let help_btn = button(
            svg(icons::question_circle_icon())
                .width(18)
                .height(18)
                .style(theme::svg_tint(Color::WHITE)),
        )
        .padding(4)
        .style(theme::plain_button(Color::WHITE, TEXT_HOVER_MUTED))
        .on_press(Message::OpenSupportUrl);
        right = right.push(help_btn);
    }

    let left: Element<'a, Message> = if !disable_external_links
        && let Some(info) = update_info
        && info.update_available
    {
        iced::widget::container(
            button(
                text(format!("Update v{}", info.latest_version))
                    .font(FONT_SEMIBOLD)
                    .size(11)
                    .color(Color::WHITE),
            )
            .padding([3, 10])
            .style(theme::primary_pill_button)
            .on_press(Message::OpenExternal(info.html_url.clone())),
        )
        .width(Length::FillPortion(1))
        .padding([0, 16])
        .into()
    } else {
        space().width(Length::FillPortion(1)).into()
    };

    row![
        left,
        iced::widget::container(logo_btn)
            .width(Length::FillPortion(1))
            .center_x(Length::Fill),
        iced::widget::container(right)
            .width(Length::FillPortion(1))
            .align_x(iced::alignment::Horizontal::Right)
            .padding([0, 20]),
    ]
    .padding([14, 0])
    .align_y(Alignment::Center)
    .into()
}
