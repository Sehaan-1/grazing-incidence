//! Rendering entry point.

use crate::camera::{
    CHAPTER_TWELVE_ASPECT, CHAPTER_TWELVE_DEPTH, CHAPTER_TWELVE_LOOKAT, CHAPTER_TWELVE_LOOKFROM,
    CHAPTER_TWELVE_VFOV, CHAPTER_TWELVE_VUP, Camera,
};
use crate::cli::Options;
use crate::error::RenderError;
use crate::hittable::HittableList;
use crate::image::Image;
use crate::rng::Rng;
use crate::scene::{self, Scene};

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = true;

/// Cast `opts.samples_per_pixel` jittered rays per pixel through the §12
/// camera and shade each hit (§6, §8, §12).
///
/// The framing is the book's §12 listing -- a 90-degree lens from
/// (-2, 2, 1) looking at (0, 0, -1) -- while the width, the sample count, the
/// seed and the scene come from the command line. §12 is what makes the width
/// the only dimension a caller has to give: the height follows from the 16:9
/// aspect ratio.
pub fn render_scene(opts: &Options) -> Result<Image, RenderError> {
    let scene = Scene::from_name(&opts.scene).ok_or_else(|| RenderError::UnknownScene {
        name: opts.scene.clone(),
    })?;

    let camera = Camera::new(
        opts.width,
        opts.samples_per_pixel,
        CHAPTER_TWELVE_DEPTH,
        CHAPTER_TWELVE_ASPECT,
        CHAPTER_TWELVE_VFOV,
        CHAPTER_TWELVE_LOOKFROM,
        CHAPTER_TWELVE_LOOKAT,
        CHAPTER_TWELVE_VUP,
    );

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

    let objects = scene::build(scene);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    let mut rng = Rng::seed_from_u64(opts.seed);
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

    #[test]
    fn accepts_the_smallest_size_the_guard_allows() {
        // 4 / (16/9) = 2.25 truncates to 2.
        let image = render_scene(&small(4, 2, DEFAULT_SEED)).unwrap();
        assert_eq!((image.width, image.height), (4, 2));
        assert_eq!(image.pixels.len(), 8);
        assert!(image.pixels.iter().flatten().all(|c| c.is_finite()));
    }

    #[test]
    fn the_render_takes_its_size_from_the_options_not_from_the_chapter() {
        // `Camera::chapter_twelve` hardcodes 400 wide at 100 samples. If
        // `render_scene` reached for that instead of reading `opts`, the CLI
        // would quietly ignore --width and --samples and this test would fail
        // on the dimensions rather than on a two-minute render.
        let image = render_scene(&small(40, 2, DEFAULT_SEED)).unwrap();
        assert_eq!((image.width, image.height), (40, 22));
    }

    #[test]
    fn an_unknown_scene_is_rejected_here_too() {
        // The CLI already refuses these, so this only fires if `Options` is
        // built by hand -- but then the renderer must not fall back to a
        // default scene without saying so.
        let opts = Options {
            scene: "nope".to_string(),
            ..small(4, 2, DEFAULT_SEED)
        };
        assert!(matches!(
            render_scene(&opts).unwrap_err(),
            RenderError::UnknownScene { ref name } if name == "nope"
        ));
    }

    #[test]
    fn gamma_is_on_and_is_the_encode_that_reaches_the_file() {
        assert!(GAMMA, "gamma correction landed in §9 and stays on");

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
