use iced::border;
use iced::font::{Family, Weight};
use iced::widget::{button, checkbox, container, progress_bar, svg, text_input, toggler};
use iced::{Background, Border, Color, Font, Shadow, Theme, Vector};

pub const SOURCE_SANS_REGULAR_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/SourceSansPro-Regular.ttf");
pub const SOURCE_SANS_SEMIBOLD_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/SourceSansPro-SemiBold.ttf");

pub const FONT_REGULAR: Font = Font {
    family: Family::Name("Source Sans Pro"),
    weight: Weight::Normal,
    ..Font::DEFAULT
};

pub const FONT_SEMIBOLD: Font = Font {
    family: Family::Name("Source Sans Pro"),
    weight: Weight::Semibold,
    ..Font::DEFAULT
};

pub const BG_DARK: Color = Color::from_rgb(0x4d as f32 / 255.0, 0x50 as f32 / 255.0, 0x57 as f32 / 255.0);
pub const BG_DISABLED: Color = Color::from_rgb(0x3a as f32 / 255.0, 0x3c as f32 / 255.0, 0x41 as f32 / 255.0);
pub const FG_DISABLED: Color = Color::from_rgb(0x78 as f32 / 255.0, 0x7c as f32 / 255.0, 0x7f as f32 / 255.0);
pub const TEXT_MUTED: Color = Color::from_rgb(0x7e as f32 / 255.0, 0x80 as f32 / 255.0, 0x85 as f32 / 255.0);
pub const TEXT_LIGHT_SOFT: Color = Color::from_rgb(0xdd as f32 / 255.0, 0xdd as f32 / 255.0, 0xdd as f32 / 255.0);
pub const TEXT_HOVER_MUTED: Color = Color::from_rgb(0x8f as f32 / 255.0, 0x92 as f32 / 255.0, 0x97 as f32 / 255.0);

pub const PRIMARY: Color = Color::from_rgb(0x00 as f32 / 255.0, 0xae as f32 / 255.0, 0xef as f32 / 255.0);
pub const PRIMARY_HOVER: Color = Color::from_rgb(0x00 as f32 / 255.0, 0x95 as f32 / 255.0, 0xcc as f32 / 255.0);
pub const WARNING: Color = Color::from_rgb(0xfc as f32 / 255.0, 0xa3 as f32 / 255.0, 0x21 as f32 / 255.0);
pub const WARNING_HOVER: Color = Color::from_rgb(0xe0 as f32 / 255.0, 0x8e as f32 / 255.0, 0x15 as f32 / 255.0);
pub const DANGER: Color = Color::from_rgb(0xd9 as f32 / 255.0, 0x53 as f32 / 255.0, 0x4f as f32 / 255.0);
pub const FAIL_RED: Color = Color::from_rgb(1.0, 0x44 as f32 / 255.0, 0x44 as f32 / 255.0);
pub const SUCCESS: Color = Color::from_rgb(0x1a as f32 / 255.0, 0xc1 as f32 / 255.0, 0x35 as f32 / 255.0);
pub const FLASHING_PURPLE: Color = Color::from_rgb(0xda as f32 / 255.0, 0x60 as f32 / 255.0, 1.0);
pub const PROGRESS_BG: Color = Color::from_rgb(0x2f as f32 / 255.0, 0x30 as f32 / 255.0, 0x33 as f32 / 255.0);

pub const MODAL_BG: Color = Color::WHITE;
pub const MODAL_TEXT: Color = Color::from_rgb(0x2a as f32 / 255.0, 0x50 as f32 / 255.0, 0x6f as f32 / 255.0);
pub const MODAL_BORDER: Color = Color::from_rgb(0xdd as f32 / 255.0, 0xe1 as f32 / 255.0, 0xf0 as f32 / 255.0);
pub const TABLE_HEADER_BG: Color = Color::from_rgb(0xf8 as f32 / 255.0, 0xf9 as f32 / 255.0, 0xfd as f32 / 255.0);
pub const SYSTEM_ROW_BG: Color = Color::from_rgb(1.0, 0xf5 as f32 / 255.0, 0xe6 as f32 / 255.0);
pub const SELECTED_ROW_BG: Color = Color::from_rgb(0xe8 as f32 / 255.0, 0xf5 as f32 / 255.0, 0xfc as f32 / 255.0);
pub const COUNT_BLUE: Color = Color::from_rgb(0x5b as f32 / 255.0, 0x82 as f32 / 255.0, 0xa7 as f32 / 255.0);

pub fn app_theme() -> Theme {
    Theme::Dark
}

pub fn dark_root_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_DARK)),
        text_color: Some(Color::WHITE),
        ..Default::default()
    }
}

pub fn drop_highlight_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.68, 0.94, 0.08))),
        border: Border {
            color: PRIMARY,
            width: 2.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

pub fn step_border_style(active: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(if active { Color::WHITE } else { FG_DISABLED })),
        ..Default::default()
    }
}

pub fn primary_pill_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Background::Color(PRIMARY)),
        text_color: Color::WHITE,
        border: border::rounded(24.0),
        ..Default::default()
    };
    match status {
        button::Status::Active => base,
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(Background::Color(PRIMARY_HOVER)),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(BG_DISABLED)),
            text_color: FG_DISABLED,
            ..base
        },
    }
}

pub fn warning_pill_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Background::Color(WARNING)),
        text_color: Color::WHITE,
        border: border::rounded(24.0),
        ..Default::default()
    };
    match status {
        button::Status::Active => base,
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(Background::Color(WARNING_HOVER)),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(BG_DISABLED)),
            text_color: FG_DISABLED,
            ..base
        },
    }
}

pub fn danger_pill_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Background::Color(DANGER)),
        text_color: Color::WHITE,
        border: border::rounded(24.0),
        ..Default::default()
    };
    match status {
        button::Status::Active => base,
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(Background::Color(Color::from_rgb(0.75, 0.22, 0.20))),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(BG_DISABLED)),
            text_color: FG_DISABLED,
            ..base
        },
    }
}

pub fn outline_pill_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Background::Color(Color::WHITE)),
        text_color: MODAL_TEXT,
        border: Border {
            color: MODAL_TEXT,
            width: 1.0,
            radius: 24.0.into(),
        },
        ..Default::default()
    };
    match status {
        button::Status::Active => base,
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(Background::Color(TABLE_HEADER_BG)),
            ..base
        },
        button::Status::Disabled => button::Style {
            text_color: FG_DISABLED,
            border: Border {
                color: FG_DISABLED,
                width: 1.0,
                radius: 24.0.into(),
            },
            ..base
        },
    }
}

pub fn flow_selector_button(active_primary: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let normal = if active_primary {
            button::Style {
                background: Some(Background::Color(PRIMARY)),
                text_color: Color::WHITE,
                border: border::rounded(24.0),
                ..Default::default()
            }
        } else {
            button::Style {
                background: None,
                text_color: Color::from_rgba(1.0, 1.0, 1.0, 0.72),
                border: border::rounded(24.0),
                ..Default::default()
            }
        };
        match status {
            button::Status::Active => normal,
            button::Status::Hovered | button::Status::Pressed => button::Style {
                background: Some(Background::Color(PRIMARY)),
                text_color: Color::WHITE,
                border: border::rounded(24.0),
                ..Default::default()
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(BG_DISABLED)),
                text_color: FG_DISABLED,
                border: border::rounded(24.0),
                ..Default::default()
            },
        }
    }
}

pub fn plain_button(
    normal_color: Color,
    hover_color: Color,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: None,
        text_color: match status {
            button::Status::Hovered | button::Status::Pressed => hover_color,
            button::Status::Disabled => FG_DISABLED,
            button::Status::Active => normal_color,
        },
        border: Border::default(),
        ..Default::default()
    }
}

pub fn modal_backdrop(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
        ..Default::default()
    }
}

pub fn modal_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(MODAL_BG)),
        text_color: Some(MODAL_TEXT),
        border: Border {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.15),
            width: 1.0,
            radius: 7.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 20.0,
        },
        ..Default::default()
    }
}

pub fn modal_footer(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(MODAL_BG)),
        border: Border {
            color: MODAL_BORDER,
            width: 1.0,
            radius: border::Radius {
                top_left: 0.0,
                top_right: 0.0,
                bottom_right: 7.0,
                bottom_left: 7.0,
            },
        },
        ..Default::default()
    }
}

pub fn analytics_banner(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(
            0x3c as f32 / 255.0,
            0x3f as f32 / 255.0,
            0x45 as f32 / 255.0,
        ))),
        text_color: Some(TEXT_LIGHT_SOFT),
        border: Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

pub fn system_danger_banner(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(WARNING)),
        text_color: Some(Color::WHITE),
        border: border::rounded(4.0),
        ..Default::default()
    }
}

pub fn warning_box_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SYSTEM_ROW_BG)),
        text_color: Some(MODAL_TEXT),
        border: border::rounded(4.0),
        ..Default::default()
    }
}

pub fn table_header_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TABLE_HEADER_BG)),
        text_color: Some(MODAL_TEXT),
        border: Border {
            color: MODAL_BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn drive_row_button(
    selected: bool,
    is_system: bool,
    disabled: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = if disabled {
            Color::from_rgba(0.95, 0.96, 0.98, 0.5)
        } else if selected {
            if is_system {
                SYSTEM_ROW_BG
            } else {
                SELECTED_ROW_BG
            }
        } else if matches!(status, button::Status::Hovered) {
            TABLE_HEADER_BG
        } else {
            Color::WHITE
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: if disabled { FG_DISABLED } else { MODAL_TEXT },
            border: Border {
                color: MODAL_BORDER,
                width: 0.5,
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn badge_container(is_error: bool, is_warning: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let (bg, fg) = if is_error {
            (
                Color::from_rgb(1.0, 0.91, 0.91),
                Color::from_rgb(0.75, 0.20, 0.18),
            )
        } else if is_warning {
            (
                Color::from_rgb(1.0, 0.95, 0.86),
                Color::from_rgb(0.76, 0.45, 0.02),
            )
        } else {
            (TABLE_HEADER_BG, COUNT_BLUE)
        };
        container::Style {
            background: Some(Background::Color(bg)),
            text_color: Some(fg),
            border: border::rounded(4.0),
            ..Default::default()
        }
    }
}

pub fn flash_progress_bar(bar_color: Color) -> impl Fn(&Theme) -> progress_bar::Style {
    move |_| progress_bar::Style {
        background: Background::Color(PROGRESS_BG),
        bar: Background::Color(bar_color),
        border: border::rounded(14.0),
    }
}

pub fn modal_checkbox(_theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let checked = matches!(
        status,
        checkbox::Status::Active { is_checked: true }
            | checkbox::Status::Hovered { is_checked: true }
            | checkbox::Status::Disabled { is_checked: true }
    );
    checkbox::Style {
        background: Background::Color(if checked { PRIMARY } else { Color::WHITE }),
        icon_color: Color::WHITE,
        border: Border {
            radius: 4.0.into(),
            width: 1.5,
            color: if checked { PRIMARY } else { MODAL_TEXT },
        },
        text_color: Some(MODAL_TEXT),
    }
}

pub fn modal_toggler(_theme: &Theme, status: toggler::Status) -> toggler::Style {
    let toggled = matches!(
        status,
        toggler::Status::Active { is_toggled: true }
            | toggler::Status::Hovered { is_toggled: true }
            | toggler::Status::Disabled { is_toggled: true }
    );
    toggler::Style {
        background: Background::Color(if toggled {
            PRIMARY
        } else {
            Color::from_rgb(0.75, 0.78, 0.82)
        }),
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: Background::Color(Color::WHITE),
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
        text_color: Some(MODAL_TEXT),
        border_radius: None,
        padding_ratio: 0.12,
    }
}

pub fn modal_text_input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let focused = matches!(status, text_input::Status::Focused { .. });
    text_input::Style {
        background: Background::Color(Color::WHITE),
        border: Border {
            color: if focused { PRIMARY } else { MODAL_BORDER },
            width: 1.5,
            radius: 4.0.into(),
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_MUTED,
        value: MODAL_TEXT,
        selection: Color::from_rgba(0.0, 0.68, 0.94, 0.25),
    }
}

pub fn tooltip_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(0.18, 0.20, 0.24))),
        text_color: Some(Color::WHITE),
        border: border::rounded(4.0),
        ..Default::default()
    }
}

pub fn svg_tint(color: Color) -> impl Fn(&Theme, svg::Status) -> svg::Style {
    move |_, _| svg::Style { color: Some(color) }
}
