//! 3D vectors and colours.

use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, Neg, Sub, SubAssign};

/// A colour is a Vec3 whose components are linear-light intensity in `[0, 1]`.
pub type Color = Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const UNIT: Vec3 = Vec3 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// Scale to unit length. A zero vector yields NaN, as in the book.
    pub fn unit(self) -> Self {
        self / self.length()
    }

    /// True when every component is within `1e-8` of zero (§10).
    pub fn near_zero(self) -> bool {
        let s = 1e-8;
        self.x.abs() < s && self.y.abs() < s && self.z.abs() < s
    }

    pub fn clamp(self, min: f64, max: f64) -> Self {
        Self::new(
            self.x.clamp(min, max),
            self.y.clamp(min, max),
            self.z.clamp(min, max),
        )
    }

    pub fn to_rgb(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    pub fn from_rgb(rgb: [f64; 3]) -> Self {
        Self::new(rgb[0], rgb[1], rgb[2])
    }
}

/// Colour-only constructors. `Color` is an alias for `Vec3`, so inherent
/// methods have to live in `impl Color` to be reachable as `Color::random`;
/// putting them in the `impl Vec3` block above would make them `Vec3::random`
/// and lose the colour reading at the call site.
impl Color {
    /// A uniformly random colour with each component in `[0, 1)` (§10).
    pub fn random(rng: &mut crate::rng::Rng) -> Self {
        Self::new(rng.f64(), rng.f64(), rng.f64())
    }

    /// A uniformly random colour with each component in `[min, max)` (§10).
    pub fn random_range(rng: &mut crate::rng::Rng, min: f64, max: f64) -> Self {
        Self::new(
            rng.range(min, max),
            rng.range(min, max),
            rng.range(min, max),
        )
    }
}

/// Reflect `v` about a unit normal `n` (§10).
pub fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - 2.0 * v.dot(n) * n
}

/// Refract `uv` (unit length, pointing toward the surface) through normal `n`
/// into a medium whose index ratio is `eta_ratio` (§11).
pub fn refract(uv: Vec3, n: Vec3, eta_ratio: f64) -> Vec3 {
    let cos_theta = (-uv).dot(n).min(1.0);
    let r_out_perp = eta_ratio * (uv + cos_theta * n);
    let r_out_parallel = -((1.0 - r_out_perp.length_squared()).abs()).sqrt() * n;
    r_out_perp + r_out_parallel
}

impl Index<usize> for Vec3 {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("index out of bounds for Vec3: {i}"),
        }
    }
}

impl IndexMut<usize> for Vec3 {
    fn index_mut(&mut self, i: usize) -> &mut f64 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!("index out of bounds for Vec3: {i}"),
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<Vec3> for Vec3 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}

impl Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, s: f64) -> Self {
        Self::new(self.x / s, self.y / s, self.z / s)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    fn assert_vec_close(a: Vec3, b: Vec3) {
        close(a.x, b.x);
        close(a.y, b.y);
        close(a.z, b.z);
    }

    #[test]
    fn new_stores_components() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn index_reads_and_writes_components() {
        let mut v = Vec3::ZERO;
        v[1] = 5.0;
        assert_eq!(v[1], 5.0);
    }

    #[test]
    fn arithmetic_operators() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_vec_close(a + b, Vec3::new(5.0, 7.0, 9.0));
        assert_vec_close(b - a, Vec3::new(3.0, 3.0, 3.0));
        assert_vec_close(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
        assert_vec_close(2.0 * a, Vec3::new(2.0, 4.0, 6.0));
        assert_vec_close(a / 2.0, Vec3::new(0.5, 1.0, 1.5));
        assert_vec_close(-a, Vec3::new(-1.0, -2.0, -3.0));
        assert_vec_close(a * b, Vec3::new(4.0, 10.0, 18.0));
    }

    #[test]
    fn assign_operators() {
        let mut a = Vec3::new(1.0, 2.0, 3.0);
        a += Vec3::new(1.0, 1.0, 1.0);
        assert_vec_close(a, Vec3::new(2.0, 3.0, 4.0));
        a -= Vec3::new(2.0, 3.0, 4.0);
        assert_vec_close(a, Vec3::ZERO);
    }

    #[test]
    fn dot_is_symmetric_and_zero_for_orthogonal() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        close(a.dot(b), 32.0);
        close(a.dot(b), b.dot(a));
        close(Vec3::new(1.0, 0.0, 0.0).dot(Vec3::new(0.0, 1.0, 0.0)), 0.0);
    }

    #[test]
    fn cross_follows_the_right_hand_rule() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        assert_vec_close(x.cross(y), Vec3::new(0.0, 0.0, 1.0));
        assert_vec_close(y.cross(x), Vec3::new(0.0, 0.0, -1.0));
        close(x.cross(y).dot(y.cross(x)), -1.0);

        // Axis vectors only ever populate the z component, so they cannot tell a
        // correct cross from one that returns 0 for x and y. This pair has every
        // component non-zero.
        assert_vec_close(
            Vec3::new(1.0, 2.0, 3.0).cross(Vec3::new(4.0, 5.0, 6.0)),
            Vec3::new(-3.0, 6.0, -3.0),
        );
    }

    #[test]
    fn length_and_length_squared() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        close(v.length_squared(), 25.0);
        close(v.length(), 5.0);
    }

    #[test]
    fn unit_produces_unit_length() {
        close(Vec3::new(3.0, 4.0, 12.0).unit().length(), 1.0);
    }

    #[test]
    fn near_zero_detects_small_vectors() {
        assert!(Vec3::new(0.0, 1e-10, -1e-10).near_zero());
        assert!(!Vec3::new(0.0, 1e-3, 0.0).near_zero());
    }

    #[test]
    fn clamp_bounds_each_component() {
        assert_vec_close(
            Vec3::new(-1.0, 0.5, 2.0).clamp(0.0, 0.999),
            Vec3::new(0.0, 0.5, 0.999),
        );
    }

    #[test]
    fn rgb_round_trips() {
        let v = Vec3::new(0.1, 0.2, 0.3);
        assert_eq!(v.to_rgb(), [0.1, 0.2, 0.3]);
        assert_vec_close(Vec3::from_rgb([0.1, 0.2, 0.3]), v);
    }

    #[test]
    fn color_random_takes_three_draws_in_component_order() {
        // `Color::random` is `Vec3::new(rng.f64(), rng.f64(), rng.f64())`, so it
        // is the first three values a fresh seed yields, in order.
        let mut rng = crate::rng::Rng::seed_from_u64(9);
        let expected = Vec3::new(rng.f64(), rng.f64(), rng.f64());

        let mut rng = crate::rng::Rng::seed_from_u64(9);
        assert_eq!(Color::random(&mut rng), expected);
    }

    #[test]
    fn color_random_stays_in_the_unit_interval() {
        let mut rng = crate::rng::Rng::seed_from_u64(31);
        for _ in 0..10_000 {
            let c = Color::random(&mut rng);
            for component in [c.x, c.y, c.z] {
                assert!(
                    (0.0..1.0).contains(&component),
                    "random colour left [0, 1): {c:?}"
                );
            }
        }
    }

    #[test]
    fn color_random_range_is_an_affine_rescale_of_three_draws() {
        // `random_range(min, max)` is `min + (max - min) * f64()` three times,
        // which is what `Rng::range` does.
        let mut rng = crate::rng::Rng::seed_from_u64(12);
        let expected = Vec3::new(
            rng.range(0.2, 0.4),
            rng.range(0.2, 0.4),
            rng.range(0.2, 0.4),
        );

        let mut rng = crate::rng::Rng::seed_from_u64(12);
        assert_eq!(Color::random_range(&mut rng, 0.2, 0.4), expected);
    }

    #[test]
    fn color_random_range_respects_its_bounds() {
        let mut rng = crate::rng::Rng::seed_from_u64(5);
        for _ in 0..10_000 {
            let c = Color::random_range(&mut rng, -0.5, 2.5);
            for component in [c.x, c.y, c.z] {
                assert!(
                    (-0.5..2.5).contains(&component),
                    "random colour left [-0.5, 2.5): {c:?}"
                );
            }
        }
    }

    #[test]
    fn reflect_preserves_length_and_mirrors_about_the_normal() {
        let v = Vec3::new(0.0, -1.0, 0.0);
        let n = Vec3::new(0.0, 1.0, 0.0);
        let r = reflect(v, n);
        assert_vec_close(r, Vec3::new(0.0, 1.0, 0.0));
        close(r.length(), v.length());
    }

    #[test]
    fn refract_entering_glass_bends_toward_the_normal() {
        // 45-degree incidence, air -> glass, eta = 1/1.5.
        let uv = Vec3::new(1.0, -1.0, 0.0).unit();
        let n = Vec3::new(0.0, 1.0, 0.0);
        let t = refract(uv, n, 1.0 / 1.5);

        // Snell: sin(45)/1.5 = sin(theta_t), so theta_t is predicted by asin of
        // that ratio. theta_t is measured from the normal, and t runs *into* the
        // surface, so it is the angle between t and the inward normal -n: acos.
        let sin_theta_i = 45.0_f64.to_radians().sin();
        let theta_t = (-t.dot(n)).acos().to_degrees();
        close(theta_t, (sin_theta_i / 1.5).asin().to_degrees());
        close(t.length(), 1.0);
    }

    #[test]
    fn refract_with_equal_indices_is_a_no_op() {
        let uv = Vec3::new(1.0, -1.0, 0.0).unit();
        let n = Vec3::new(0.0, 1.0, 0.0);
        assert_vec_close(refract(uv, n, 1.0), uv);
    }

    #[test]
    fn refract_past_the_critical_angle_stays_finite() {
        let uv = Vec3::new(1.0, -0.01, 0.0).unit();
        let n = Vec3::new(0.0, 1.0, 0.0);
        let t = refract(uv, n, 1.5);
        assert!(t.x.is_finite() && t.y.is_finite() && t.z.is_finite());

        // eta * sin(theta_i) = 1.4999 > 1, so this is past the critical angle and
        // there is no refracted ray: the sqrt argument goes negative. The book's
        // formula has no total-internal-reflection case, and `refract` keeps the
        // result finite by taking the absolute value of that argument. The
        // magnitude that falls out is therefore sqrt(2q - 1) for q = |r_out_perp|^2,
        // not 1 — a unit-length refracted ray does not exist here.
        let q = (1.5 * uv.x).powi(2);
        close(t.length(), (2.0 * q - 1.0).sqrt());
    }
}
