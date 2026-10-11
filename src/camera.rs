//! The camera and the shading loop.

use crate::error::RenderError;
use crate::hittable::Hittable;
use crate::image::Image;
use crate::ray::Ray;
use crate::rng::Rng;
use crate::vec3::Vec3;

/// The book's §12 listing, split into constants so `chapter_twelve`, the
/// renderer and the CLI all read the framing off one place instead of each
/// restating it.
pub const CHAPTER_TWELVE_WIDTH: usize = 400;
pub const CHAPTER_TWELVE_SAMPLES: usize = 100;
pub const CHAPTER_TWELVE_DEPTH: usize = 50;
pub const CHAPTER_TWELVE_ASPECT: f64 = 16.0 / 9.0;
pub const CHAPTER_TWELVE_VFOV: f64 = 90.0;
pub const CHAPTER_TWELVE_LOOKFROM: Vec3 = Vec3::new(-2.0, 2.0, 1.0);
pub const CHAPTER_TWELVE_LOOKAT: Vec3 = Vec3::new(0.0, 0.0, -1.0);
pub const CHAPTER_TWELVE_VUP: Vec3 = Vec3::new(0.0, 1.0, 0.0);

/// §13 narrows the lens from 90 degrees to 20 and opens it to a 10-degree
/// aperture focused at 3.4. The eye and the target are unchanged from §12 --
/// only the framing and the aperture move, so they keep the §12 constants.
pub const CHAPTER_THIRTEEN_VFOV: f64 = 20.0;
pub const CHAPTER_THIRTEEN_DEFOCUS_ANGLE: f64 = 10.0;
pub const CHAPTER_THIRTEEN_FOCUS_DIST: f64 = 3.4;

/// §14's camera, split into constants for the same reason §12's were: the
/// renderer builds a cover camera from the command line's width and sample
/// count and has to read the rest of the framing off one place.
pub const COVER_WIDTH: usize = 1200;
pub const COVER_SAMPLES: usize = 500;
pub const COVER_DEPTH: usize = 50;
pub const COVER_ASPECT: f64 = 16.0 / 9.0;
pub const COVER_VFOV: f64 = 20.0;
pub const COVER_LOOKFROM: Vec3 = Vec3::new(13.0, 2.0, 3.0);
pub const COVER_LOOKAT: Vec3 = Vec3::ZERO;
pub const COVER_VUP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
pub const COVER_DEFOCUS_ANGLE: f64 = 0.6;
pub const COVER_FOCUS_DIST: f64 = 10.0;

/// A camera: a pinhole from §7, positioned and aimed from §12, given a
/// lens aperture in §13.
///
/// §12 is where the image stops being a fixed viewport seen from the origin.
/// The eye moves to `lookfrom`, the frame is built out of `lookat` and `vup`
/// instead of out of the world axes, and the vertical field of view becomes a
/// number rather than the implicit `viewport_height = 2`.
///
/// The orientation changed with it. §4-§7 anchored on the *bottom*-left
/// viewport corner and counted `j` upward, so `j = 0` was the bottom of the
/// picture and `render` had to mirror its write index. §12 anchors on
/// `pixel00_loc` -- the *top*-left -- and its `pixel_delta_v` already points
/// down, because `viewport_v = viewport_height * -v` flips the camera's own up
/// vector. So `j = 0` is the top row and no mirror is needed. `render` writes
/// `(i, j)` straight through; see `render`.
///
/// §13 gives the camera a lens instead of a pinhole. Ray origins stop being
/// `lookfrom` and become a uniform point on a defocus disk centred there, sized
/// by `focus_dist` and `defocus_angle`. The orientation above is untouched:
/// the disk is spanned by the same `u` and `v`, and the picture still reads
/// row `j` as row `j`. At `defocus_angle = 0` the disk has zero radius and the
/// camera is exactly the §7 pinhole again.
pub struct Camera {
    pub image_width: usize,
    pub samples_per_pixel: usize,
    pub max_depth: usize,
    pub aspect_ratio: f64,
    pub vfov: f64,
    pub lookfrom: Vec3,
    pub lookat: Vec3,
    pub vup: Vec3,

    /// The aperture (§13), kept public so a caller can tell a pinhole from a
    /// thin lens without re-deriving it. `defocus_angle` is the full opening
    /// angle of the lens in degrees; `focus_dist` is how far in front of the
    /// eye the plane of focus sits.
    pub defocus_angle: f64,
    pub focus_dist: f64,

    pub image_height: usize,
    pixel00_loc: Vec3,
    pixel_delta_u: Vec3,
    pixel_delta_v: Vec3,

    /// The aperture's own basis vectors (§13): `defocus_disk_u = u * radius`
    /// and `defocus_disk_v = v * radius`. Together they span the disk that ray
    /// origins are scattered over. At `defocus_angle = 0` both are zero and the
    /// disk collapses to `lookfrom`, which is the pinhole camera of §7-§12.
    defocus_disk_u: Vec3,
    defocus_disk_v: Vec3,

    /// The camera frame (§12). `w` points from `lookat` back to `lookfrom`, `u`
    /// runs right across the picture and `v` runs up it; all three are unit
    /// and mutually perpendicular. They are kept on the camera rather than
    /// discarded at construction so the frame can be inspected.
    pub u: Vec3,
    pub v: Vec3,
    pub w: Vec3,
}

impl Camera {
    /// §12's height rule, on its own: `image_width / aspect_ratio`, truncated
    /// toward zero, then floored at 1.
    ///
    /// The floor is what stops a very wide viewport -- or a very small width --
    /// from asking for a zero-row image. The CLI needs this before it has a
    /// `Camera` to ask, which is why it lives here rather than inline in
    /// `new`.
    pub fn derived_height(image_width: usize, aspect_ratio: f64) -> usize {
        ((image_width as f64 / aspect_ratio) as usize).max(1)
    }

    /// Build the camera frame (§12).
    ///
    /// `lookfrom` is the eye, `lookat` the point it looks at and `vup` the
    /// world direction that counts as "up" in the picture. `vfov` is the
    /// vertical field of view in degrees; the horizontal one follows from
    /// `aspect_ratio`, so the caller gives a width and gets a height.
    ///
    /// `u`, `v` and `w` are the orthonormal frame the picture is drawn in:
    /// `w` points back from the target toward the eye, `u` runs right and `v`
    /// runs up. The view's `viewport_v` is `viewport_height * -v`, so image
    /// rows increase downward.
    ///
    /// `defocus_angle` is the lens aperture in degrees and `focus_dist` is the
    /// distance at which the lens is focused (§13). Both only affect where ray
    /// *origins* land; the viewport above, and therefore every framing
    /// property this camera has, is independent of them. A `defocus_angle` of
    /// zero is a pinhole.
    ///
    /// Ten parameters trips clippy's `too_many_arguments`, which counts `self`
    /// and defaults to a threshold of seven. The arguments are a closed
    /// record of the book's camera settings rather than a bundle of behaviour,
    /// so the attribute is the honest answer here.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        image_width: usize,
        samples_per_pixel: usize,
        max_depth: usize,
        aspect_ratio: f64,
        vfov: f64,
        lookfrom: Vec3,
        lookat: Vec3,
        vup: Vec3,
        defocus_angle: f64,
        focus_dist: f64,
    ) -> Self {
        let image_height = Self::derived_height(image_width, aspect_ratio);

        // §12: the eye is `focal_length` in front of the target, and the
        // viewport is the plane of width `2 h focal_length` at the eye,
        // half-angles at the edges of `vfov`.
        let focal_length = (lookfrom - lookat).length();
        let theta = crate::degrees_to_radians(vfov);
        let h = (theta / 2.0).tan();
        let viewport_height = 2.0 * h * focal_length;
        let viewport_width = viewport_height * (image_width as f64 / image_height as f64);

        let w = (lookfrom - lookat).unit();
        let u = vup.cross(w).unit();
        let v = w.cross(u);

        let viewport_u = viewport_width * u;
        let viewport_v = viewport_height * -v;

        let pixel_delta_u = viewport_u / image_width as f64;
        let pixel_delta_v = viewport_v / image_height as f64;

        // The upper *left* corner, then the centre of pixel (0, 0). This is
        // the half-pixel the book's `0.5 *` exists for: without it, pixel (0, 0)
        // would be sampled at the corner rather than the middle of its square.
        let viewport_upper_left = lookfrom - focal_length * w - viewport_u / 2.0 - viewport_v / 2.0;
        let pixel00_loc = viewport_upper_left + 0.5 * (pixel_delta_u + pixel_delta_v);

        // §13: the defocus disk is the lens aperture. Rays start anywhere on it
        // rather than all at `lookfrom`, which spreads out-of-focus rays the way
        // a real lens does. The half-angle, not the full angle, is what fixes
        // the radius: the cone of rays the lens admits half-angles at
        // `defocus_angle / 2`, and at `focus_dist` that cone is a disk of
        // radius `focus_dist * tan(that half-angle)`.
        //
        // `defocus_angle = 0` gives a radius of exactly 0, so the disk is the
        // point `lookfrom` and the camera is the §7 pinhole.
        let defocus_radius = focus_dist * (crate::degrees_to_radians(defocus_angle / 2.0)).tan();
        let defocus_disk_u = u * defocus_radius;
        let defocus_disk_v = v * defocus_radius;

        Self {
            image_width,
            samples_per_pixel,
            max_depth,
            aspect_ratio,
            vfov,
            lookfrom,
            lookat,
            vup,
            defocus_angle,
            focus_dist,
            image_height,
            pixel00_loc,
            pixel_delta_u,
            pixel_delta_v,
            defocus_disk_u,
            defocus_disk_v,
            u,
            v,
            w,
        }
    }

    /// The §12 listing: 400px wide, 16:9, a 90-degree lens from (-2, 2, 1)
    /// looking at the origin's -z end. §12 has no aperture, so the last two
    /// arguments pin it shut: a zero `defocus_angle` collapses the defocus disk
    /// to a point and every ray leaves from `lookfrom`.
    pub fn chapter_twelve() -> Self {
        Self::new(
            CHAPTER_TWELVE_WIDTH,
            CHAPTER_TWELVE_SAMPLES,
            CHAPTER_TWELVE_DEPTH,
            CHAPTER_TWELVE_ASPECT,
            CHAPTER_TWELVE_VFOV,
            CHAPTER_TWELVE_LOOKFROM,
            CHAPTER_TWELVE_LOOKAT,
            CHAPTER_TWELVE_VUP,
            0.0,
            10.0,
        )
    }

    /// The §13 listing: the same eye and target as §12, but a 20-degree lens
    /// behind a 10-degree aperture focused at 3.4.
    ///
    /// `focus_dist = 3.4` is almost the `3.4641...` from the eye to the middle
    /// ball, so the three spheres sit on the plane of focus and the ground
    /// running away to the horizon -- infinitely far, in the limit -- does not.
    /// That is the whole of the chapter's image: a wide lens close to the
    /// subject blurs everything behind it.
    ///
    /// The frame size and the aspect ratio are read off the §12 constants
    /// because §13 does not change either; only the lens and the aperture move.
    pub fn chapter_thirteen() -> Self {
        Self::new(
            CHAPTER_TWELVE_WIDTH,
            CHAPTER_TWELVE_SAMPLES,
            CHAPTER_TWELVE_DEPTH,
            CHAPTER_TWELVE_ASPECT,
            CHAPTER_THIRTEEN_VFOV,
            CHAPTER_TWELVE_LOOKFROM,
            CHAPTER_TWELVE_LOOKAT,
            CHAPTER_TWELVE_VUP,
            CHAPTER_THIRTEEN_DEFOCUS_ANGLE,
            CHAPTER_THIRTEEN_FOCUS_DIST,
        )
    }

    /// The §14 listing: the cover render's framing, at the cover render's own
    /// size.
    ///
    /// 1200 wide at 500 samples, a 20-degree lens from (13, 2, 3) looking at the
    /// origin, behind a 0.6-degree aperture focused at 10. The focal length is
    /// `|lookfrom - lookat| = sqrt(13^2 + 2^2 + 3^2) = sqrt(182) = 13.4907...`,
    /// and the height follows from the aspect ratio as
    /// `1200 / (16/9) = 1200 * 9 / 16 = 675` exactly.
    ///
    /// This is the *listing*, not what `--width` and `--samples` produce: the
    /// renderer builds the same lens from `COVER_*` and the command line's
    /// numbers, the way §13's camera is built rather than called. Rendering it
    /// as written is 1200 * 675 * 500 = 405 million primary rays against a
    /// world of roughly 488 spheres at up to 50 bounces each.
    pub fn cover() -> Self {
        Self::new(
            COVER_WIDTH,
            COVER_SAMPLES,
            COVER_DEPTH,
            COVER_ASPECT,
            COVER_VFOV,
            COVER_LOOKFROM,
            COVER_LOOKAT,
            COVER_VUP,
            COVER_DEFOCUS_ANGLE,
            COVER_FOCUS_DIST,
        )
    }

    /// Cast one ray per sample through the viewport and shade it (§6, §7, §8).
    ///
    /// Every sample is a fresh jittered ray from `rng`, so the same generator
    /// state replays the same image and the test suite can assert on exact
    /// pixel values. `renderer::render_scene` owns the dimension check;
    /// `new` returns `Self`, not a `Result`.
    ///
    /// §13 costs two more draws per sample, because a sample with an open
    /// aperture also draws a point on the defocus disk. A pinned camera draws
    /// none: `get_ray` short-circuits on `defocus_angle <= 0` before touching
    /// `rng`. That is what keeps the exact-pixel tests below exact, since they
    /// index the stream at two draws per sample.
    ///
    /// Row `j` is written to image row `j`, unmirrored. §12 anchored the
    /// viewport on `pixel00_loc`, the top-left, and gave the camera a downward
    /// `pixel_delta_v`, so `j = 0` is already the top of the picture.
    pub fn render(&self, world: &dyn Hittable, rng: &mut Rng) -> Result<Image, RenderError> {
        let mut image = Image::new(self.image_width, self.image_height)?;

        for j in 0..self.image_height {
            eprint!("\rScanlines remaining: {} ", self.image_height - j);
            for i in 0..self.image_width {
                let mut color = Vec3::ZERO;
                for _ in 0..self.samples_per_pixel {
                    let du = rng.range(0.0, 1.0);
                    let dv = rng.range(0.0, 1.0);
                    let ray = self.get_ray(i, j, du, dv, rng);
                    color += self.ray_color(&ray, self.max_depth, world, rng);
                }
                image.set(i, j, (color / self.samples_per_pixel as f64).to_rgb());
            }
        }
        eprintln!("\rDone.                 ");

        Ok(image)
    }

    /// A camera ray through pixel (i, j), offset by the (du, dv) sample.
    ///
    /// The sample lands at `(i + du) / image_width` and `(j + dv) /
    /// image_height` across the viewport, so `i + du` runs over `[i, i + 1)` and
    /// `image_width` samples cover the viewport edge to edge. Normalising by
    /// `image_width - 1` instead would push the last row and column past the
    /// far edge -- pixel 7 of an 8-wide camera would reach `u = (7 + du) / 7`,
    /// which is at least 1 -- and would leave the first row and column a full
    /// sample short of the near edge. The book makes the same choice.
    ///
    /// `j` counts *down*. That is the whole of §12's orientation change and it
    /// is why `render` no longer mirrors its write index.
    ///
    /// §13 splits the ray's two ends apart. `pixel_sample` -- the point on the
    /// viewport -- is unchanged, so the framing is unchanged. The *origin* is a
    /// point on the defocus disk instead of `lookfrom`, and the direction runs
    /// from there to `pixel_sample`. Rays that hit the same point on the
    /// viewport from different points on the lens are what make an
    /// out-of-focus surface spread.
    pub fn get_ray(&self, i: usize, j: usize, du: f64, dv: f64, rng: &mut Rng) -> Ray {
        let pixel_sample = self.pixel00_loc
            + self.pixel_delta_u * (i as f64 + du)
            + self.pixel_delta_v * (j as f64 + dv);

        // A zero aperture is a pinhole: the disk has zero radius, so the sample
        // would be `lookfrom` exactly, but taking the branch outright also keeps
        // the generator untouched and this a two-draw sample.
        let ray_origin = if self.defocus_angle <= 0.0 {
            self.lookfrom
        } else {
            self.defocus_disk_sample(rng)
        };

        Ray::new(ray_origin, pixel_sample - ray_origin)
    }

    /// A random point on the defocus disk (§13).
    ///
    /// `Rng::in_unit_disk` rejects until it has a point strictly inside the
    /// unit disk, so `|p| < 1` and the sample is at most `defocus_radius` from
    /// `lookfrom` -- never more, and never on the rim in practice.
    pub fn defocus_disk_sample(&self, rng: &mut Rng) -> Vec3 {
        let p = rng.in_unit_disk();
        self.lookfrom + self.defocus_disk_u * p.x + self.defocus_disk_v * p.y
    }

    /// Shade one ray: a hit bounces off its material, a miss is the sky (§9).
    ///
    /// `depth` counts the bounces still available, so a camera with none left
    /// renders black. Every bounce draws from `rng`, the same generator that
    /// places the samples, so a render is reproducible end to end.
    fn ray_color(&self, ray: &Ray, depth: usize, world: &dyn Hittable, rng: &mut Rng) -> Vec3 {
        if depth == 0 {
            return Vec3::ZERO;
        }

        // §9: a t_min of 0.001 skips the self-intersection that floating-point
        // rounding introduces. This is the "shadow acne" the book fixes here.
        if let Some(rec) = world.hit(ray, 0.001, f64::INFINITY) {
            if let Some((scattered, attenuation)) = rec.material.scatter(ray, &rec, rng) {
                return attenuation * self.ray_color(&scattered, depth - 1, world, rng);
            }
            return Vec3::ZERO;
        }

        sky_color(ray.direction)
    }
}

/// The sky: white at the horizon, blue overhead (§4).
fn sky_color(direction: Vec3) -> Vec3 {
    let unit_direction = direction.unit();
    let a = 0.5 * (unit_direction.y + 1.0);
    Vec3::UNIT * (1.0 - a) + Vec3::new(0.5, 0.7, 1.0) * a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hittable::HittableList;
    use crate::scene::{self, Scene};

    /// The seed the pixel-exact tests below are derived against.
    const SEED: u64 = 1;

    /// A camera on the book's axis: eye at the origin, looking down -z, +y up,
    /// a 90-degree lens.
    ///
    /// Every test that needs a closed form uses this one, because a frame
    /// aligned with the world axes can be worked out on paper. §12's own
    /// framing is tested through `Camera::chapter_twelve`, where the frame is
    /// oblique and only its invariants -- orthonormality, ordering -- survive
    /// without a render.
    ///
    /// The aperture is pinned shut at 0.0, which §13 needs for two reasons: the
    /// ray origin is then exactly `lookfrom`, so the directions below stay
    /// closed-form, and `get_ray` draws nothing from `rng`, so a sample stays
    /// worth two draws and `first_draw` still indexes the stream.
    fn axis_camera(image_width: usize, samples: usize, depth: usize, aspect: f64) -> Camera {
        Camera::new(
            image_width,
            samples,
            depth,
            aspect,
            90.0,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
            0.0,
            10.0,
        )
    }

    /// The §6 world. `HittableList` borrows the objects, so the caller has to
    /// hold them alive for as long as the list is used.
    fn test_world<'a>(objects: &'a [Box<dyn Hittable>]) -> HittableList<'a> {
        let mut list = HittableList::new();
        for object in objects {
            list.add(object.as_ref());
        }
        list
    }

    /// A world with nothing in it, so every ray misses and every sample costs
    /// exactly the two draws `render` makes before `get_ray`.
    ///
    /// This is what keeps the pixel-exact tests below exact. §9 gave the
    /// generator a second job: a scattered ray draws from the same `Rng` that
    /// places the samples, and how many draws that costs depends on the path
    /// the ray takes. Against the §6 scene, pixel (i, j)'s `du` is no longer at
    /// a fixed offset into the stream, because every pixel ahead of it may have
    /// spent a different number of draws. Against an empty world the
    /// arithmetic in `draw` and `first_draw` holds exactly.
    ///
    /// §13 adds a third caller of the generator -- the defocus disk -- but only
    /// for a camera with an open aperture, and `empty_world` is only ever
    /// paired with `axis_camera`'s. So "two draws" survives §13 unchanged.
    fn empty_world() -> HittableList<'static> {
        HittableList::new()
    }

    /// The book's normal colour map: each normal component maps from [-1, 1] to
    /// [0, 1] (§6.1, unchanged through §7).
    ///
    /// §9 replaced this with `Material::scatter`, so nothing in the renderer
    /// calls it any more. It stays here, in the tests, because the test below
    /// still pins what the map did.
    fn shade(normal: Vec3) -> Vec3 {
        normal * 0.5 + Vec3::UNIT * 0.5
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    fn assert_vec_close(a: Vec3, b: Vec3) {
        close(a.x, b.x);
        close(a.y, b.y);
        close(a.z, b.z);
    }

    /// The `n`-th `f64()` a seeded generator yields, replayed from scratch.
    ///
    /// `render` consumes the generator in one fixed order -- scanline `j`, then
    /// column `i`, then sample, two draws per ray, `du` before `dv` -- so any
    /// one sample can be reproduced without rendering the whole image, as long
    /// as the world draws nothing of its own. Against `empty_world` that holds;
    /// against the §6 scene it does not, because §9 spends extra draws per
    /// scatter. It also only holds for a pinned camera: §13's disk draw would
    /// make it four per ray.
    fn draw(seed: u64, n: usize) -> f64 {
        let mut rng = Rng::seed_from_u64(seed);
        for _ in 0..n {
            rng.f64();
        }
        rng.f64()
    }

    /// Where pixel (i, j)'s first draw begins in that stream.
    ///
    /// Exact only while nothing ahead of (i, j) consumes extra draws, so every
    /// caller pairs this with `empty_world` and with `axis_camera`'s shut
    /// aperture.
    fn first_draw(camera: &Camera, i: usize, j: usize) -> usize {
        2 * ((j * camera.image_width + i) * camera.samples_per_pixel)
    }

    /// Sample `k` of pixel (i, j), from the generator state `render` had
    /// reached at that point. Only exact against `empty_world`, for the reason
    /// `first_draw` gives.
    ///
    /// The `rng` handed to `get_ray` is the one the replay has left. `axis_camera`
    /// is pinned shut, so `get_ray` never reads from it -- but passing a live
    /// generator keeps this honest if the aperture ever opens.
    fn sample_ray(camera: &Camera, seed: u64, i: usize, j: usize, k: usize) -> Ray {
        let mut rng = Rng::seed_from_u64(seed);
        for _ in 0..first_draw(camera, i, j) + 2 * k {
            rng.f64();
        }
        let du = rng.range(0.0, 1.0);
        let dv = rng.range(0.0, 1.0);
        camera.get_ray(i, j, du, dv, &mut rng)
    }

    #[test]
    fn render_produces_the_requested_pixel_count() {
        let objects = scene::build(Scene::Spheres, &mut Rng::seed_from_u64(SEED));
        let world = test_world(&objects);
        // 20 / (20/12) = 12 exactly, so the height comes back as asked for.
        let camera = axis_camera(20, 1, 2, 20.0 / 12.0);
        let image = camera
            .render(&world, &mut Rng::seed_from_u64(SEED))
            .unwrap();
        assert_eq!(image.pixels.len(), 20 * 12);
        assert_eq!(image.width, 20);
        assert_eq!(image.height, 12);
    }

    #[test]
    fn height_follows_from_the_aspect_ratio() {
        let camera = Camera::chapter_twelve();
        assert_eq!(camera.image_width, 400);
        // 400 / (16/9) = 225 exactly in f64 -- verified, not assumed -- and
        // truncating it gives 225 rows for the book's 400x225 frame.
        assert_eq!(400.0_f64 / (16.0_f64 / 9.0), 225.0);
        assert_eq!(camera.image_height, 225);
    }

    #[test]
    fn derived_height_floors_at_one() {
        // 1 / 1000 = 0.001, which truncates to 0.
        assert_eq!(Camera::derived_height(1, 1000.0), 1);
        assert_eq!(Camera::derived_height(400, 16.0 / 9.0), 225);
    }

    #[test]
    fn camera_frame_vectors_are_orthonormal() {
        // lookfrom - lookat = (-2, 2, 2), length sqrt(12) = 2 sqrt(3), so
        //     w = (-1, 1, 1) / sqrt(3)
        // vup x w = (1/sqrt(3), 0, 1/sqrt(3)), length sqrt(2/3), so
        //     u = (1, 0, 1) / sqrt(2)
        // and w x u = (1, 2, -1) / sqrt(6), whose squared length is
        // (1 + 4 + 1) / 6 = 1 with no rescale needed.
        //
        // Each dot product cancels by pairing the equal and opposite terms:
        //   u . v = (1/2)(1/sqrt(6)) + 0 + (1/2)(-1/sqrt(6)) = 0
        //   u . w = -(1/sqrt(6)) + 0 + (1/sqrt(6))         = 0
        //   v . w = -(1/sqrt(18)) + 2/sqrt(18) - 1/sqrt(18) = 0
        let camera = Camera::chapter_twelve();
        let (u, v, w) = (camera.u, camera.v, camera.w);

        assert!((u.length() - 1.0).abs() < 1e-9, "u is not unit: {:?}", u);
        assert!((v.length() - 1.0).abs() < 1e-9, "v is not unit: {:?}", v);
        assert!((w.length() - 1.0).abs() < 1e-9, "w is not unit: {:?}", w);
        assert!(u.dot(v).abs() < 1e-9, "u . v = {}", u.dot(v));
        assert!(u.dot(w).abs() < 1e-9, "u . w = {}", u.dot(w));
        assert!(v.dot(w).abs() < 1e-9, "v . w = {}", v.dot(w));

        // The derived form of the same frame. If `u` lost its `.unit()`, or `v`
        // stopped being `w x u`, this is what notices.
        let root2 = 2.0_f64.sqrt();
        let root3 = 3.0_f64.sqrt();
        let root6 = 6.0_f64.sqrt();
        assert_vec_close(w, Vec3::new(-1.0 / root3, 1.0 / root3, 1.0 / root3));
        assert_vec_close(u, Vec3::new(1.0 / root2, 0.0, 1.0 / root2));
        assert_vec_close(v, Vec3::new(1.0 / root6, 2.0 / root6, -1.0 / root6));
    }

    #[test]
    fn aspect_ratio_never_yields_a_zero_height() {
        let camera = axis_camera(1, 1, 1, 1000.0);
        assert_eq!(camera.image_height, 1);
    }

    #[test]
    fn a_narrower_lens_shrinks_the_viewport() {
        // focal_length = |(-2, 2, 1) - (0, 0, -1)| = |(-2, 2, 2)| = sqrt(12).
        // viewport_height = 2 tan(vfov / 2) sqrt(12), and
        //     |pixel_delta_u| = viewport_width / image_width
        //                    = viewport_height * (image_width / image_height) / image_width
        //                    = viewport_height / image_height
        // so the two pixel widths should land on that closed form, with the
        // 20-degree one well under the 90-degree one.
        let lookfrom = Vec3::new(-2.0, 2.0, 1.0);
        let lookat = Vec3::new(0.0, 0.0, -1.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let narrow = Camera::new(
            400,
            1,
            1,
            16.0 / 9.0,
            20.0,
            lookfrom,
            lookat,
            vup,
            0.0,
            10.0,
        );
        let wide = Camera::new(
            400,
            1,
            1,
            16.0 / 9.0,
            90.0,
            lookfrom,
            lookat,
            vup,
            0.0,
            10.0,
        );

        let expected = |vfov: f64| {
            2.0 * (crate::degrees_to_radians(vfov) / 2.0).tan() * 12.0_f64.sqrt() / 225.0
        };
        assert!((wide.pixel_delta_u.length() - expected(90.0)).abs() < 1e-12);
        assert!((narrow.pixel_delta_u.length() - expected(20.0)).abs() < 1e-12);
        assert!(
            narrow.pixel_delta_u.length() < wide.pixel_delta_u.length(),
            "a 20-degree lens should cover fewer pixels than a 90-degree one"
        );
    }

    #[test]
    fn jittered_rays_land_inside_their_own_pixel() {
        // An 8x8 axis camera. focal_length = 1, w = (0, 0, 1), u = (1, 0, 0),
        // v = (0, 1, 0), viewport_height = 2 and viewport_width = 2, so
        //     pixel_delta_u = (1/4, 0, 0)  and  pixel_delta_v = (0, -1/4, 0)
        //     viewport_upper_left = (0,0,0) - (0,0,1) - (1,0,0) - (0,-1,0)
        //                         = (-1, 1, -1)
        //     pixel00_loc = (-1, 1, -1) + (1/8, -1/8, 0) = (-7/8, 7/8, -1)
        // and the ray for (i, j, du, dv) points at
        //     (-7/8 + (i + du)/4,  7/8 - (j + dv)/4,  -1).
        //
        // The half-pixel offset is why x starts at -7/8 rather than -1: pixel
        // (0, 0) is sampled at the middle of its square, not its corner.
        let camera = axis_camera(8, 8, 5, 1.0);
        for j in 0..camera.image_height {
            for i in 0..camera.image_width {
                for k in 0..camera.samples_per_pixel {
                    let dir = sample_ray(&camera, SEED, i, j, k).direction;
                    // Column 0 starts at -7/8; the last column stops short of
                    // the far edge at 9/8, because `du < 1`.
                    assert!(
                        dir.x >= -0.875 && dir.x < 1.125,
                        "sample for pixel ({i}, {j}) left the viewport in x: {dir:?}"
                    );
                    // Row 0 stops at 7/8 and row 7 runs down past -1, because
                    // `pixel_delta_v` is negative. The bound is asymmetric and
                    // that asymmetry is the orientation.
                    assert!(
                        dir.y > -1.125 && dir.y <= 0.875,
                        "sample for pixel ({i}, {j}) left the viewport in y: {dir:?}"
                    );
                    // Nothing in the frame tilts, so z is exactly -1.
                    close(dir.z, -1.0);
                }
            }
        }
    }

    #[test]
    fn larger_j_points_lower() {
        // The orientation, stated as a property of the camera rather than of a
        // rendered pixel: one row down, one pixel down, no mirror anywhere.
        //
        // `t = tan(90 deg / 2)` is 0.9999999999999999, not 1 -- the closest
        // f64 to tan(pi/4) is one ulp low -- so every constant below is
        // written in terms of `t` rather than as a decimal:
        //     viewport_height   = 2 t          pixel_delta_v = -2 t / 8 = -t/4
        //     viewport_u width  = 2 t          pixel_delta_u =  2 t / 8 =  t/4
        //     upper-left x      = -t           upper-left y  =  t
        //     pixel00_loc       = (-7t/8, 7t/8, -1)
        // so a ray through (i, j) with sample (du, dv) points at
        //     (t (-7/8 + (i + du)/4),  t (7/8 - (j + dv)/4),  -1).
        let t = (crate::degrees_to_radians(90.0) / 2.0).tan();
        let camera = axis_camera(8, 1, 5, 1.0);
        let x = |i: usize, du: f64| t * (-7.0 / 8.0 + (i as f64 + du) / 4.0);
        let y = |j: usize, dv: f64| t * (7.0 / 8.0 - (j as f64 + dv) / 4.0);

        // §12 flips the frame: `viewport_v = viewport_height * -v` makes the row
        // step negative, so `j` counts down and the last row is the bottom.
        assert!(
            camera.pixel_delta_v.y < 0.0,
            "j must count downward, got {:?}",
            camera.pixel_delta_v
        );
        assert!(camera.pixel_delta_u.x > 0.0, "`u` still runs right");

        // The aperture is shut, so `get_ray` never reads the generator and one
        // throwaway per call is enough to hand it a live one.
        let mut rng = Rng::seed_from_u64(SEED);
        for (i, j, du, dv) in [
            (3usize, 0usize, 0.25, 0.25),
            (3, 1, 0.25, 0.25),
            (3, 7, 0.25, 0.25),
        ] {
            let dir = camera.get_ray(i, j, du, dv, &mut rng).direction;
            close(dir.x, x(i, du));
            close(dir.y, y(j, dv));
        }

        // One row is exactly one pixel step down, and the whole image is
        // `image_height` of them.
        let top = camera.get_ray(3, 0, 0.25, 0.25, &mut rng).direction;
        let below = camera.get_ray(3, 1, 0.25, 0.25, &mut rng).direction;
        let bottom = camera.get_ray(3, 7, 0.25, 0.25, &mut rng).direction;
        close(below.y - top.y, -t / 4.0);
        close(bottom.y - top.y, -7.0 * t / 4.0);
        assert!(top.y > below.y && below.y > bottom.y);

        // Right is still right: `pixel_delta_u` keeps the sign of `u`.
        let left = camera.get_ray(0, 3, 0.5, 0.5, &mut rng).direction;
        let right = camera.get_ray(7, 3, 0.5, 0.5, &mut rng).direction;
        close(left.x, x(0, 0.5));
        close(right.x, x(7, 0.5));
        close(right.x - left.x, 7.0 * t / 4.0);
    }

    #[test]
    fn the_viewport_follows_the_image_aspect_ratio() {
        // A 20-wide camera on 20:12. focal_length = 1 and viewport_height =
        // 2 tan(90/2), so viewport_width = 2 tan(90/2) * 20/12 and
        //     pixel_delta_u = (1/6, 0, 0)  and  pixel_delta_v = (0, -1/6, 0)
        //     viewport_upper_left = (-5/3, 1, -1)
        //     pixel00_loc = (-5/3, 1, -1) + (1/12, -1/12, 0) = (-19/12, 11/12, -1)
        // up to the same one-ulp-low tan(pi/4) that `larger_j_points_lower`
        // works around, which `close`'s 1e-9 more than covers.
        //
        // Pixel (0, 0)'s ray therefore points at
        //     (-19/12 + du/6,  11/12 - dv/6,  -1).
        let world = empty_world();
        let camera = axis_camera(20, 1, 5, 20.0 / 12.0);
        let image = camera
            .render(&world, &mut Rng::seed_from_u64(SEED))
            .unwrap();

        // One sample per pixel and (0, 0) is pixel 0 of the loop, so its draws
        // are numbers 0 and 1 of the stream.
        let n = first_draw(&camera, 0, 0);
        assert_eq!(n, 0);
        let du = draw(SEED, n);
        let dv = draw(SEED, n + 1);

        // §13's aperture is shut on this camera, so the generator `get_ray`
        // takes is never read and the ray is still the §7 pinhole ray.
        let mut rng = Rng::seed_from_u64(SEED);

        let direction = Vec3::new(-19.0 / 12.0 + du / 6.0, 11.0 / 12.0 - dv / 6.0, -1.0);
        assert_vec_close(camera.get_ray(0, 0, du, dv, &mut rng).direction, direction);

        // a = 0.5 * (unit.y + 1): blue is (1 - a) + a = 1, red gives up 0.5a
        // and green 0.3a. Pixel (0, 0) is the viewport's top-left square, so
        // §12 writes it to image row 0 with no mirror.
        assert_eq!(
            Vec3::from_rgb(image.pixels[0]),
            sky_color(direction),
            "the top-left pixel should be the sky in the top-left sample"
        );
        // 11/12 - dv/6 is at least 11/12 - 1/6 = 3/4, so this ray points up and
        // the miss really is the sky rather than the ground.
        assert!(direction.y > 0.75);
    }

    #[test]
    fn multiple_samples_average_jittered_rays() {
        // An empty world, so each sample costs exactly the two draws `render`
        // makes and `first_draw` still locates every sample.
        let world = empty_world();

        let mut rng = Rng::seed_from_u64(SEED);
        let once = axis_camera(8, 1, 5, 1.0).render(&world, &mut rng).unwrap();
        let mut rng = Rng::seed_from_u64(SEED);
        let four = axis_camera(8, 4, 5, 1.0).render(&world, &mut rng).unwrap();

        // One pixel's worth of samples, taken from the same stream `render`
        // walked: sample k of pixel (i, j) starts 2 * k draws after the pixel's
        // own first draw. Every pixel must come out as the mean of its
        // `samples_per_pixel` shaded samples -- summing instead of dividing
        // would be four times as bright and fail every row below.
        let camera = axis_camera(8, 4, 5, 1.0);
        for j in 0..camera.image_height {
            for i in 0..camera.image_width {
                let mut color = Vec3::ZERO;
                for k in 0..camera.samples_per_pixel {
                    let ray = sample_ray(&camera, SEED, i, j, k);
                    color += camera.ray_color(
                        &ray,
                        camera.max_depth,
                        &world,
                        &mut Rng::seed_from_u64(SEED),
                    );
                }
                // Row `j`, not row `image_height - 1 - j`: §12 counts down.
                assert_vec_close(
                    Vec3::from_rgb(four.pixels[j * four.width + i]),
                    color / camera.samples_per_pixel as f64,
                );
            }
        }

        // The two images share a seed, so the four-sample pixel's first sample
        // is the one-sample pixel's only sample. They still have to differ: if
        // the jitter were dropped, all four samples would be the same ray,
        // averaging would be a no-op, and the images would match.
        assert_ne!(once.pixels, four.pixels);
    }

    #[test]
    fn antialiasing_still_separates_against_the_real_scene() {
        // The averaging test above cannot use the §6 scene -- §9 spends extra
        // draws per scatter and the stream stops being indexable -- so this
        // keeps the same comparison alive where the pixels actually come from
        // materials: the two renders must not coincide.
        let objects = scene::build(Scene::Spheres, &mut Rng::seed_from_u64(SEED));
        let world = test_world(&objects);

        let mut rng = Rng::seed_from_u64(SEED);
        let once = axis_camera(8, 1, 5, 1.0).render(&world, &mut rng).unwrap();
        let mut rng = Rng::seed_from_u64(SEED);
        let four = axis_camera(8, 4, 5, 1.0).render(&world, &mut rng).unwrap();

        assert_ne!(once.pixels, four.pixels);
    }

    #[test]
    fn max_depth_zero_yields_black() {
        let objects = scene::build(Scene::Spheres, &mut Rng::seed_from_u64(SEED));
        let world = test_world(&objects);
        let camera = axis_camera(8, 1, 0, 8.0 / 5.0);
        let image = camera
            .render(&world, &mut Rng::seed_from_u64(SEED))
            .unwrap();
        // A zero-depth camera absorbs every ray, so the image is entirely black.
        assert!(image.pixels.iter().all(|p| p == &[0.0, 0.0, 0.0]));
    }

    #[test]
    fn sky_is_blue_overhead_and_white_straight_down() {
        // a = 0.5 * (unit.y + 1.0): +y is the tint, -y is white, y=0 is halfway.
        assert_vec_close(
            sky_color(Vec3::new(0.0, 1.0, 0.0)),
            Vec3::new(0.5, 0.7, 1.0),
        );
        assert_vec_close(
            sky_color(Vec3::new(0.0, -1.0, 0.0)),
            Vec3::new(1.0, 1.0, 1.0),
        );
        assert_vec_close(
            sky_color(Vec3::new(1.0, 0.0, 0.0)),
            Vec3::new(0.75, 0.85, 1.0),
        );

        // The three above are already unit vectors, so they cannot tell a
        // correct sky from one that skipped `.unit()`. (3,4,12) has length 13,
        // giving unit.y = 4/13 and a = 17/26, hence 9/26 white and 17/26 tint:
        // x = 9/26 + 17/52 = 35/52, y = 9/26 + 119/260 = 209/260, z = 1.
        assert_vec_close(
            sky_color(Vec3::new(3.0, 4.0, 12.0)),
            Vec3::new(35.0 / 52.0, 209.0 / 260.0, 1.0),
        );
    }

    #[test]
    fn sky_is_above_and_ground_below() {
        // An 8x8 axis camera: with t = tan(45 deg) and the viewport 2t tall and
        // 2t wide,
        //     pixel00_loc = (-7t/8, 7t/8, -1), pixel_delta_u = (t/4, 0, 0),
        //     pixel_delta_v = (0, -t/4, 0)
        // so the direction is t(-7/8 + (i + du)/4, 7/8 - (j + dv)/4, -1).
        // Column 0 puts x in [-7/8, -3/4) and rows 0 and 7 are the ends:
        //   row 0  y in (5/8, 7/8]   -- looking up
        //   row 7  y in [-9/8, -7/8) -- looking down
        // `t` is one ulp under 1, so the bounds below are rounded outwards.
        let objects = scene::build(Scene::Spheres, &mut Rng::seed_from_u64(SEED));
        let world = test_world(&objects);
        let camera = axis_camera(8, 1, 5, 1.0);
        let image = camera
            .render(&world, &mut Rng::seed_from_u64(SEED))
            .unwrap();
        let width = image.width;

        // Row 0 looks up: y is in (5/8, 7/8]. All four spheres miss over that
        // whole rectangle, whatever the draw.
        //
        //   ground  half_b = 100.5 y - 1 >= 100.5 * 5/8 - 1 = 61.8125 and
        //           c = 101.25 > 0, so sqrtd < half_b and both roots in
        //           `Sphere::hit` are negative.
        //   glass   disc = -0.75 x^2 - 2 x - 1.75 y^2 - 0.75, maximised at
        //           (x, y) = (-7/8, 5/8): -0.2578125 < 0.
        //   blue    disc = 1 - 0.75 (x^2 + y^2 + 1) <= 1 - 0.75 * 1.78125
        //           = -0.3359375 < 0.
        //   metal   half_b = -x - 1 <= -1/4, so half_b^2 <= 0.0625 against
        //           1.75 (x^2 + y^2 + 1) >= 3.11640625.
        //
        // The sky pins blue at exactly 1.0, because `sky_color` returns
        // (1 - a) + a. Over that rectangle the unit y runs from
        // (5/8) / sqrt(0.765625 + 0.390625 + 1) = 0.42562827 to
        // (7/8) / sqrt(0.390625 + 0.765625 + 1) = 0.59587957, so
        // a runs [0.71281413, 0.79793979], red gives up 0.5a and green 0.3a,
        // and red lands in [0.60103011, 0.64359293] and green in
        // [0.76061806, 0.78615576].
        let top = image.pixels[0];
        assert_eq!(top[2], 1.0, "sky blue is exactly 1, got {top:?}");
        assert!(
            top[0] > 0.6009 && top[0] < 0.6437,
            "sky red {top:?}, expected in (0.6009, 0.6437)"
        );
        assert!(
            top[1] > 0.7605 && top[1] < 0.7863,
            "sky green {top:?}, expected in (0.7605, 0.7863)"
        );

        // Row 7 looks down: y is in [-9/8, -7/8), and the same four checks
        // leave the ground as the only thing there to hit. The glass's disc tops
        // out at -0.9140625, the blue ball's at -0.6171875, and the metal ball
        // is out on x as above.
        //
        // So this pixel's first material is the ground, whose albedo §11 made
        // (0.8, 0.8, 0.0). That fixes two things exactly rather than by bound:
        //
        //   blue is exactly 0, because the attenuation multiplies through every
        //     remaining bounce and 0 times anything is 0. §9's neutral 0.5
        //     could not say that, which is why the sky-versus-ground split used
        //     to rest on a cap instead.
        //   red and green are at most 0.8, since 0.8 is the ground's own albedo
        //     and every albedo in the scene and every sky channel is <= 1. The
        //     tightest downstream albedo is §11's own glass at 1.0, so the 0.8
        //     here is the cap and not a bound on the chain.
        let bottom = image.pixels[7 * width];
        assert_eq!(
            bottom[2], 0.0,
            "a ground-first pixel has no blue at all, got {bottom:?}"
        );
        for (channel, value) in bottom.iter().enumerate() {
            assert!(
                *value >= 0.0 && *value <= 0.8,
                "ground channel {channel} should be in [0, 0.8], got {value} in {bottom:?}"
            );
        }

        // Reinstate `let row = self.image_height - 1 - j;` in `render` and these
        // two rows swap: the sky above is pinned to blue 1.0 and the ground
        // below to blue 0.0, so this is what turns round.
        assert!(
            top[2] > bottom[2],
            "sky should sit above ground, got top {top:?} and bottom {bottom:?}"
        );
    }

    #[test]
    fn a_column_reads_sky_then_ball_then_ground() {
        // An 8-wide camera on 8:16, so 16 rows. viewport_width = 2 * 8/16 = 1
        // and viewport_height = 2, giving
        //     pixel_delta_u = (1/8, 0, 0)  and  pixel_delta_v = (0, -1/8, 0)
        //     viewport_upper_left = (-1/2, 1, -1)
        //     pixel00_loc = (-1/2 + 1/16, 1 - 1/16, -1) = (-7/16, 15/16, -1)
        // and the direction is (-7/16 + (i + du)/8, 15/16 - (j + dv)/8, -1).
        //
        // Column 0 puts x in [-7/16, -5/16), which is inside the middle ball's
        // x range [-1/2, 1/2] and outside both side balls: the glass is at
        // x = -1 with r = 0.5 and needs x <= -0.4514 to be hit at all, and its
        // disc at (x, y) = (-7/16, 0) is -0.01855469 < 0. So column 0 sees
        // only sky, the middle ball and the ground, and the three occupy
        // cleanly separated bands of rows:
        //
        //   rows 0-2    y in (9/16, 15/16]   -- above the ball
        //   rows 5-9    |y| <= 5/16           -- inside it (the ball reaches
        //                                            |y| <= 0.37673211 at
        //                                            x = -7/16 and at most
        //                                            0.48546584 at x = -5/16)
        //   rows 12-15  |y| >= 9/16           -- below it
        //
        // and rows 3, 4, 10 and 11 straddle a silhouette, so the jitter picks
        // what they hit and all that survives is a range.
        let objects = scene::build(Scene::Spheres, &mut Rng::seed_from_u64(SEED));
        let world = test_world(&objects);
        let camera = axis_camera(8, 1, 5, 8.0 / 16.0);
        let image = camera
            .render(&world, &mut Rng::seed_from_u64(SEED))
            .unwrap();
        let width = image.width;
        assert_eq!((image.width, image.height), (8, 16));

        // Rows 0-2 are sky whatever the draw. Over x in [-7/16, -5/16) and
        // y in (9/16, 15/16] the unit y runs from
        // (9/16) / sqrt(0.19140625 + 0.31640625 + 1) = 0.45808794 to
        // (15/16) / sqrt(0.09765625 + 0.87890625 + 1) = 0.66683134, so a runs
        // [0.72904397, 0.83341567], red in [0.58329217, 0.63547802] and green
        // in [0.74997530, 0.78128681].
        for row in 0..=2 {
            let got = image.pixels[row * width];
            assert_eq!(got[2], 1.0, "row {row} should be sky, got {got:?}");
            assert!(
                got[0] > 0.5832 && got[0] < 0.6356,
                "row {row} red {got:?}, expected in (0.5832, 0.6356)"
            );
            assert!(
                got[1] > 0.7499 && got[1] < 0.7814,
                "row {row} green {got:?}, expected in (0.7499, 0.7814)"
            );
        }

        // Rows 5-9 provably start on the middle ball, whose albedo is
        // (0.1, 0.2, 0.5). Every later surface -- the glass at 1.0, the ground
        // at 0.8, the metal at 0.8 and the sky at no more than 1 -- only
        // multiplies, so the ball's own albedo caps the channel. This is the
        // only surface in the frame with a red albedo below 0.8, which is what
        // makes the cap tight rather than merely true.
        for row in 5..=9 {
            let got = image.pixels[row * width];
            assert!(
                got[0] <= 0.1 && got[1] <= 0.2 && got[2] <= 0.5,
                "row {row} should be the middle ball under its own albedo, got {got:?}"
            );
            assert!(
                got.iter().all(|c| c.is_finite() && *c >= 0.0),
                "row {row} should stay finite and non-negative, got {got:?}"
            );
        }

        // Rows 12-15 provably start on the ground, whose albedo §11 made
        // (0.8, 0.8, 0.0). Blue is 0.0 exactly, for the same reason the sky's
        // is 1.0: the attenuation multiplies through and 0 * anything is 0.
        // Red and green are capped by the ground's own 0.8.
        for row in 12..=15 {
            let got = image.pixels[row * width];
            assert_eq!(
                got[2], 0.0,
                "row {row} should start on the ground, got {got:?}"
            );
            for (channel, value) in got.iter().enumerate() {
                assert!(
                    *value >= 0.0 && *value <= 0.8,
                    "row {row} channel {channel} should be in [0, 0.8], got {value} in {got:?}"
                );
            }
        }

        // Rows 3, 4, 10 and 11 straddle a silhouette, so nothing is pinned.
        for row in [3, 4, 10, 11] {
            let got = image.pixels[row * width];
            assert!(
                got.iter().all(|c| c.is_finite() && *c >= 0.0 && *c <= 1.0),
                "row {row} should stay inside [0, 1], got {got:?}"
            );
        }

        // The order of the three bands is what turns round if the mirror comes
        // back, or if `viewport_upper_left` takes `+ viewport_v / 2`: sky above,
        // ground below, and the two pinned blues far apart.
        let sky = image.pixels[0][2];
        let earth = image.pixels[15 * width][2];
        assert_eq!(sky, 1.0);
        assert_eq!(earth, 0.0);
        assert!(
            sky > earth,
            "sky should sit above ground, got {sky} and {earth}"
        );
    }

    #[test]
    fn zero_aperture_collapses_the_disk_to_a_point() {
        // `defocus_angle = 0` gives `defocus_radius = 10 * tan(0) = 0`, so both
        // disk axes are the zero vector and every sample is `lookfrom`. The sum
        // is exact rather than merely close: `a + (+/-0.0) == a` for every
        // non-zero `a`, and `lookfrom` has no zero components.
        let camera = Camera::chapter_twelve();
        assert_eq!(camera.defocus_angle, 0.0);
        assert_eq!(camera.focus_dist, 10.0);

        let mut rng = Rng::seed_from_u64(1);
        for _ in 0..100 {
            assert_eq!(camera.defocus_disk_sample(&mut rng), camera.lookfrom);
        }
    }

    #[test]
    fn the_defocus_disk_is_the_aperture_sized_by_the_half_angle() {
        // `defocus_radius = focus_dist * tan(defocus_angle / 2)` is the radius at
        // which the cone of admitted rays crosses the plane of focus. Two
        // independent evaluations of it, so a stray `* PI / 180` or a missing
        // halving cannot hide behind the test's own copy of the formula.
        //
        //   focus_dist = 10.0, defocus_angle = 0.6
        //     half-angle = 0.3 deg = 0.3 * PI / 180 = 0.00523598775598299
        //     tan of it                = 0.00523603560570013
        //     radius = 10.0 * that     = 0.052360356057001275
        let lookfrom = Vec3::new(-2.0, 2.0, 1.0);
        let lookat = Vec3::new(0.0, 0.0, -1.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let camera = Camera::new(
            400,
            1,
            1,
            16.0 / 9.0,
            20.0,
            lookfrom,
            lookat,
            vup,
            0.6,
            10.0,
        );

        let expected = 10.0 * 0.3_f64.to_radians().tan();
        close(camera.defocus_disk_u.length() / camera.u.length(), expected);
        assert!(
            (camera.defocus_disk_u.length() - 0.052360356057001275).abs() < 1e-15,
            "defocus_radius was {}",
            camera.defocus_disk_u.length()
        );

        // The §13 listing's own radius, on the other end of the scale:
        //     3.4 * tan(5 deg) = 3.4 * 0.08748866352592401
        //                       = 0.29746145598814161
        let thirteen = Camera::chapter_thirteen();
        assert!(
            (thirteen.defocus_disk_u.length() - 0.29746145598814161).abs() < 1e-15,
            "chapter_thirteen's radius was {}",
            thirteen.defocus_disk_u.length()
        );
    }

    #[test]
    fn the_defocus_disk_spans_the_camera_axes() {
        // `defocus_disk_u = u * radius` and `defocus_disk_v = v * radius`. The
        // two are interchangeable in *length* -- |u| = |v| = 1 -- so any check
        // that only compares magnitudes cannot tell them apart. Swapping them
        // would rotate the lens's sampling pattern by a quarter turn and still
        // pass every radius assertion in this file, so the orientation is
        // pinned against the camera's own public frame instead.
        let camera = Camera::chapter_thirteen();
        let radius = 0.29746145598814161;

        assert_vec_close(camera.defocus_disk_u, camera.u * radius);
        assert_vec_close(camera.defocus_disk_v, camera.v * radius);
        // And the signs: `u` runs right and `v` runs up, so the offsets the
        // disk sample adds are along those and not against them.
        assert!(camera.defocus_disk_u.dot(camera.u) > 0.0);
        assert!(camera.defocus_disk_v.dot(camera.v) > 0.0);
    }

    #[test]
    fn a_pinned_camera_leaves_the_generator_untouched() {
        // The zero-aperture branch in `get_ray` is not cosmetic. With the
        // branch, a sample on a pinhole camera costs exactly the two draws
        // `render` makes for `du` and `dv`, which is what lets `first_draw`
        // index the stream at `2 * samples`. Take the branch out and the disk
        // draw shifts every sample after the first, and the pixel-exact tests
        // above fail on their arithmetic rather than on a value.
        let camera = axis_camera(4, 1, 1, 1.0);
        let mut before = Rng::seed_from_u64(SEED);
        let mut after = Rng::seed_from_u64(SEED);
        before.f64();
        after.f64();
        let _ = camera.get_ray(1, 1, 0.5, 0.5, &mut after);
        assert_eq!(
            before.f64(),
            after.f64(),
            "a pinned camera must not spend a draw on the defocus disk"
        );
    }

    #[test]
    fn an_open_aperture_spends_a_draw_on_the_disk() {
        // The mirror of the test above, and the reason the branch is worth
        // having at all: with an aperture open, two samples through the same
        // pixel leave from different points, so the pixel is an average over a
        // cone rather than a single ray. Reject `get_ray`'s return and compare
        // the origins directly.
        let camera = Camera::chapter_thirteen();
        let mut rng = Rng::seed_from_u64(SEED);
        let first = camera.get_ray(3, 3, 0.5, 0.5, &mut rng);
        let second = camera.get_ray(3, 3, 0.5, 0.5, &mut rng);
        assert_ne!(
            first.origin, second.origin,
            "an open aperture should move the ray origin across the disk"
        );
        // Same pixel, same sample offset, so both rays run to the same point of
        // the viewport -- `Ray::new` leaves the direction unnormalised, so
        // `at(1.0)` is exactly that shared endpoint. Only the lens end moved.
        assert_eq!(first.at(1.0), second.at(1.0));
        // And each origin is within a defocus radius of the eye.
        let radius = camera.defocus_disk_u.length();
        assert!((first.origin - camera.lookfrom).length() <= radius + 1e-9);
    }

    #[test]
    fn a_wide_aperture_spreads_ray_origins_over_the_disk() {
        // The sample is `lookfrom + disk_u * p.x + disk_v * p.y` with `p` drawn
        // from the unit disk, and `u` and `v` are orthonormal, so
        //     |sample - lookfrom| = radius * |p| < radius
        // strictly. `Rng::in_unit_disk` rejects while `p.length_squared() >= 1`,
        // which is what makes the inequality strict; a sampler that allowed the
        // rim would still satisfy the non-strict bound asserted here.
        let camera = Camera::chapter_thirteen();
        let mut rng = Rng::seed_from_u64(1);
        let center = camera.lookfrom;
        let mut max_distance = 0.0f64;
        for _ in 0..1_000 {
            let d = (camera.defocus_disk_sample(&mut rng) - center).length();
            max_distance = max_distance.max(d);
        }
        let radius = (camera.defocus_disk_u.length() + camera.defocus_disk_v.length()) / 2.0;
        assert!(max_distance > 0.0 && max_distance <= radius + 1e-9);
    }

    #[test]
    fn defocus_radius_follows_the_aperture_angle() {
        // Same eye, same target, same 20-degree lens, same focus distance of
        // 3.4; only the opening changes. `radius = 3.4 tan(angle / 2)`:
        //
        //   2 degrees  -> 3.4 tan(1 deg) = 3.4 * 0.01745506493 = 0.05934722
        //  20 degrees  -> 3.4 tan(10 deg) = 3.4 * 0.1763269807 = 0.59951173
        //
        // and `u` is the same unit vector in both, so the comparison on the last
        // line is a comparison of radii.
        let lookfrom = Vec3::new(-2.0, 2.0, 1.0);
        let lookat = Vec3::new(0.0, 0.0, -1.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let small = Camera::new(400, 1, 1, 16.0 / 9.0, 20.0, lookfrom, lookat, vup, 2.0, 3.4);
        let large = Camera::new(
            400,
            1,
            1,
            16.0 / 9.0,
            20.0,
            lookfrom,
            lookat,
            vup,
            20.0,
            3.4,
        );
        assert!(large.defocus_disk_u.length() > small.defocus_disk_u.length());
    }

    #[test]
    fn the_normal_colour_map_sends_plus_y_to_pale_green_and_minus_y_to_magenta() {
        // 0.5 * n + 0.5: +y -> (0, 0.5, 0) + (0.5, 0.5, 0.5) = (0.5, 1.0, 0.5),
        // -y -> (0, -0.5, 0) + (0.5, 0.5, 0.5) = (0.5, 0.0, 0.5).
        assert_vec_close(shade(Vec3::new(0.0, 1.0, 0.0)), Vec3::new(0.5, 1.0, 0.5));
        assert_vec_close(shade(Vec3::new(0.0, -1.0, 0.0)), Vec3::new(0.5, 0.0, 0.5));

        // A unit normal has every component at an endpoint of [-1, 1], so the
        // map sends it to white, and the zero normal to the midpoint grey.
        assert_vec_close(shade(Vec3::UNIT), Vec3::UNIT);
        assert_vec_close(shade(Vec3::ZERO), Vec3::new(0.5, 0.5, 0.5));
    }
}
