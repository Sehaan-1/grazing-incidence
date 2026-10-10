//! Rays in linear space.

use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub const fn new(origin: Vec3, direction: Vec3) -> Self {
        Self { origin, direction }
    }

    /// The point at parameter `t` along the ray: `origin + t * direction`.
    pub fn at(&self, t: f64) -> Vec3 {
        self.origin + t * self.direction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_zero_is_the_origin() {
        let r = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(r.at(0.0), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn at_one_is_origin_plus_direction() {
        let r = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(r.at(1.0), Vec3::new(1.0, 3.0, 3.0));
    }

    #[test]
    fn at_two_is_double_the_direction() {
        let r = Ray::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0));
        assert_eq!(r.at(2.0), Vec3::new(2.0, 2.0, 0.0));
    }
}
