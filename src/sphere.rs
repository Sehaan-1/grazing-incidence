//! Spheres.

use crate::hittable::{HitRecord, Hittable};
use crate::material::Material;
use crate::ray::Ray;
use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f64,
    pub material: Material,
}

impl Sphere {
    pub const fn new(center: Vec3, radius: f64, material: Material) -> Self {
        Self {
            center,
            radius,
            material,
        }
    }
}

impl Hittable for Sphere {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        // §6.2: divide the quadratic through by 4 and track half of b, so the
        // discriminant is half_b^2 - a*c instead of b^2 - 4*a*c.
        let oc = ray.origin - self.center;
        let a = ray.direction.dot(ray.direction);
        let half_b = oc.dot(ray.direction);
        let c = oc.dot(oc) - self.radius * self.radius;

        let discriminant = half_b * half_b - a * c;
        if discriminant < 0.0 {
            return None;
        }
        let sqrtd = discriminant.sqrt();

        let mut t = (-half_b - sqrtd) / a;
        if t < t_min || t > t_max {
            t = (-half_b + sqrtd) / a;
            if t < t_min || t > t_max {
                return None;
            }
        }

        let point = ray.at(t);
        let outward = (point - self.center) / self.radius;

        // §6.4: the normal always opposes the incoming ray, whichever face.
        let front_face = ray.direction.dot(outward) < 0.0;
        let normal = if front_face { outward } else { -outward };

        Some(HitRecord {
            t,
            point,
            normal,
            front_face,
            material: self.material,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::Material;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    #[test]
    fn hits_the_book_example_exactly() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(sphere.hit(&ray, 0.001, f64::INFINITY).unwrap().t, 10.5);
    }

    #[test]
    fn misses_when_aimed_away() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0));
        assert!(sphere.hit(&ray, 0.001, f64::INFINITY).is_none());
    }

    #[test]
    fn misses_when_offset_beyond_the_radius() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let ray = Ray::new(Vec3::new(-11.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert!(sphere.hit(&ray, 0.001, f64::INFINITY).is_none());
    }

    #[test]
    fn respects_t_max() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        // Roots are 10.5 and 11.5. A t_max below both rejects the near root and
        // the far root is beyond t_max too, so the whole hit is rejected.
        assert!(sphere.hit(&ray, 0.001, 10.0).is_none());
        let rec = sphere.hit(&ray, 0.001, 12.0).unwrap();
        close(rec.t, 10.5);
    }

    #[test]
    fn ray_starting_inside_hits_the_far_side() {
        let sphere = Sphere::new(Vec3::ZERO, 5.0, Material::lambertian(Vec3::UNIT));
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        close(sphere.hit(&ray, 0.001, f64::INFINITY).unwrap().t, 5.0);
    }

    #[test]
    fn hit_point_lies_on_the_surface() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let rec = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        close(ray.at(rec.t).length(), sphere.radius);
    }

    #[test]
    fn half_b_form_agrees_with_the_books_full_quadratic() {
        // The book's own worked numbers through both formulations.
        let sphere = Sphere::new(
            Vec3::new(0.0, 0.0, -1.0),
            0.5,
            Material::lambertian(Vec3::UNIT),
        );
        let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));

        let oc = ray.origin - sphere.center;
        let a = ray.direction.dot(ray.direction);
        let half_b = oc.dot(ray.direction);
        let c = oc.dot(oc) - sphere.radius * sphere.radius;

        // full form: b = 2*half_b, so b^2 - 4ac == 4*(half_b^2 - a*c)
        let b = 2.0 * half_b;
        close(b * b - 4.0 * a * c, 4.0 * (half_b * half_b - a * c));

        close(sphere.hit(&ray, 0.001, f64::INFINITY).unwrap().t, 0.5);
    }

    /// The record has to report the sphere's own material, not a default one,
    /// or §9 would shade every surface the same.
    #[test]
    fn hit_reports_the_sphere_material() {
        let material = Material::lambertian(Vec3::new(0.7, 0.3, 0.3));
        let sphere = Sphere::new(Vec3::ZERO, 0.5, material);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let rec = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert_eq!(rec.material, material);
        assert_eq!(sphere.material, material);
    }
}
