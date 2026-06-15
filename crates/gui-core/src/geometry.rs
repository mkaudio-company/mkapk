use core::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T> {
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

impl<T: Default> Point<T> {
    pub fn zero() -> Self {
        Self::new(T::default(), T::default())
    }
}

impl<T: Add<Output = T>> Point<T> {
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl<T: Sub<Output = T>> Point<T> {
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl<T> Point<T>
where
    T: Mul<Output = T> + Copy,
{
    pub fn scale(self, s: T) -> Self {
        Self::new(self.x * s, self.y * s)
    }
}

impl Point<f32> {
    pub fn distance_squared(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

impl<T: Default> Size<T> {
    pub fn zero() -> Self {
        Self::new(T::default(), T::default())
    }
}

impl<T> Size<T>
where
    T: Mul<Output = T> + PartialOrd + Default + Copy,
{
    pub fn area(self) -> T {
        self.width * self.height
    }

    pub fn is_empty(self) -> bool {
        self.width <= T::default() || self.height <= T::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect<T> {
    pub origin: Point<T>,
    pub size: Size<T>,
}

impl<T> Rect<T> {
    pub const fn new(origin: Point<T>, size: Size<T>) -> Self {
        Self { origin, size }
    }
}

impl<T: Copy + PartialOrd + Default + Sub<Output = T> + Mul<Output = T>> Rect<T> {
    pub fn from_points(min: Point<T>, max: Point<T>) -> Self {
        Self::new(min, Size::new(max.x - min.x, max.y - min.y))
    }

    pub fn width(self) -> T {
        self.size.width
    }

    pub fn height(self) -> T {
        self.size.height
    }

    pub fn is_empty(self) -> bool {
        self.size.is_empty()
    }
}

impl<T: Copy + PartialOrd + Default + Add<Output = T> + Sub<Output = T>> Rect<T> {
    pub fn contains(self, point: Point<T>) -> bool {
        let min = self.origin;
        let max_x = self.origin.x + self.size.width;
        let max_y = self.origin.y + self.size.height;
        point.x >= min.x && point.x < max_x && point.y >= min.y && point.y < max_y
    }
}

impl<T> Rect<T>
where
    T: Copy
        + PartialOrd
        + Default
        + Add<Output = T>
        + Sub<Output = T>
        + Mul<Output = T>
        + core::cmp::Ord,
{
    pub fn intersect(self, other: Self) -> Self {
        let min_x = self.origin.x.max(other.origin.x);
        let min_y = self.origin.y.max(other.origin.y);
        let max_x = (self.origin.x + self.size.width).min(other.origin.x + other.size.width);
        let max_y = (self.origin.y + self.size.height).min(other.origin.y + other.size.height);

        let width = if max_x > min_x {
            max_x - min_x
        } else {
            T::default()
        };
        let height = if max_y > min_y {
            max_y - min_y
        } else {
            T::default()
        };

        Self::new(Point::new(min_x, min_y), Size::new(width, height))
    }

    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }

        let min_x = self.origin.x.min(other.origin.x);
        let min_y = self.origin.y.min(other.origin.y);
        let max_x = (self.origin.x + self.size.width).max(other.origin.x + other.size.width);
        let max_y = (self.origin.y + self.size.height).max(other.origin.y + other.size.height);

        Self::new(
            Point::new(min_x, min_y),
            Size::new(max_x - min_x, max_y - min_y),
        )
    }
}

impl<T: Copy + Add<Output = T> + Sub<Output = T>> Rect<T> {
    pub fn inset(self, insets: Insets<T>) -> Self {
        let origin = Point::new(self.origin.x + insets.left, self.origin.y + insets.top);
        let size = Size::new(
            self.size.width - insets.left - insets.right,
            self.size.height - insets.top - insets.bottom,
        );
        Self::new(origin, size)
    }

    pub fn offset(self, dx: T, dy: T) -> Self {
        Self::new(
            Point::new(self.origin.x + dx, self.origin.y + dy),
            self.size,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Insets<T> {
    pub left: T,
    pub top: T,
    pub right: T,
    pub bottom: T,
}

impl<T> Insets<T> {
    pub const fn new(left: T, top: T, right: T, bottom: T) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

impl<T: Copy> Insets<T> {
    pub fn uniform(value: T) -> Self {
        Self::new(value, value, value, value)
    }
}

impl<T: Copy + Add<Output = T>> Insets<T> {
    pub fn horizontal(self) -> T {
        self.left + self.right
    }

    pub fn vertical(self) -> T {
        self.top + self.bottom
    }
}

pub type Pointf = Point<f32>;
pub type Sizef = Size<f32>;
pub type Rectf = Rect<f32>;
pub type Insetsf = Insets<f32>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_new_zero() {
        let p = Point::new(1, 2);
        assert_eq!(p.x, 1);
        assert_eq!(p.y, 2);
        assert_eq!(Point::<i32>::zero(), Point::new(0, 0));
    }

    #[test]
    fn point_arithmetic() {
        let a = Point::new(1.0, 2.0);
        let b = Point::new(3.0, 4.0);
        assert_eq!(a.add(b), Point::new(4.0, 6.0));
        assert_eq!(b.sub(a), Point::new(2.0, 2.0));
        assert_eq!(a.scale(2.0), Point::new(2.0, 4.0));
    }

    #[test]
    fn point_distance_squared() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(3.0, 4.0);
        assert_eq!(a.distance_squared(b), 25.0);
    }

    #[test]
    fn size_area_and_empty() {
        let s = Size::new(3, 4);
        assert_eq!(s.area(), 12);
        assert!(!s.is_empty());
        assert!(Size::<i32>::zero().is_empty());
        assert!(Size::new(-1, 5).is_empty());
    }

    #[test]
    fn rect_from_points_contains() {
        let r = Rect::from_points(Point::new(0, 0), Point::new(10, 10));
        assert!(r.contains(Point::new(0, 0)));
        assert!(r.contains(Point::new(5, 5)));
        assert!(r.contains(Point::new(9, 9)));
        assert!(!r.contains(Point::new(10, 10)));
        assert!(!r.contains(Point::new(-1, 5)));
    }

    #[test]
    fn rect_intersect() {
        let a = Rect::from_points(Point::new(0, 0), Point::new(10, 10));
        let b = Rect::from_points(Point::new(5, 5), Point::new(15, 15));
        assert_eq!(a.intersect(b), Rect::new(Point::new(5, 5), Size::new(5, 5)));
    }

    #[test]
    fn rect_intersect_non_overlapping() {
        let a = Rect::from_points(Point::new(0, 0), Point::new(2, 2));
        let b = Rect::from_points(Point::new(5, 5), Point::new(7, 7));
        assert!(a.intersect(b).is_empty());
    }

    #[test]
    fn rect_union() {
        let a = Rect::from_points(Point::new(0, 0), Point::new(4, 4));
        let b = Rect::from_points(Point::new(2, 2), Point::new(6, 6));
        assert_eq!(a.union(b), Rect::new(Point::new(0, 0), Size::new(6, 6)));
    }

    #[test]
    fn rect_inset_offset() {
        let r = Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0));
        let inset = r.inset(Insets::uniform(2.0));
        assert_eq!(inset.origin, Point::new(2.0, 2.0));
        assert_eq!(inset.size, Size::new(6.0, 6.0));

        let offset = r.offset(3.0, 4.0);
        assert_eq!(offset.origin, Point::new(3.0, 4.0));
        assert_eq!(offset.size, Size::new(10.0, 10.0));
    }

    #[test]
    fn rect_zero_sized() {
        let r = Rect::<i32>::new(Point::new(0, 0), Size::new(0, 0));
        assert!(r.is_empty());
        assert!(!r.contains(Point::new(0, 0)));
    }

    #[test]
    fn insets() {
        let i = Insets::new(1, 2, 3, 4);
        assert_eq!(i.horizontal(), 4);
        assert_eq!(i.vertical(), 6);
        assert_eq!(Insets::uniform(5), Insets::new(5, 5, 5, 5));
    }
}
