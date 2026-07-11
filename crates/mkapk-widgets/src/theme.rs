use mkapk_core::{Color, Insetsf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub background: Color,
    pub surface: Color,
    pub primary: Color,
    pub text: Color,
    pub border: Color,
    pub border_width: f32,
    pub corner_radius: f32,
    pub padding: Insetsf,
    pub font_size: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::new(30, 30, 30, 255),
            surface: Color::new(50, 50, 50, 255),
            primary: Color::new(100, 180, 255, 255),
            text: Color::new(230, 230, 230, 255),
            border: Color::new(80, 80, 80, 255),
            border_width: 1.0,
            corner_radius: 4.0,
            padding: Insetsf::uniform(8.0),
            font_size: 14.0,
        }
    }
}
