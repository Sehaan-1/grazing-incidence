//! Rendering entry point.

use crate::camera::{
    APERTURED_DEFOCUS_ANGLE, APERTURED_FOCUS_DIST, APERTURED_VFOV, COVER_ASPECT,
    COVER_DEFOCUS_ANGLE, COVER_DEPTH, COVER_FOCUS_DIST, COVER_LOOKAT, COVER_LOOKFROM, COVER_VFOV,
    COVER_VUP, Camera, PINHOLE_ASPECT, PINHOLE_DEPTH, PINHOLE_LOOKAT, PINHOLE_LOOKFROM,
    PINHOLE_VUP,
};
use crate::cli::Options;
use crate::error::RenderError;
use crate::hittable::HittableList;
use crate::image::Image;
use crate::rng::Rng;
use crate::scene::{self, Scene};

/// Whether to apply the sqrt gamma encode when writing pixels.
pub const GAMMA: bool = true;

/// The camera for a scene, at the size the command line asked for.
///
/// The named constructors pin fixed sizes -- `Camera::pinhole` and
/// `Camera::apertured` at 400 wide and 100 samples, `Camera::cover` at
/// 1200 and 500 -- but the command line owns both numbers, and the width is the
/// only dimension a caller has to give. So the scene picks the *lens* and the
/// caller picks the size.
///
/// `Camera::cover` is deliberately *not* called here. It returns a camera
/// hardwired to 1200x675 at 500 samples, which would ignore `--width` and
/// `--samples` entirely and turn `--scene cover --width 80 --samples 4` into
/// the full 405-million-ray render.
fn camera_for(scene: Scene, width: usize, samples_per_pixel: usize) -> Camera {
    match scene {
        Scene::Spheres => Camera::new(
            width,
            samples_per_pixel,
            PINHOLE_DEPTH,
            PINHOLE_ASPECT,
            APERTURED_VFOV,
            PINHOLE_LOOKFROM,
            PINHOLE_LOOKAT,
            PINHOLE_VUP,
            APERTURED_DEFOCUS_ANGLE,
            APERTURED_FOCUS_DIST,
        ),
        Scene::Cover => Camera::new(
            width,
            samples_per_pixel,
            COVER_DEPTH,
            COVER_ASPECT,
            COVER_VFOV,
            COVER_LOOKFROM,
            COVER_LOOKAT,
            COVER_VUP,
            COVER_DEFOCUS_ANGLE,
            COVER_FOCUS_DIST,
        ),
    }
}

/// Cast `opts.samples_per_pixel` jittered rays per pixel through the scene's
/// camera and shade each hit.
///
/// The scene chooses the lens -- the spheres scene's 20-degree lens from
/// (-2, 2, 1) focused at 3.4, or the cover's from (13, 2, 3) focused at 10 --
/// while the width, the sample count, the seed and the scene itself come from
/// the command line. The width is the only dimension a caller has to give: the
/// height follows from the 16:9 aspect ratio.
///
/// **Scene construction and the render share one generator.** The cover scene
/// draws from `rng` to place its 484 spheres, and the render then draws from
/// where it left off. That is deliberate: it means one seed fixes both the
/// layout and the image, so the cover is reproducible end to end. A separate
/// generator would be reproducible too, but it would mean the sphere layout and
/// the samples were each replayed independently, and the image you get back
/// would no longer be the one a given `--seed` names on its own.
pub fn render_scene(opts: &Options) -> Result<Image, RenderError> {
    let mut rng = Rng::seed_from_u64(opts.seed);

    let camera = camera_for(opts.scene, opts.width, opts.samples_per_pixel);

    // Both axes are still held to 2, even though only the width is a parameter
    // now: the height follows from the aspect ratio, and 2/(16/9) = 1.125
    // truncates to 1. A one-pixel axis describes no field of view at all --
    // the viewport collapses to a line -- so the render would be a 1-pixel
    // smear rather than a picture. The guard lives here rather than in
    // `Camera::new`, because that constructor returns `Self` and not a
    // `Result`; building the camera first means the height in the error is the
    // height that was actually going to be used.
    if opts.width < 2 || camera.image_height < 2 {
        return Err(RenderError::InvalidDimension {
            width: opts.width,
            height: camera.image_height,
        });
    }

    let objects = scene::build(opts.scene, &mut rng);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    camera.render(&world, &mut rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::DEFAULT_SEED;

    /// A small render. The defaults are 400x225 at 100 samples per pixel and
    /// `cargo test` builds unoptimized, so two default renders inside a test
    /// would take many minutes. 40 wide is 40x22 -- `40 / (16/9) = 22.5`
    /// truncates to 22 -- at 2 samples, which exercises the same determinism
    /// guarantee for 1760 samples rather than 9 million.
    fn small(width: usize, samples: usize, seed: u64) -> Options {
        Options {
            width,
            samples_per_pixel: samples,
            seed,
            ..Options::default()
        }
    }

    /// A small render of one named scene.
    fn small_scene(scene: Scene, width: usize, samples: usize, seed: u64) -> Options {
        Options {
            scene,
            ..small(width, samples, seed)
        }
    }

    /// Small enough that a cover render at this size is milliseconds even
    /// unoptimized: 8x4 is 32 pixels and 1 sample each, against a world of
    /// roughly 488 spheres rather than 4.
    const COVER_TEST_WIDTH: usize = 8;
    const COVER_TEST_SAMPLES: usize = 1;

    #[test]
    fn rejects_one_pixel_wide() {
        // 1 / (16/9) = 0.5625 truncates to 0 and is floored at 1.
        let err = render_scene(&small(1, 2, DEFAULT_SEED)).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension {
                width: 1,
                height: 1
            }
        ));
    }

    #[test]
    fn rejects_a_width_that_leaves_a_one_pixel_height() {
        // 2 / (16/9) = 1.125 truncates to 1, so the width is legal on its own
        // and the image would still be a single row. The height has to be
        // checked even though nothing asked for it.
        let err = render_scene(&small(2, 2, DEFAULT_SEED)).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension {
                width: 2,
                height: 1
            }
        ));
    }

    /// The reachable panic, end to end. `--width 4000000000` used to abort
    /// with `capacity overflow` (exit 101) inside `Image::new`; the spec says an
    /// invalid dimension returns `Err` and exits non-zero.
    ///
    /// `4e9 / (16/9) = 2.25e9` rows, so the product is 9e18 pixels. That fits in
    /// a 64-bit `usize` (max 1.8446744e19), so nothing overflows -- it is
    /// `Image::new`'s `MAX_PIXELS` cap that turns it into an `Err`. Taken to
    /// the point where `usize` itself would overflow, `--width 9223372036854775807`
    /// gives 5.19e18 rows and a product of 4.79e37, and `checked_mul` catches
    /// that instead.
    #[test]
    fn rejects_a_width_whose_pixel_count_overflows_or_exceeds_the_cap() {
        for width in [4_000_000_000usize, 9_223_372_036_854_775_807] {
            let err = render_scene(&small(width, 2, DEFAULT_SEED)).unwrap_err();
            assert!(
                matches!(err, RenderError::InvalidDimension { .. }),
                "width {width} should have been refused, got {err:?}"
            );
        }
    }

    #[test]
    fn accepts_the_smallest_size_the_guard_allows() {
        // 4 / (16/9) = 2.25 truncates to 2.
        let image = render_scene(&small(4, 2, DEFAULT_SEED)).unwrap();
        assert_eq!((image.width, image.height), (4, 2));
        assert_eq!(image.pixels.len(), 8);
        assert!(image.pixels.iter().flatten().all(|c| c.is_finite()));
    }

    #[test]
    fn the_render_takes_its_size_from_the_options_not_from_a_pinned_camera() {
        // `Camera::pinhole` hardcodes 400 wide at 100 samples. If
        // `render_scene` reached for that instead of reading `opts`, the CLI
        // would quietly ignore --width and --samples and this test would fail
        // on the dimensions rather than on a two-minute render.
        let image = render_scene(&small(40, 2, DEFAULT_SEED)).unwrap();
        assert_eq!((image.width, image.height), (40, 22));
    }

    #[test]
    fn an_unknown_scene_never_reaches_the_renderer() {
        // `Options::scene` is a `Scene`, not a `String`, so a name the CLI would
        // have refused cannot be built here at all. That is the whole of the
        // guarantee this used to make by re-checking the name at render time,
        // and it is why the renderer no longer carries that check: the type
        // makes the error unrepresentable rather than merely unlikely.
        assert!(Scene::parse("nope").is_err());
    }

    #[test]
    fn the_cover_scene_renders_through_the_same_path() {
        // Proves the cover wiring end to end at a size that costs nothing: the
        // scene builds, the cover lens is used, and pixels come out finite and
        // in range. 8 / (16/9) = 4.5 truncates to 4, so this is 8x4 = 32 pixels.
        let image = render_scene(&small_scene(
            Scene::Cover,
            COVER_TEST_WIDTH,
            COVER_TEST_SAMPLES,
            DEFAULT_SEED,
        ))
        .unwrap();
        assert_eq!((image.width, image.height), (8, 4));
        assert_eq!(image.pixels.len(), 32);
        assert!(
            image
                .pixels
                .iter()
                .flatten()
                .all(|c| c.is_finite() && (0.0..=1.0).contains(c)),
            "every cover channel should be finite and inside [0, 1]"
        );
        // And the cover is not the spheres scene wearing a new name: a field of
        // matte spheres has no blue in it and plenty of it, so at least one
        // pixel is off-black.
        assert!(
            image.pixels.iter().any(|p| *p != [0.0, 0.0, 0.0]),
            "a cover render of 32 pixels should not be entirely black"
        );
    }

    #[test]
    fn the_cover_scene_honours_the_command_lines_width_and_samples() {
        // `Camera::cover` is the cover render's own 1200x675 at 500. Calling it from
        // `render_scene` would ignore `--width` and `--samples` outright, and
        // this test is what says it does not: 20 wide is 20x11 --
        // 20 / (16/9) = 11.25 truncates to 11 -- at 2 samples.
        let image = render_scene(&small_scene(Scene::Cover, 20, 2, DEFAULT_SEED)).unwrap();
        assert_eq!((image.width, image.height), (20, 11));
    }

    #[test]
    fn the_cover_scene_is_reproducible_from_its_seed() {
        // The cover draws from the shared generator *before* the render does, so
        // this asserts more than the spheres test above: the same seed has to
        // reproduce the sphere layout and the sampling together. Two different
        // seeds have to move the pixels, which is what says the layout really is
        // seed-driven and not merely fixed.
        let a = render_scene(&small_scene(Scene::Cover, 8, 1, DEFAULT_SEED)).unwrap();
        let b = render_scene(&small_scene(Scene::Cover, 8, 1, DEFAULT_SEED)).unwrap();
        assert_eq!(a.pixels, b.pixels, "one seed must give one cover image");

        let c = render_scene(&small_scene(Scene::Cover, 8, 1, DEFAULT_SEED + 1)).unwrap();
        assert_ne!(
            a.pixels, c.pixels,
            "a different seed must move the cover, or the layout is not seed-driven"
        );
    }

    #[test]
    fn gamma_is_on_and_is_the_encode_that_reaches_the_file() {
        assert!(GAMMA, "gamma correction stays on");

        let image = render_scene(&small(6, 1, DEFAULT_SEED)).unwrap();
        // 6 / (16/9) = 3.375 truncates to 3.
        assert_eq!((image.width, image.height), (6, 3));

        let path = std::env::temp_dir().join("gi_renderer_gamma.ppm");
        image.write_ppm(&path, GAMMA).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let header = b"P6\n6 3\n255\n";
        assert_eq!(&bytes[..header.len()], header);
        let body = &bytes[header.len()..];

        // Every byte is the sqrt of the linear channel, not the channel itself:
        // `write_ppm` is the only place gamma is applied, so this is where the
        // committed images pick it up.
        for (index, pixel) in image.pixels.iter().enumerate() {
            for (channel, &linear) in pixel.iter().enumerate() {
                assert_eq!(
                    body[index * 3 + channel],
                    (256.0 * linear.sqrt().clamp(0.0, 0.999)) as u8,
                    "pixel {index} channel {channel} is {linear}"
                );
            }
        }

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn same_seed_produces_an_identical_image() {
        let a = render_scene(&small(40, 2, DEFAULT_SEED)).unwrap();
        let b = render_scene(&small(40, 2, DEFAULT_SEED)).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }

    #[test]
    fn a_different_seed_moves_the_pixels() {
        // Same geometry, same sample count, one seed apart: if the seed were
        // dropped on the floor both renders would be byte-identical, and the
        // determinism test above would be passing for the wrong reason.
        let a = render_scene(&small(40, 2, DEFAULT_SEED)).unwrap();
        let b = render_scene(&small(40, 2, DEFAULT_SEED + 1)).unwrap();
        assert_ne!(a.pixels, b.pixels);
    }
}
