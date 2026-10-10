//! The intersection abstraction.

use crate::material::Material;
use crate::ray::Ray;
use crate::vec3::Vec3;

/// Everything known about one ray-surface intersection (§6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRecord {
    pub t: f64,
    pub point: Vec3,
    pub normal: Vec3,
    pub front_face: bool,
    pub material: Material,
}

/// Anything a ray can intersect (§6.3).
pub trait Hittable {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord>;
}

/// An ordered set of hittables; the closest hit wins (§6.5).
#[derive(Default)]
pub struct HittableList<'a> {
    objects: Vec<&'a dyn Hittable>,
}

impl<'a> HittableList<'a> {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    pub fn add(&mut self, object: &'a dyn Hittable) {
        self.objects.push(object);
    }
}

impl Hittable for HittableList<'_> {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        let mut closest = t_max;
        let mut hit = None;

        for object in &self.objects {
            if let Some(record) = object.hit(ray, t_min, closest) {
                closest = record.t;
                hit = Some(record);
            }
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::Material;
    use crate::sphere::Sphere;

    #[test]
    fn empty_list_hits_nothing() {
        let world = HittableList::new();
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert!(world.hit(&ray, 0.001, f64::INFINITY).is_none());
    }

    #[test]
    fn list_returns_the_closest_of_two() {
        let far = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let near = Sphere::new(
            Vec3::new(-5.0, 0.0, 0.0),
            0.5,
            Material::lambertian(Vec3::UNIT),
        );
        let mut world = HittableList::new();
        world.add(&far);
        world.add(&near);

        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let rec = world.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert_eq!(rec.t, 5.5);
    }

    #[test]
    fn the_record_carries_the_material_of_the_sphere_it_hit() {
        // The two spheres differ only in albedo, so `t` alone cannot tell them
        // apart. `list_returns_the_closest_of_two` proves the near sphere wins;
        // this proves the record reports the near sphere's material rather than
        // whichever happened to be added last.
        let far = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let near = Sphere::new(
            Vec3::new(-5.0, 0.0, 0.0),
            0.5,
            Material::lambertian(Vec3::new(0.7, 0.3, 0.3)),
        );
        let mut world = HittableList::new();
        world.add(&far);
        world.add(&near);

        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let rec = world.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert_eq!(rec.t, 5.5);
        assert_eq!(
            rec.material,
            Material::Lambertian {
                albedo: Vec3::new(0.7, 0.3, 0.3)
            }
        );
    }

    #[test]
    fn front_face_is_true_when_the_ray_faces_the_normal() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let rec = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(rec.front_face);
        // The outward normal at the entry point points back along the ray.
        assert!((rec.normal.x + 1.0).abs() < 1e-9);
    }

    #[test]
    fn back_face_normal_flips_and_front_face_is_false() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT));
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        let rec = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(!rec.front_face);
        // The normal always opposes the incoming ray.
        assert!(rec.normal.dot(ray.direction) < 0.0);
    }

    #[test]
    fn hit_point_matches_the_ray_parameter() {
        let sphere = Sphere::new(
            Vec3::new(0.0, 0.0, -1.0),
            0.5,
            Material::lambertian(Vec3::UNIT),
        );
        let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
        let rec = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert_eq!(rec.point, ray.at(rec.t));
    }
}
