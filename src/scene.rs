//! Named scenes.

use crate::hittable::Hittable;
use crate::sphere::Sphere;
use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// The ground sphere plus one ball (§6).
    Spheres,
}

/// Build the objects for a scene. Returns owned boxes so the caller can keep
/// them alive while `HittableList` borrows them.
pub fn build(scene: Scene) -> Vec<Box<dyn Hittable>> {
    match scene {
        // The book's §6 world. The ground sphere's top surface sits at
        // -100.5 + 100 = -0.5, below the eye at the origin; an earlier ground at
        // (0, -1000, 0) with r = 1000 put that surface exactly at y = 0, so the
        // camera sat on it and no ray ever reached the sky.
        Scene::Spheres => {
            let ball = Sphere::new(Vec3::new(0.0, 0.0, -1.0), 0.5);
            let ground = Sphere::new(Vec3::new(0.0, -100.5, -1.0), 100.0);
            vec![Box::new(ball), Box::new(ground)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hittable::HittableList;
    use crate::ray::Ray;

    #[test]
    fn spheres_scene_contains_two_objects() {
        assert_eq!(build(Scene::Spheres).len(), 2);
    }

    #[test]
    fn spheres_scene_hits_the_ball() {
        let objects = build(Scene::Spheres);
        let mut world = HittableList::new();
        for o in &objects {
            world.add(o.as_ref());
        }

        // A ray straight down from above the ball must hit the ball, not the
        // ground. The ball at (0,0,-1) r=0.5 puts the surface at t = 2 - 0.5 =
        // 1.5 along this ray; the ground at (0,-100.5,-1) r=100 is far enough
        // below that its surface is at t = 102.5 - 100 = 2.5. `HittableList`
        // takes the closest, so this pins which sphere answered.
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
