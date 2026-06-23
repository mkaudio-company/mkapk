use crate::geometry::Point;

const TWO_PI: f32 = 2.0 * core::f32::consts::PI;

pub(crate) fn sin_cos(x: f32) -> (f32, f32) {
    let mut theta = x % TWO_PI;
    if theta < 0.0 {
        theta += TWO_PI;
    }

    let (sign_sin, sign_cos, y) = if theta <= core::f32::consts::FRAC_PI_2 {
        (1.0, 1.0, theta)
    } else if theta <= core::f32::consts::PI {
        (1.0, -1.0, core::f32::consts::PI - theta)
    } else if theta <= 3.0 * core::f32::consts::FRAC_PI_2 {
        (-1.0, -1.0, theta - core::f32::consts::PI)
    } else {
        (-1.0, 1.0, TWO_PI - theta)
    };

    let y2 = y * y;
    let y3 = y2 * y;
    let y5 = y3 * y2;
    let y7 = y5 * y2;
    let y9 = y7 * y2;

    let sin = y - y3 / 6.0 + y5 / 120.0 - y7 / 5_040.0 + y9 / 362_880.0;

    let y4 = y2 * y2;
    let y6 = y4 * y2;
    let y8 = y6 * y2;
    let cos = 1.0 - y2 / 2.0 + y4 / 24.0 - y6 / 720.0 + y8 / 40_320.0;

    (sign_sin * sin, sign_cos * cos)
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Transform {
    pub(crate) m11: f32,
    pub(crate) m12: f32,
    pub(crate) m21: f32,
    pub(crate) m22: f32,
    pub(crate) m31: f32,
    pub(crate) m32: f32,
}

impl Transform {
    pub const fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: 0.0,
            m32: 0.0,
        }
    }

    pub const fn translate(tx: f32, ty: f32) -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: tx,
            m32: ty,
        }
    }

    pub const fn scale(sx: f32, sy: f32) -> Self {
        Self {
            m11: sx,
            m12: 0.0,
            m21: 0.0,
            m22: sy,
            m31: 0.0,
            m32: 0.0,
        }
    }

    pub fn rotate(angle_rad: f32) -> Self {
        let (sin, cos) = sin_cos(angle_rad);
        Self {
            m11: cos,
            m12: sin,
            m21: -sin,
            m22: cos,
            m31: 0.0,
            m32: 0.0,
        }
    }

    pub fn then(&self, other: &Transform) -> Self {
        Self {
            m11: self.m11 * other.m11 + self.m12 * other.m21,
            m12: self.m11 * other.m12 + self.m12 * other.m22,
            m21: self.m21 * other.m11 + self.m22 * other.m21,
            m22: self.m21 * other.m12 + self.m22 * other.m22,
            m31: self.m31 * other.m11 + self.m32 * other.m21 + other.m31,
            m32: self.m31 * other.m12 + self.m32 * other.m22 + other.m32,
        }
    }

    pub fn apply(&self, point: Point<f32>) -> Point<f32> {
        Point::new(
            self.m11 * point.x + self.m21 * point.y + self.m31,
            self.m12 * point.x + self.m22 * point.y + self.m32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Pointf;

    #[test]
    fn identity_transform() {
        let t = Transform::identity();
        assert_eq!(t.apply(Pointf::new(3.0, 4.0)), Pointf::new(3.0, 4.0));
    }

    #[test]
    fn translate_transform() {
        let t = Transform::translate(2.0, 3.0);
        assert_eq!(t.apply(Pointf::new(1.0, 1.0)), Pointf::new(3.0, 4.0));
    }

    #[test]
    fn scale_transform() {
        let t = Transform::scale(2.0, -1.0);
        assert_eq!(t.apply(Pointf::new(3.0, 4.0)), Pointf::new(6.0, -4.0));
    }

    #[test]
    fn rotate_transform() {
        let t = Transform::rotate(0.0);
        assert_eq!(t.apply(Pointf::new(1.0, 0.0)).x, 1.0);
    }

    #[test]
    fn compose_transforms() {
        let translate = Transform::translate(1.0, 0.0);
        let scale = Transform::scale(2.0, 2.0);
        let composed = scale.then(&translate);
        assert_eq!(composed.apply(Pointf::new(1.0, 1.0)), Pointf::new(3.0, 2.0));
    }
}
