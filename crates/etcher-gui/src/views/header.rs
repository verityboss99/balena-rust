use iced::widget::{button, row, space, svg};
use iced::{Alignment, Color, Element, Length};

use crate::app::Message;
use crate::icons;
use crate::theme::{self, TEXT_HOVER_MUTED};

pub fn view(disable_external_links: bool) -> Element<'static, Message> {
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

    row![
        space().width(Length::FillPortion(1)),
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
