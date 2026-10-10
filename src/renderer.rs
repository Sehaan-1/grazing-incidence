//! Rendering entry point.

use crate::camera::Camera;
use crate::error::RenderError;
use crate::hittable::HittableList;
use crate::image::Image;
use crate::scene::{self, Scene};

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

/// Cast one ray per pixel through the §7 camera and shade each hit (§6).
///
/// `width` and `height` must both be at least 2: `Camera::render` normalises
/// each axis by `axis - 1`, so a 1-pixel axis divides by zero and every channel
/// becomes NaN. NaN survives `f64::clamp` and truncates to 0 as `u8`, so without
/// this guard a 1xN render silently writes a black image instead of reporting
/// the mistake. The guard lives here, not in `Camera::new`, because that
/// constructor returns `Self` rather than a `Result`.
pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
    if width < 2 || height < 2 {
        return Err(RenderError::InvalidDimension { width, height });
    }

    let objects = scene::build(Scene::Spheres);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    let camera = Camera::new(width, height, 1, 5);
    camera.render(&world)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_one_pixel_wide() {
        let err = render_scene(1, 4).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension {
                width: 1,
                height: 4
            }
        ));
    }

    #[test]
    fn rejects_one_pixel_tall() {
        let err = render_scene(4, 1).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension {
                width: 4,
                height: 1
            }
        ));
    }

    #[test]
    fn accepts_two_by_two_as_the_smallest_size() {
        let image = render_scene(2, 2).unwrap();
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(image.pixels.len(), 4);
        // (width - 1) and (height - 1) are 1 at this size, so every channel is
        // finite rather than NaN.
        assert!(image.pixels.iter().flatten().all(|c| c.is_finite()));
    }
}
