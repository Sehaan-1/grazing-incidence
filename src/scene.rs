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
        Scene::Spheres => {
            let ground = Sphere::new(Vec3::new(0.0, -1000.0, 0.0), 1000.0);
            let ball = Sphere::new(Vec3::new(0.0, 0.5, 0.0), 0.5);
            vec![Box::new(ground), Box::new(ball)]
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

        // A ray straight down from above the ball must hit it.
        let ray = Ray::new(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let rec = world.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(rec.t < 5.0);
    }
}
