use ratatui::style::Color;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub surface: Color,
    pub overlay: Color,
    pub text: Color,
    pub subtext: Color,
    pub muted: Color,
    pub accent: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub border: Color,
}

impl Palette {
    pub fn mocha() -> Self {
        Self {
            bg: Color::Rgb(30, 30, 46),
            surface: Color::Rgb(49, 50, 68),
            overlay: Color::Rgb(69, 71, 90),
            text: Color::Rgb(205, 214, 244),
            subtext: Color::Rgb(166, 173, 200),
            muted: Color::Rgb(108, 112, 134),
            accent: Color::Rgb(137, 180, 250),
            green: Color::Rgb(166, 227, 161),
            yellow: Color::Rgb(249, 226, 175),
            red: Color::Rgb(243, 139, 168),
            border: Color::Rgb(69, 71, 90),
        }
    }
}
