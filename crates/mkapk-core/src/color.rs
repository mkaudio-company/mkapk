#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    #[allow(clippy::manual_clamp)]
    pub fn from_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        fn channel(value: f32) -> u8 {
            let clamped = if value < 0.0 {
                0.0
            } else if value > 1.0 {
                1.0
            } else {
                value
            };
            (clamped * 255.0 + 0.5) as u8
        }
        Self::new(channel(r), channel(g), channel(b), channel(a))
    }

    pub fn to_premultiplied_f32(&self) -> [f32; 4] {
        let a = self.a as f32 / 255.0;
        let r = self.r as f32 / 255.0 * a;
        let g = self.g as f32 / 255.0 * a;
        let b = self.b as f32 / 255.0 * a;
        [r, g, b, a]
    }
}

pub const BLACK: Color = Color::new(0, 0, 0, 255);
pub const WHITE: Color = Color::new(255, 255, 255, 255);
pub const RED: Color = Color::new(255, 0, 0, 255);
pub const GREEN: Color = Color::new(0, 255, 0, 255);
pub const BLUE: Color = Color::new(0, 0, 255, 255);
pub const TRANSPARENT: Color = Color::new(0, 0, 0, 0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_new_and_rgb() {
        let c = Color::new(10, 20, 30, 40);
        assert_eq!(c.r, 10);
        assert_eq!(c.g, 20);
        assert_eq!(c.b, 30);
        assert_eq!(c.a, 40);
        assert_eq!(Color::from_rgb(1, 2, 3), Color::new(1, 2, 3, 255));
    }

    #[test]
    fn color_from_f32() {
        let c = Color::from_f32(1.0, 0.5, 0.0, 1.0);
        assert_eq!(c, Color::new(255, 128, 0, 255));
    }

    #[test]
    fn color_to_premultiplied() {
        let c = Color::new(128, 0, 0, 128);
        let [r, g, b, a] = c.to_premultiplied_f32();
        let alpha = 128.0 / 255.0;
        let expected_red = alpha * alpha;
        assert!((r - expected_red).abs() < 1e-5);
        assert_eq!(g, 0.0);
        assert_eq!(b, 0.0);
        assert!((a - alpha).abs() < 1e-5);
    }

    #[test]
    fn color_constants() {
        assert_eq!(RED, Color::new(255, 0, 0, 255));
        assert_eq!(TRANSPARENT, Color::new(0, 0, 0, 0));
    }
}
