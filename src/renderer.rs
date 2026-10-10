//! Rendering entry point.

use crate::camera::Camera;
use crate::error::RenderError;
use crate::hittable::HittableList;
use crate::image::Image;
use crate::rng::Rng;
use crate::scene::{self, Scene};

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

/// The seed every committed image is rendered with, so reruns are identical.
pub const DEFAULT_SEED: u64 = 0;

/// Samples per pixel for a committed render (§8).
pub const SAMPLES_PER_PIXEL: usize = 100;

/// Cast `SAMPLES_PER_PIXEL` jittered rays per pixel through the §7 camera and
/// shade each hit (§6, §8).
///
/// `width` and `height` must both be at least 2. §8 no longer forces that: a
/// sample sits at `(i + du) / width`, so `width = 1` divides by 1 rather than
/// 0 and every channel stays finite. The bound is kept because a single-pixel
/// axis describes no field of view at all -- `viewport_width` collapses to
/// `2 / height` and the render is a 1-pixel smear. The guard lives here, not in
/// `Camera::new`, because that constructor returns `Self` rather than a
/// `Result`.
pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
    if width < 2 || height < 2 {
        return Err(RenderError::InvalidDimension { width, height });
    }

    let objects = scene::build(Scene::Spheres);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    let mut rng = Rng::seed_from_u64(DEFAULT_SEED);
    let camera = Camera::new(width, height, SAMPLES_PER_PIXEL, 5);
    camera.render(&world, &mut rng)
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
        // With `width = 2` the jitter divisor is 2, not `width - 1 = 1`, and
        // `du` is below 1, so every sample stays inside [0, 1) and no channel
        // can go NaN or infinite.
        assert!(image.pixels.iter().flatten().all(|c| c.is_finite()));
    }

    #[test]
    fn same_seed_produces_an_identical_image() {
        let a = render_scene(32, 20).unwrap();
        let b = render_scene(32, 20).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }

    #[test]
    fn a_different_seed_moves_the_pixels() {
        // Same geometry, same 100 samples, one seed apart: if the seed were
        // dropped on the floor both renders would be byte-identical, and the
        // determinism test above would be passing for the wrong reason.
        let a = render_with_seed(4, 4, DEFAULT_SEED);
        let b = render_with_seed(4, 4, DEFAULT_SEED + 1);
        assert_ne!(a.pixels, b.pixels);
    }

    /// `render_scene` with the seed left open, for the tests that need to vary
    /// it. A 4x4 render is 16 pixels, so four of these cost nothing next to the
    /// 32x20 determinism render.
    fn render_with_seed(width: usize, height: usize, seed: u64) -> Image {
        let objects = scene::build(Scene::Spheres);
        let mut world = HittableList::new();
        for object in &objects {
            world.add(object.as_ref());
        }
        let mut rng = Rng::seed_from_u64(seed);
        Camera::new(width, height, SAMPLES_PER_PIXEL, 5)
            .render(&world, &mut rng)
            .unwrap()
    }
}
