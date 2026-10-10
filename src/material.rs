//! Surface materials.

use crate::hittable::HitRecord;
use crate::ray::Ray;
use crate::rng::Rng;
use crate::vec3::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    Lambertian { albedo: Color },
}

impl Material {
    pub fn lambertian(albedo: Color) -> Self {
        Self::Lambertian { albedo }
    }

    /// Choose a scattered ray and its attenuation (§9). `None` absorbs the ray.
    ///
    /// The book's `lambertian` hands back only a ray; this returns the
    /// attenuation alongside it so §10's metal can tint its reflection from
    /// the same signature, without a breaking change.
    ///
    /// `_ray` is unused until §10 reflects it. Naming it with an underscore
    /// keeps the one-variant enum out of `unused_variables`; §10 drops the
    /// underscore when the arm that reads it lands.
    pub fn scatter(&self, _ray: &Ray, rec: &HitRecord, rng: &mut Rng) -> Option<(Ray, Color)> {
        match self {
            // §9: normal + a random unit vector approximates the Lambertian
            // distribution. The near_zero guard catches the case where the two
            // cancel exactly, which would otherwise yield a zero direction.
            Material::Lambertian { albedo } => {
                let mut direction = rec.normal + rng.unit_vector();
                if direction.near_zero() {
                    direction = rec.normal;
                }
                Some((Ray::new(rec.point, direction), *albedo))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Vec3;

    fn record(point: Vec3, normal: Vec3, material: Material) -> HitRecord {
        HitRecord {
            t: 1.0,
            point,
            normal,
            front_face: true,
            material,
        }
    }

    fn straight_down(normal: Vec3) -> Ray {
        Ray::new(Vec3::ZERO, -normal)
    }

    #[test]
    fn lambertian_returns_the_albedo_unchanged() {
        let albedo = Vec3::new(0.4, 0.2, 0.1);
        let material = Material::lambertian(albedo);
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let rec = record(Vec3::ZERO, normal, material);
        let (_, attenuation) = material
            .scatter(&straight_down(normal), &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        assert_eq!(attenuation, albedo);
    }

    #[test]
    fn lambertian_scatters_from_the_hit_point() {
        let material = Material::lambertian(Vec3::UNIT);
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let point = Vec3::new(1.0, 2.0, 3.0);
        let rec = record(point, normal, material);
        let (scattered, _) = material
            .scatter(&straight_down(normal), &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        assert_eq!(scattered.origin, point);
    }

    #[test]
    fn lambertian_never_scatters_below_the_surface() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::lambertian(Vec3::UNIT);
        let rec = record(Vec3::ZERO, normal, material);
        let mut rng = Rng::seed_from_u64(7);
        for _ in 0..10_000 {
            let (scattered, _) = material
                .scatter(&straight_down(normal), &rec, &mut rng)
                .unwrap();
            assert!(scattered.direction.dot(normal) > 0.0);
        }
    }

    /// A smoke test only: `normal + unit_vector` cancels to zero just when the
    /// draw lands within `1e-8` of `-normal`, which a random generator will not
    /// do in 10,000 tries. Removing the guard leaves this test green -- see
    /// `lambertian_replaces_a_direction_that_cancels_to_zero` for the one that
    /// does not.
    #[test]
    fn lambertian_scatter_direction_is_never_zero_length() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::lambertian(Vec3::UNIT);
        let rec = record(Vec3::ZERO, normal, material);
        let mut rng = Rng::seed_from_u64(3);
        for _ in 0..10_000 {
            let (scattered, _) = material
                .scatter(&straight_down(normal), &rec, &mut rng)
                .unwrap();
            assert!(!scattered.direction.near_zero());
        }
    }

    /// The `near_zero` guard, driven at the exact case it exists for.
    ///
    /// `Rng` is deterministic, so the first `unit_vector` a seed yields is
    /// known in advance. Handing `scatter` a normal of `-u` and then the same
    /// seed makes the first draw `u`, and `(-u) + u` is exactly `0.0` in every
    /// component under IEEE 754. Without the guard the ray leaves with a zero
    /// direction, which `unit()` turns into NaN; with it, the direction falls
    /// back to `rec.normal`.
    #[test]
    fn lambertian_replaces_a_direction_that_cancels_to_zero() {
        const SEED: u64 = 3;
        let mut probe = Rng::seed_from_u64(SEED);
        let u = probe.unit_vector();
        // Not an arbitrary pick: the two cancel only if `u` really is what
        // `scatter` draws first, so pin that down before relying on it.
        assert!(!u.near_zero());

        let normal = -u;
        let material = Material::lambertian(Vec3::UNIT);
        let point = Vec3::new(1.0, 2.0, 3.0);
        let rec = record(point, normal, material);

        let (scattered, attenuation) = material
            .scatter(
                &Ray::new(Vec3::ZERO, u),
                &rec,
                &mut Rng::seed_from_u64(SEED),
            )
            .unwrap();
        assert_eq!(scattered.origin, point);
        assert_eq!(attenuation, Vec3::UNIT);
        assert_eq!(scattered.direction, normal);
        assert!(!scattered.direction.near_zero());
    }

    #[test]
    fn scattering_is_reproducible_for_a_given_seed() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::lambertian(Vec3::UNIT);
        let rec = record(Vec3::ZERO, normal, material);
        let ray = straight_down(normal);
        let a = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(4))
            .unwrap()
            .0;
        let b = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(4))
            .unwrap()
            .0;
        assert_eq!(a, b);
    }
}
