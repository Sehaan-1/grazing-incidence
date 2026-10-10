//! Spheres.

use crate::ray::Ray;
use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f64,
}

impl Sphere {
    pub const fn new(center: Vec3, radius: f64) -> Self {
        Self { center, radius }
    }

    /// Nearest intersection with parameter in `[t_min, t_max]`, if any (§5).
    ///
    /// Uses the book's full quadratic coefficients. §6.2 simplifies these to
    /// the half-b form.
    pub fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<f64> {
        let oc = ray.origin - self.center;

        let a = ray.direction.dot(ray.direction);
        let b = 2.0 * oc.dot(ray.direction);
        let c = oc.dot(oc) - self.radius * self.radius;

        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return None;
        }
        let sqrtd = discriminant.sqrt();

        let mut t = (-b - sqrtd) / (2.0 * a);
        if t < t_min || t > t_max {
            t = (-b + sqrtd) / (2.0 * a);
            if t < t_min || t > t_max {
                return None;
            }
        }
        Some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    #[test]
    fn hits_the_book_example_exactly() {
        // The book's worked example: ray (-11,0,0) + t*(1,0,0) at a unit sphere.
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(sphere.hit(&ray, 0.001, f64::INFINITY), Some(10.5));
    }

    #[test]
    fn misses_when_aimed_away() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(sphere.hit(&ray, 0.001, f64::INFINITY), None);
    }

    #[test]
    fn misses_when_offset_beyond_the_radius() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(sphere.hit(&ray, 0.001, f64::INFINITY), None);
    }

    #[test]
    fn respects_t_max() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        // Roots are 10.5 and 11.5. A t_max below both rejects the near root and
        // the far root is beyond t_max too, so the whole hit is rejected.
        assert_eq!(sphere.hit(&ray, 0.001, 10.0), None);
        let t = sphere.hit(&ray, 0.001, 12.0).unwrap();
        close(t, 10.5);
    }

    #[test]
    fn ray_starting_inside_hits_the_far_side() {
        let sphere = Sphere::new(Vec3::ZERO, 5.0);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        close(sphere.hit(&ray, 0.001, f64::INFINITY).unwrap(), 5.0);
    }

    #[test]
    fn hit_point_lies_on_the_surface() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let t = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        close(ray.at(t).length(), sphere.radius);
    }
}
