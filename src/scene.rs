//! Named scenes.

use crate::hittable::Hittable;
use crate::material::Material;
use crate::sphere::Sphere;
use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// The ground sphere plus one ball (§6).
    Spheres,
}

/// Names accepted by `--scene`. The CLI checks a `--scene` argument against
/// this list, so adding a variant means adding a name here with it.
pub const SCENE_NAMES: &[&str] = &["spheres"];

impl Scene {
    /// Look a scene up by its `--scene` name, or `None` if there is no such
    /// scene.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "spheres" => Some(Self::Spheres),
            _ => None,
        }
    }
}

/// Build the objects for a scene. Returns owned boxes so the caller can keep
/// them alive while `HittableList` borrows them.
pub fn build(scene: Scene) -> Vec<Box<dyn Hittable>> {
    match scene {
        // The book's §6 world, with §9's Lambertian ball, §10's metal ball and
        // §11's glass ball sharing it. The ground sphere's top surface sits at
        // -100.5 + 100 = -0.5, below the eye at the origin; an earlier ground at
        // (0, -1000, 0) with r = 1000 put that surface exactly at y = 0, so the
        // camera sat on it and no ray ever reached the sky.
        Scene::Spheres => {
            let ground = Sphere::new(
                Vec3::new(0.0, -100.5, -1.0),
                100.0,
                Material::lambertian(Vec3::new(0.8, 0.8, 0.0)),
            );
            // The book's order: glass on the left, matte in the middle, metal on
            // the right, all radius 0.5 at z = -1. The glass refracts what is
            // behind it, which is what puts the middle ball's image inside the
            // left one.
            let left = Sphere::new(Vec3::new(-1.0, 0.0, -1.0), 0.5, Material::dielectric(1.5));
            let middle = Sphere::new(
                Vec3::new(0.0, 0.0, -1.0),
                0.5,
                Material::lambertian(Vec3::new(0.1, 0.2, 0.5)),
            );
            let right = Sphere::new(
                Vec3::new(1.0, 0.0, -1.0),
                0.5,
                Material::metal(Vec3::new(0.8, 0.6, 0.2), 0.0),
            );
            vec![
                Box::new(ground),
                Box::new(left),
                Box::new(middle),
                Box::new(right),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hittable::HittableList;
    use crate::ray::Ray;

    #[test]
    fn spheres_scene_contains_four_objects() {
        assert_eq!(build(Scene::Spheres).len(), 4);
    }

    #[test]
    fn every_advertised_scene_name_resolves() {
        // `--scene` validates against `SCENE_NAMES` and the renderer resolves
        // through `from_name`. If those two ever drift, a name the CLI accepts
        // would fail at render time, so they are checked against each other
        // rather than written down twice.
        for name in SCENE_NAMES {
            assert!(
                Scene::from_name(name).is_some(),
                "`{name}` is advertised but resolves to nothing"
            );
        }
        assert_eq!(Scene::from_name("nope"), None);
        assert_eq!(SCENE_NAMES.len(), 1);
    }

    #[test]
    fn spheres_scene_hits_the_middle_ball() {
        let objects = build(Scene::Spheres);
        let mut world = HittableList::new();
        for o in &objects {
            world.add(o.as_ref());
        }

        // A ray straight down from above the middle ball must hit that ball, not
        // the ground. The ball at (0,0,-1) r=0.5 puts the surface at t = 2 - 0.5
        // = 1.5 along this ray; the ground at (0,-100.5,-1) r=100 is far enough
        // below that its surface is at t = 102.5 - 100 = 2.5. The two outer
        // balls are 1.0 away from the ray's own line in x, outside their radius
        // of 0.5, so neither can answer. `HittableList` takes the closest of what
        // is left, so this pins which sphere did.
        let ray = Ray::new(Vec3::new(0.0, 2.0, -1.0), Vec3::new(0.0, -1.0, 0.0));
        let rec = world.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(
            (rec.t - 1.5).abs() < 1e-9,
            "expected t = 1.5, got {}",
            rec.t
        );
    }

    #[test]
    fn the_eye_sits_above_the_ground_not_on_it() {
        // The bug this scene guards: a ground at (0,-1000,0) with r = 1000 has
        // its surface at y = 0, exactly where the camera is, so every ray that
        // tilts downward starts already inside the sphere and no ray reaches the
        // sky. The book's ground tops out at y = -0.5, so a ray from the origin
        // aimed straight up misses everything.
        let objects = build(Scene::Spheres);
        let mut world = HittableList::new();
        for o in &objects {
            world.add(o.as_ref());
        }

        let up = Ray::new(Vec3::ZERO, Vec3::new(0.0, 1.0, -0.001).unit());
        assert!(world.hit(&up, 0.001, f64::INFINITY).is_none());
    }
}
