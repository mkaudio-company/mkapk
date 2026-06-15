#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Px(f32);

impl Px {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> f32 {
        self.0
    }

    pub fn from_dp(dp: Dp, scale: f32) -> Self {
        Self::new(dp.get() * scale)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Dp(f32);

impl Dp {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> f32 {
        self.0
    }

    pub fn from_px(px: Px, scale: f32) -> Self {
        Self::new(px.get() / scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_from_dp_and_roundtrip() {
        let dp = Dp::new(10.0);
        let px = Px::from_dp(dp, 2.0);
        assert_eq!(px.get(), 20.0);
        assert_eq!(Dp::from_px(px, 2.0).get(), 10.0);
    }

    #[test]
    fn dp_from_px() {
        let px = Px::new(96.0);
        let dp = Dp::from_px(px, 2.0);
        assert_eq!(dp.get(), 48.0);
    }
}
