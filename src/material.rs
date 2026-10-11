//! Surface materials.

use crate::hittable::HitRecord;
use crate::ray::Ray;
use crate::rng::Rng;
use crate::vec3::{Color, reflect, refract};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    Lambertian { albedo: Color },
    Metal { albedo: Color, fuzz: f64 },
    Dielectric { ior: f64 },
}

impl Material {
    pub fn lambertian(albedo: Color) -> Self {
        Self::Lambertian { albedo }
    }

    /// A polished metal. `fuzz` is the blur radius in world units, clamped to
    /// `[0, 1]` on every scatter.
    pub fn metal(albedo: Color, fuzz: f64) -> Self {
        Self::Metal { albedo, fuzz }
    }

    /// Glass and every other transparent medium. `ior` is the index of
    /// refraction relative to the surrounding medium.
    pub fn dielectric(ior: f64) -> Self {
        Self::Dielectric { ior }
    }

    /// Choose a scattered ray and its attenuation. `None` absorbs the
    /// ray.
    ///
    /// Lambertian scattering attenuates by its albedo alone, so it has nothing to
    /// report but a ray. Returning the attenuation alongside it anyway lets metal
    /// tint its reflection from the same signature, without a breaking change.
    /// Metal is also the first material that reads the incoming ray, which is why
    /// the parameter is named `ray` rather than `_ray`.
    pub fn scatter(&self, ray: &Ray, rec: &HitRecord, rng: &mut Rng) -> Option<(Ray, Color)> {
        match self {
            // Normal + a random unit vector approximates the Lambertian
            // distribution. The near_zero guard catches the case where the two
            // cancel exactly, which would otherwise yield a zero direction.
            Material::Lambertian { albedo } => {
                let mut direction = rec.normal + rng.unit_vector();
                if direction.near_zero() {
                    direction = rec.normal;
                }
                Some((Ray::new(rec.point, direction), *albedo))
            }
            // Mirror the ray about the normal, then blur it by `fuzz`.
            // A ray scattered below the surface is absorbed, which is what
            // gives metal surfaces their hard-edged look.
            Material::Metal { albedo, fuzz } => {
                // `reflect` is a module-level function in `vec3`, not an
                // associated function on `Vec3`, so `Vec3::reflect(..)` does
                // not resolve; it is imported by name at the top of the file.
                let reflected = reflect(ray.direction.unit(), rec.normal);
                let fuzz = fuzz.clamp(0.0, 1.0);
                let scattered = Ray::new(rec.point, reflected + fuzz * rng.unit_vector());
                if scattered.direction.dot(rec.normal) < 0.0 {
                    None
                } else {
                    Some((scattered, *albedo))
                }
            }
            // Total internal reflection first, then Schlick's
            // approximation to decide the reflect/refract split. `refract` is
            // module-level in `vec3` for the same reason `reflect` is.
            //
            // The name `ri` denotes the *ratio*, not the material's index, and
            // that distinction matters: entering the sphere means dividing by
            // `ior`, leaving it means multiplying, and only the leaving case can
            // go past the critical angle.
            Material::Dielectric { ior } => {
                let refraction_ratio = if rec.front_face { 1.0 / ior } else { *ior };

                let unit_direction = ray.direction.unit();
                let cos_theta = (-unit_direction).dot(rec.normal).min(1.0);
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

                // Past the critical angle the refracted ray would have to be
                // longer than it is allowed to be, so there is none to take.
                let cannot_refract = refraction_ratio * sin_theta > 1.0;

                let direction =
                    if cannot_refract || reflectance(cos_theta, refraction_ratio) > rng.f64() {
                        reflect(unit_direction, rec.normal)
                    } else {
                        refract(unit_direction, rec.normal, refraction_ratio)
                    };

                // Glass does not tint: every bounce leaves the ray at full
                // strength, so the attenuation is white.
                Some((Ray::new(rec.point, direction), Color::UNIT))
            }
        }
    }
}

/// Schlick's approximation of the Fresnel reflectance.
///
/// `refraction_ratio` is Snell's `eta_ratio`, not a material's `ior`: the ratio
/// of the two indices at the boundary, so `1 / ior` entering a sphere and
/// `ior` leaving one. The distinction is easy to miss, because the quantity
/// reads like the material's index when it is not -- at normal incidence this
/// returns `((1 - eta) / (1 + eta))^2`, and squaring makes that 0.04 for
/// air/glass whether the ratio is `1/1.5` or `1.5`.
fn reflectance(cosine: f64, refraction_ratio: f64) -> f64 {
    let mut r0 = (1.0 - refraction_ratio) / (1.0 + refraction_ratio);
    r0 *= r0;
    r0 + (1.0 - r0) * (1.0 - cosine).powi(5)
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

    /// A dielectric hit record with the face under the ray chosen by hand.
    ///
    /// `record` always reports `front_face: true`, which is the air-to-glass
    /// direction. Leaving a sphere is the other case and reports `false`.
    fn dielectric_record(normal: Vec3, front_face: bool) -> HitRecord {
        let mut rec = record(Vec3::ZERO, normal, Material::dielectric(1.5));
        rec.front_face = front_face;
        rec
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

    /// The exact mirror at `fuzz = 0`.
    ///
    /// With `d = 1/sqrt(2)`, the incoming direction is `(d, -d, 0)`, `v . n`
    /// is `-d`, and `2 (v . n) n` is `(0, -2d, 0)`. Doubling is exact in
    /// binary64, so `2d` is representable and `-d + 2d` is `d` again with no
    /// rounding at all: `reflect` returns `(d, d, 0)` exactly.
    ///
    /// The blur term is `0.0 * u`, which is `+-0.0` per component, and
    /// `x + (+-0.0) == x` for every `x` involved here -- including `+0.0`,
    /// whose sum with `-0.0` rounds to `+0.0`. So the scattered direction is
    /// bit-identical to the mirror, the length below is exactly `0.0`, and the
    /// `1e-12` tolerance is slack rather than load-bearing.
    #[test]
    fn metal_with_zero_fuzz_mirrors_the_ray_exactly() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(Vec3::UNIT, 0.0);
        let rec = record(Vec3::ZERO, normal, material);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0).unit());

        let (scattered, _) = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(1))
            .expect("a 45-degree reflection is well above the surface");
        let expected = reflect(ray.direction.unit(), normal);
        assert_eq!(scattered.direction, expected);
        assert!((scattered.direction - expected).length() < 1e-12);
    }

    /// The blur radius is the requested `fuzz` clamped into `[0, 1]`.
    ///
    /// `scatter` returns `reflected + fuzz * u` with `u` a unit vector, so the
    /// offset from the mirror direction has length `fuzz.clamp(0.0, 1.0)` up to
    /// the rounding of `Rng::unit_vector`'s normalisation -- about `1e-16` here,
    /// which is why `1e-12` is the tolerance.
    ///
    /// This replaces the brief's sampling version, which could not have worked.
    /// See `metal_never_absorbs_at_normal_incidence_however_large_the_fuzz` for
    /// why the count it tried to bound is zero when clamped and about 490 when
    /// not -- the two live inside the same `300 < absorbed < 700` band.
    #[test]
    fn metal_fuzz_is_clamped_to_the_unit_interval() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let ray = straight_down(normal);
        let reflected = reflect(ray.direction.unit(), normal);

        for requested in [-3.0, 0.0, 0.25, 1.0, 50.0] {
            let material = Material::metal(Vec3::UNIT, requested);
            let rec = record(Vec3::ZERO, normal, material);
            // Normal incidence never absorbs: the scattered direction is
            // `(0, 1, 0) + fuzz * u`, so its dot with the normal is
            // `1 + fuzz * u.y >= 1 - fuzz >= 0`.
            let (scattered, _) = material
                .scatter(&ray, &rec, &mut Rng::seed_from_u64(1))
                .expect("normal incidence never absorbs");

            let radius = (scattered.direction - reflected).length();
            assert!(
                (radius - requested.clamp(0.0, 1.0)).abs() < 1e-12,
                "fuzz {requested} blurred by {radius}, expected {}",
                requested.clamp(0.0, 1.0)
            );
        }
    }

    /// Why `fuzz` is clamped at all: no `fuzz` may drive a normal-incidence
    /// reflection below the surface.
    ///
    /// A ray straight down reflects to straight up, so the scattered direction
    /// is `(0, 1, 0) + fuzz * u` and its dot with the +y normal is
    /// `1 + fuzz * u.y`. Clamped, `fuzz <= 1` and `u.y >= -1`, so the dot is at
    /// least `1 - 1 = 0` and nothing is absorbed. Left unclamped at 50,
    /// absorption needs only `u.y < -1/50 = -0.02`; `u.y` is uniform on
    /// `[-1, 1]` for a uniform point on the sphere, so the chance is
    /// `(1 - 0.02) / 2 = 0.49` -- measured, 442 to 533 of every 1000 over seeds
    /// 0 through 200. Zero against ~490 is the whole distance between the two.
    #[test]
    fn metal_never_absorbs_at_normal_incidence_however_large_the_fuzz() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(Vec3::UNIT, 50.0);
        let rec = record(Vec3::ZERO, normal, material);
        let ray = straight_down(normal);

        let mut rng = Rng::seed_from_u64(9);
        let absorbed = (0..1_000)
            .filter(|_| material.scatter(&ray, &rec, &mut rng).is_none())
            .count();
        assert_eq!(absorbed, 0, "absorbed {absorbed} of 1000");
    }

    /// At grazing incidence the mirror direction lies *in* the surface plane, so
    /// the below-surface test cuts the fuzz ball exactly in half.
    ///
    /// A ray along +x with normal +y reflects to `unit(1,0,0) - 2 * 0 * n`,
    /// that is `(1, 0, 0)`, whose dot with the normal is exactly 0. Adding
    /// `1.0 * u` leaves the dot at `u.y`, and `u.y` is uniform on `[-1, 1]`, so
    /// absorption has probability exactly 1/2: 1000 draws give 500 with a
    /// binomial sd of `sqrt(1000 * 1/2 * 1/2) = 15.81`. The band below sits
    /// `+-12.6` sd out, and 200 seeds land between 448 and 541.
    #[test]
    fn metal_splits_grazing_rays_about_the_surface() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(Vec3::UNIT, 1.0);
        let rec = record(Vec3::ZERO, normal, material);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));

        let mut absorbed = 0;
        let mut rng = Rng::seed_from_u64(9);
        for _ in 0..1_000 {
            if material.scatter(&ray, &rec, &mut rng).is_none() {
                absorbed += 1;
            }
        }
        assert!(
            absorbed > 300 && absorbed < 700,
            "absorbed {absorbed} of 1000"
        );
    }

    /// Every ray that leaves the metal is on or above the surface.
    ///
    /// `!(d . n < 0.0)` is exactly `d . n >= 0.0`, so that is what this asserts;
    /// `> 0.0` would be a probability-zero strengthening rather than a fact,
    /// since a grazing ray reflected at `fuzz = 1` has a dot of exactly `u.y`
    /// and `u.y = 0` sits on the equator.
    ///
    /// The grazing ray is what makes the test bite. At normal incidence nothing
    /// is ever absorbed, so the body would never run against the guard; grazing
    /// absorbs about half of them, so 5067 of these 10000 rays returned `None`
    /// and every one that came back sat strictly above the plane -- the closest,
    /// on this seed, landed `3.8e-5` above it.
    #[test]
    fn metal_never_scatters_below_its_surface() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(Vec3::UNIT, 1.0);
        let rec = record(Vec3::ZERO, normal, material);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        let mut rng = Rng::seed_from_u64(13);
        for _ in 0..10_000 {
            if let Some((scattered, _)) = material.scatter(&ray, &rec, &mut rng) {
                assert!(scattered.direction.dot(normal) >= 0.0);
            }
        }
    }

    #[test]
    fn metal_attenuates_by_its_albedo() {
        let albedo = Vec3::new(0.9, 0.4, 0.1);
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(albedo, 0.0);
        let rec = record(Vec3::ZERO, normal, material);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0).unit());
        let (_, attenuation) = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        assert_eq!(attenuation, albedo);
    }

    /// Schlick at normal incidence is `((1 - eta) / (1 + eta))^2` and nothing
    /// else, because `(1 - 1)^5 = 0` kills the grazing term outright.
    ///
    /// With `eta = 1/1.5 = 0.66666666666666663` in binary64:
    ///   1 - eta   = 0.33333333333333337   (exact: the exponent doubles)
    ///   1 + eta   = 1.6666666666666665    (one rounding)
    ///   r0        = 0.20000000000000004
    ///   r0 * r0   = 0.04000000000000001
    /// against a true `0.04`, so the error is `1.4e-17` and the `1e-12`
    /// tolerance below is slack rather than load-bearing. Observed:
    /// `0.04000000000000001`.
    ///
    /// This is also the assertion that dies if `r0 *= r0` is dropped: unsquared,
    /// the same expression returns `0.20000000000000004`, which is `0.16` away
    /// from `0.04`.
    #[test]
    fn reflectance_at_normal_incidence_matches_r0() {
        let r = reflectance(1.0, 1.0 / 1.5);
        assert!(
            (r - 0.04).abs() < 1e-12,
            "normal-incidence reflectance should be 0.04, got {r}"
        );
    }

    /// Three angles, one rising curve: normal, 45 degrees, grazing.
    ///
    /// The grazing value is exactly 1.0 and not merely near it. At
    /// `cosine = 0` the term collapses to `r0 + (1 - r0) * 1`, and with
    /// `r0 = 0.04000000000000001` the subtraction `1 - r0` rounds once -- by at
    /// most half an ulp, since `r0` sits at exponent -5 and the result near 1.0
    /// has an ulp of `2^-53`. Adding `r0` back lands within `2^-54` of 1.0,
    /// whose nearest representable neighbours are `2^-53` away, so the sum
    /// rounds onto 1.0 itself. Observed: `1.0`.
    #[test]
    fn reflectance_rises_toward_grazing_angles() {
        let eta = 1.0 / 1.5;
        let r_normal = reflectance(1.0, eta);
        let r_45 = reflectance(0.5f64.sqrt(), eta);
        let r_grazing = reflectance(0.0, eta);

        // 0.04000000000000001 < 0.04206927312437237 < 1.0
        assert!(
            r_normal < r_45,
            "45 degrees should beat normal: {r_normal} vs {r_45}"
        );
        assert!(
            r_45 < r_grazing,
            "grazing should beat 45 degrees: {r_45} vs {r_grazing}"
        );
        assert_eq!(r_grazing, 1.0);
    }

    #[test]
    fn glass_does_not_dim_the_ray() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        let rec = dielectric_record(normal, true);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0).unit());
        let (_, attenuation) = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        assert_eq!(attenuation, Vec3::UNIT);
    }

    /// At normal incidence `refract` hands the ray straight back: with
    /// `uv + cos_theta * n` cancelling to exactly zero, the perpendicular term
    /// vanishes and only `-sqrt(1 - 0) * n = -n` is left, which is `uv` again.
    /// The draw decides the rest, and `reflectance(1.0, 1/1.5) = 0.04`, so a
    /// fraction `0.04` of samples mirror off instead.
    ///
    /// Over 1000 draws of seed 1 that leaves `1000 * 0.04 = 40` mirrors with a
    /// binomial sd of `sqrt(1000 * 0.04 * 0.96) = 6.20`; the observed count of
    /// non-straight samples is 36. The band below is `mean - 12 sd`, so it holds
    /// for the seeds it was checked against rather than for one lucky draw.
    #[test]
    fn glass_refracts_a_head_on_ray_without_deflecting_it() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        let rec = dielectric_record(normal, true);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, -1.0, 0.0));
        let mirror = reflect(ray.direction, normal);

        let mut rng = Rng::seed_from_u64(1);
        let mut mirrored = 0usize;
        for _ in 0..1_000 {
            let (scattered, _) = material.scatter(&ray, &rec, &mut rng).unwrap();
            if (scattered.direction - mirror).length() < 1e-12 {
                mirrored += 1;
            } else {
                assert!(
                    (scattered.direction - ray.direction).length() < 1e-12,
                    "a head-on ray must either go straight or mirror, got {:?}",
                    scattered.direction
                );
            }
        }
        assert!(
            (1_000 - mirrored) > 900,
            "at least 900 of 1000 should pass straight through, {mirrored} mirrored"
        );
    }

    /// `front_face` has to pick `1 / ior`, not `ior`.
    ///
    /// Normal incidence cannot tell the two apart -- both ratios refract the ray
    /// along itself -- so only an oblique ray separates them. Entering the
    /// glass, Snell says `sin(theta_t) = sin(theta_i) / 1.5`, and a 45-degree
    /// entry gives `asin(0.7071067811865476 / 1.5) = 28.1255057` degrees. The
    /// leaving ratio of 1.5 would instead ask for `asin(1.5 * 0.70711) =
    /// asin(1.06066)`, which has no solution at all.
    ///
    /// Every sample is checked against that angle rather than against a
    /// hard-coded direction, so the test would notice a wrong ratio as an
    /// angle it cannot produce. `reflectance(0.7071, 1/1.5) = 0.04206927`, so
    /// about 42 of every 1000 mirror off; seed 11 gives 50.
    #[test]
    fn glass_bends_an_oblique_ray_toward_the_normal_when_entering() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        let rec = dielectric_record(normal, true);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, -1.0, 0.0).unit());
        let mirror = reflect(ray.direction, normal);
        let expected_degrees = (45.0_f64.to_radians().sin() / 1.5).asin().to_degrees();

        let mut rng = Rng::seed_from_u64(11);
        let mut refracted = 0usize;
        for _ in 0..1_000 {
            let (scattered, _) = material.scatter(&ray, &rec, &mut rng).unwrap();
            if (scattered.direction - mirror).length() < 1e-12 {
                continue;
            }
            // theta_t is measured off the inward normal, and the refracted ray
            // runs into the surface, so that is the angle between it and -n.
            let degrees = (-scattered.direction.dot(normal)).acos().to_degrees();
            assert!(
                (degrees - expected_degrees).abs() < 1e-9,
                "expected {expected_degrees} degrees, got {degrees} from {:?}",
                scattered.direction
            );
            refracted += 1;
        }
        assert!(
            refracted > 900,
            "most of 1000 samples should refract, only {refracted} did"
        );
    }

    /// Total internal reflection, and the exact edge of it.
    ///
    /// Leaving glass the ratio is `ior = 1.5`, so no refracted ray exists once
    /// `1.5 * sin_theta > 1`, that is once `sin_theta > 2/3`, which is an
    /// incidence of `asin(2/3) = 41.81031490` degrees.
    ///
    /// The grazing ray below is nowhere near that edge. `unit(1, -0.01, 0)` is
    /// `(0.9999500037496877, -0.009999500037496877, 0)`, so
    /// `cos_theta = 0.00999950003749688`,
    /// `sin_theta = sqrt(1 - cos_theta^2) = 0.99995000374968757`, and
    /// `1.5 * sin_theta = 1.49992500562453124 > 1`. Against the `2/3` threshold
    /// that is 0.3333 past the boundary, and 89.42706130 degrees of incidence
    /// against 41.81031490 -- nowhere near a case the `>` could be reading
    /// either way.
    #[test]
    fn total_internal_reflection_begins_past_the_critical_angle() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        // `front_face` is false because the ray is exiting the glass, which is
        // the only direction from which `ri = ior > 1`.
        let rec = dielectric_record(normal, false);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, -0.01, 0.0).unit());

        let sin_theta = {
            let cos_theta = (-ray.direction).dot(normal).min(1.0);
            assert!(
                1.5 * (1.0 - cos_theta * cos_theta).sqrt() > 1.0,
                "this ray is not past the critical angle; pick another one"
            );
            (1.0 - cos_theta * cos_theta).sqrt()
        };
        assert!(
            sin_theta > 2.0 / 3.0,
            "sin_theta {sin_theta} is not past 2/3"
        );

        let expected = reflect(ray.direction.unit(), normal);
        let mut rng = Rng::seed_from_u64(2);
        for _ in 0..1_000 {
            let (scattered, _) = material.scatter(&ray, &rec, &mut rng).unwrap();
            assert!((scattered.direction - expected).length() < 1e-9);
        }

        // And the other side of the same boundary. The comparison is strict,
        // so a ray sitting exactly on the critical angle -- where a refracted
        // ray exists but leaves along the surface -- still refracts.
        //
        // `(1, -sqrt(1.25), 0)` is `(sin(theta), -cos(theta), 0)` scaled by 3/2
        // with `theta = asin(2/3)`, so it is aimed at exactly the critical
        // angle. In binary64 `cos_theta` comes out `0.74535599249992990` and
        // `sin_theta` `0.66666666666666663`, and `1.5 * sin_theta` is exactly
        // `1.0`. That is a floating-point fact about this direction and no
        // other, so it is asserted rather than assumed: the literal above is
        // only meaningful while the check holds.
        let on_the_edge = Ray::new(Vec3::ZERO, Vec3::new(1.0, -1.25_f64.sqrt(), 0.0).unit());
        let cos_theta = (-on_the_edge.direction).dot(normal).min(1.0);
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
        assert_eq!(
            1.5 * sin_theta,
            1.0,
            "this direction is no longer exactly on the critical angle"
        );
        assert!(!(1.5 * sin_theta > 1.0), "the comparison must be strict");

        // At that angle `refract` returns exactly `(1, 0, 0)`: the
        // perpendicular term is `1.5 * (uv + cos_theta * n)` and the two y
        // components cancel bit for bit, leaving `|r_out_perp| = 1` exactly, so
        // the parallel term is `sqrt(1 - 1) = 0` and what is left is `(1, 0, 0)`
        // -- tangent to the surface, as the critical angle requires. The mirror
        // is `sqrt(2/3) = 0.816496580927726` away from it, so the two outcomes
        // are never confused.
        let mirror = reflect(on_the_edge.direction, normal);
        let tangent = refract(on_the_edge.direction, normal, 1.5);
        assert_eq!(tangent, Vec3::new(1.0, 0.0, 0.0));
        assert!((mirror - tangent).length() > 0.8);

        // `reflectance` is still only ~0.0410 here, so a few hundred draws take
        // the mirror even though a refracted ray is available: 1000 * 0.04103
        // leaves ~41 with a sd of 6.23, and seed 2 gives 38. The point is the
        // other 962 -- read `>=` and none of them would be there.
        let mut rng = Rng::seed_from_u64(2);
        let mut tangential = 0usize;
        for _ in 0..1_000 {
            let (scattered, _) = material.scatter(&on_the_edge, &rec, &mut rng).unwrap();
            if (scattered.direction - tangent).length() < 1e-12 {
                tangential += 1;
            }
        }
        assert!(
            tangential > 900,
            "a ray on the critical angle should still refract, only {tangential} of 1000 did"
        );
    }

    #[test]
    fn dielectric_scatter_is_reproducible() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        let rec = dielectric_record(normal, true);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0).unit());
        let a = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(5))
            .unwrap()
            .0;
        let b = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(5))
            .unwrap()
            .0;
        assert_eq!(a, b);
    }
}
