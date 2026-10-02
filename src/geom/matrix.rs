use std::fmt;

/// A 2D point with coordinates `x` and `y`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Point({:.3}, {:.3})", self.x, self.y)
    }
}

/// An axis-aligned 2D bounding rectangle defined by min and max coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Rect {
    #[inline]
    pub const fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        let (actual_min_x, actual_max_x) = if min_x <= max_x {
            (min_x, max_x)
        } else {
            (max_x, min_x)
        };
        let (actual_min_y, actual_max_y) = if min_y <= max_y {
            (min_y, max_y)
        } else {
            (max_y, min_y)
        };
        Self {
            min_x: actual_min_x,
            min_y: actual_min_y,
            max_x: actual_max_x,
            max_y: actual_max_y,
        }
    }

    #[inline]
    pub fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rect([{:.3}, {:.3}] -> [{:.3}, {:.3}])",
            self.min_x, self.min_y, self.max_x, self.max_y
        )
    }
}

/// 3x3 affine transformation matrix `[a, b, c, d, e, f]` representing:
/// ```text
/// [a, b, 0]
/// [c, d, 0]
/// [e, f, 1]
/// ```
///
/// Multiplied with row vectors:
/// `[x', y', 1] = [x, y, 1] * Matrix`
/// which yields:
/// `x' = a * x + c * y + e`
/// `y' = b * x + d * y + f`
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Default for Matrix {
    #[inline]
    fn default() -> Self {
        Self::identity()
    }
}

impl Matrix {
    #[inline]
    pub const fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// Constructs the standard identity matrix `[1, 0, 0, 1, 0, 0]`.
    #[inline]
    pub const fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Constructs a translation matrix.
    #[inline]
    pub const fn translation(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    /// Constructs a scaling matrix.
    #[inline]
    pub const fn scaling(sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Constructs a rotation matrix in radians.
    #[inline]
    pub fn rotation(radians: f32) -> Self {
        let cos = radians.cos();
        let sin = radians.sin();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Concatenates this matrix with another matrix: `self * other`.
    ///
    /// Following ISO 32000-1 §8.3.3:
    /// ```text
    /// [a1, b1, 0]   [a2, b2, 0]
    /// [c1, d1, 0] * [c2, d2, 0]
    /// [e1, f1, 1]   [e2, f2, 1]
    /// ```
    #[inline]
    pub fn multiply(&self, other: &Matrix) -> Matrix {
        Matrix {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }

    /// Transforms a 2D point (x, y) by this matrix:
    /// `x' = a * x + c * y + e`
    /// `y' = b * x + d * y + f`
    #[inline]
    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        let new_x = self.a * x + self.c * y + self.e;
        let new_y = self.b * x + self.d * y + self.f;
        (new_x, new_y)
    }

    /// Transforms a `Point`.
    #[inline]
    pub fn transform_p(&self, point: Point) -> Point {
        let (x, y) = self.transform_point(point.x, point.y);
        Point::new(x, y)
    }

    /// Transforms an axis-aligned bounding box and calculates the tight enclosing
    /// bounding box of all four transformed vertices.
    pub fn transform_rect(&self, rect: &Rect) -> Rect {
        let p1 = self.transform_point(rect.min_x, rect.min_y);
        let p2 = self.transform_point(rect.max_x, rect.min_y);
        let p3 = self.transform_point(rect.max_x, rect.max_y);
        let p4 = self.transform_point(rect.min_x, rect.max_y);

        let min_x = p1.0.min(p2.0).min(p3.0).min(p4.0);
        let max_x = p1.0.max(p2.0).max(p3.0).max(p4.0);
        let min_y = p1.1.min(p2.1).min(p3.1).min(p4.1);
        let max_y = p1.1.max(p2.1).max(p3.1).max(p4.1);

        Rect {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    /// Computes the inverse matrix if non-singular.
    pub fn inverse(&self) -> Option<Matrix> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() <= 1e-12 {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Matrix {
            a: self.d * inv_det,
            b: -self.b * inv_det,
            c: -self.c * inv_det,
            d: self.a * inv_det,
            e: (self.c * self.f - self.d * self.e) * inv_det,
            f: (self.b * self.e - self.a * self.f) * inv_det,
        })
    }
}

impl std::ops::Mul for Matrix {
    type Output = Matrix;

    #[inline]
    fn mul(self, rhs: Matrix) -> Matrix {
        self.multiply(&rhs)
    }
}

impl fmt::Display for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Matrix([{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}])",
            self.a, self.b, self.c, self.d, self.e, self.f
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_and_points() {
        let identity = Matrix::identity();
        assert_eq!(identity.transform_point(10.0, 20.0), (10.0, 20.0));
        assert_eq!(identity.multiply(&identity), identity);
    }

    #[test]
    fn test_translation_and_scaling() {
        let t = Matrix::translation(100.0, 50.0);
        assert_eq!(t.transform_point(10.0, 20.0), (110.0, 70.0));

        let s = Matrix::scaling(2.0, 3.0);
        assert_eq!(s.transform_point(10.0, 20.0), (20.0, 60.0));

        // Scale then translate vs translate then scale
        let st = s.multiply(&t);
        // (10, 20) * S * T = (20, 60) * T = (120, 110)
        assert_eq!(st.transform_point(10.0, 20.0), (120.0, 110.0));
    }

    #[test]
    fn test_matrix_inverse() {
        let m = Matrix::new(2.0, 0.5, 0.0, 1.5, 10.0, -20.0);
        let inv = m.inverse().expect("matrix should be invertible");
        let ident = m.multiply(&inv);

        assert!((ident.a - 1.0).abs() < 1e-5);
        assert!(ident.b.abs() < 1e-5);
        assert!(ident.c.abs() < 1e-5);
        assert!((ident.d - 1.0).abs() < 1e-5);
        assert!(ident.e.abs() < 1e-5);
        assert!(ident.f.abs() < 1e-5);

        // Singular matrix
        let singular = Matrix::new(1.0, 2.0, 2.0, 4.0, 0.0, 0.0);
        assert_eq!(singular.inverse(), None);
    }

    #[test]
    fn test_rect_transformation() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(r.width(), 20.0);
        assert_eq!(r.height(), 20.0);

        let t = Matrix::translation(5.0, 10.0);
        let tr = t.transform_rect(&r);
        assert_eq!(tr, Rect::new(15.0, 30.0, 35.0, 50.0));

        // 90 degree rotation
        let rot = Matrix::new(0.0, 1.0, -1.0, 0.0, 0.0, 0.0);
        let rotated_r = rot.transform_rect(&r);
        // (10, 20) -> (-20, 10)
        // (30, 20) -> (-20, 30)
        // (30, 40) -> (-40, 30)
        // (10, 40) -> (-40, 10)
        // Bbox should be [-40, 10, -20, 30]
        assert_eq!(rotated_r.min_x, -40.0);
        assert_eq!(rotated_r.min_y, 10.0);
        assert_eq!(rotated_r.max_x, -20.0);
        assert_eq!(rotated_r.max_y, 30.0);
    }
}
