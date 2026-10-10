//! The camera and the shading loop.

use crate::error::RenderError;
use crate::hittable::Hittable;
use crate::image::Image;
use crate::ray::Ray;
use crate::rng::Rng;
use crate::vec3::Vec3;

/// A pinhole camera (§7). Position and orientation become settable in §12.
pub struct Camera {
    pub image_width: usize,
    pub image_height: usize,
    pub samples_per_pixel: usize,
    pub max_depth: usize,

    origin: Vec3,
    horizontal: Vec3,
    vertical: Vec3,
    lower_left_corner: Vec3,
}

impl Camera {
    pub fn new(
        image_width: usize,
        image_height: usize,
        samples_per_pixel: usize,
        max_depth: usize,
    ) -> Self {
        let viewport_height = 2.0;
        let viewport_width = viewport_height * (image_width as f64 / image_height as f64);
        let origin = Vec3::ZERO;
        let horizontal = Vec3::new(viewport_width, 0.0, 0.0);
        let vertical = Vec3::new(0.0, viewport_height, 0.0);
        let lower_left_corner =
            origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new(0.0, 0.0, 1.0);

        Self {
            image_width,
            image_height,
            samples_per_pixel,
            max_depth,
            origin,
            horizontal,
            vertical,
            lower_left_corner,
        }
    }

    /// Cast one ray per sample through the viewport and shade it (§6, §7, §8).
    ///
    /// Every sample is a fresh jittered ray from `rng`, so the same generator
    /// state replays the same image and the test suite can assert on exact
    /// pixel values. `renderer::render_scene` owns the dimension check;
    /// `new` returns `Self`, not a `Result`.
    pub fn render(&self, world: &dyn Hittable, rng: &mut Rng) -> Result<Image, RenderError> {
        let mut image = Image::new(self.image_width, self.image_height)?;

        for j in 0..self.image_height {
            eprint!("\rScanlines remaining: {} ", self.image_height - j);
            for i in 0..self.image_width {
                let mut color = Vec3::ZERO;
                for _ in 0..self.samples_per_pixel {
                    let ray = self.get_ray(i, j, rng);
                    color += self.ray_color(&ray, self.max_depth, world, rng);
                }
                // The book's §4-§7 camera anchors on the bottom-left viewport
                // corner and counts j upward, so j = 0 lands in the image's top
                // row. Mirror the write index to keep every image right-side-up.
                // §12 re-anchors on `pixel00_loc`, at which point this comes out.
                let row = self.image_height - 1 - j;
                image.set(i, row, (color / self.samples_per_pixel as f64).to_rgb());
            }
        }
        eprintln!("\rDone.                 ");

        Ok(image)
    }

    /// A camera ray for pixel (i, j), jittered within the pixel (§8).
    ///
    /// The sample lands at `(i + du) / image_width` with `du` uniform on
    /// `[0, 1)`, so `i + du` runs over `[i, i + 1)` and `width` samples cover
    /// the viewport edge to edge. Normalising by `width - 1` instead would push
    /// the last row and column past the far edge -- pixel 7 of an 8-wide camera
    /// would reach `u = (7 + du) / 7`, which is at least 1 -- and would leave
    /// the first row and column a full sample short of the near edge. The
    /// book makes the same change.
    fn get_ray(&self, i: usize, j: usize, rng: &mut Rng) -> Ray {
        let du = rng.range(0.0, 1.0);
        let dv = rng.range(0.0, 1.0);
        let u = (i as f64 + du) / self.image_width as f64;
        let v = (j as f64 + dv) / self.image_height as f64;

        Ray::new(
            self.origin,
            self.lower_left_corner + self.horizontal * u + self.vertical * v - self.origin,
        )
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
    /// exactly the two draws `get_ray` makes.
    ///
    /// This is what keeps the pixel-exact tests below exact. §9 gave the
    /// generator a second job: a scattered ray draws from the same `Rng` that
    /// places the samples, and how many draws that costs depends on the path
    /// the ray takes. Against the §6 scene, pixel (i, j)'s `du` is no longer at
    /// a fixed offset into the stream, because every pixel ahead of it may have
    /// spent a different number of draws. Against an empty world the
    /// arithmetic in `draw` and `first_draw` holds exactly.
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
    /// scatter.
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
    /// caller pairs this with `empty_world`.
    fn first_draw(camera: &Camera, i: usize, j: usize) -> usize {
        2 * ((j * camera.image_width + i) * camera.samples_per_pixel)
    }

    /// Sample `k` of pixel (i, j), from the generator state `render` had
    /// reached at that point. Only exact against `empty_world`, for the reason
    /// `first_draw` gives.
    fn sample_ray(camera: &Camera, seed: u64, i: usize, j: usize, k: usize) -> Ray {
        let mut rng = Rng::seed_from_u64(seed);
        for _ in 0..first_draw(camera, i, j) + 2 * k {
            rng.f64();
        }
        camera.get_ray(i, j, &mut rng)
    }

    #[test]
    fn render_produces_the_requested_pixel_count() {
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(20, 12, 1, 2);
        let mut rng = Rng::seed_from_u64(SEED);
        let image = camera.render(&world, &mut rng).unwrap();
        assert_eq!(image.pixels.len(), 20 * 12);
        assert_eq!(image.width, 20);
        assert_eq!(image.height, 12);
    }

    #[test]
    fn jittered_rays_land_inside_their_own_pixel() {
        // A sample sits at ((i + du) / width, (j + dv) / height) with du, dv
        // uniform on [0, 1), so u is in [i / width, (i + 1) / width): it never
        // reaches 1, and the far row and column stop just short of the far
        // edge. On an 8x8 camera `viewport_width` is 2 * (8 / 8) = 2 and
        // `viewport_height` is 2, so the viewport spans x and y in [-1, 1) and
        // the eye is the zero vector, which makes the direction itself the
        // viewport point.
        //
        // Normalising by `width - 1` instead would break this: pixel 7 would
        // sit at u = (7 + du) / 7, which is at least 1.
        let camera = Camera::new(8, 8, 64, 5);
        let mut rng = Rng::seed_from_u64(SEED);
        for j in 0..camera.image_height {
            for i in 0..camera.image_width {
                for _ in 0..camera.samples_per_pixel {
                    let dir = camera.get_ray(i, j, &mut rng).direction;
                    assert!(
                        dir.x >= -1.0 && dir.x < 1.0 && dir.y >= -1.0 && dir.y < 1.0,
                        "sample for pixel ({i}, {j}) left the viewport: {dir:?}"
                    );
                    close(dir.z, -1.0);
                }
            }
        }
    }

    #[test]
    fn the_viewport_follows_the_image_aspect_ratio() {
        // Pixel (0, 0) is the very first one `render` draws, so its `du` and
        // `dv` are the first two values the seed yields no matter what world is
        // in play -- nothing runs ahead of it. Against `empty_world` the pixel
        // is background, which is what lets the closed form below stand in for
        // the shading.
        let world = empty_world();
        let camera = Camera::new(20, 12, 1, 5);
        let mut rng = Rng::seed_from_u64(SEED);
        let image = camera.render(&world, &mut rng).unwrap();

        // One sample per pixel and (0, 0) is pixel 0 of the loop, so its draws
        // are numbers 0 and 1 of the stream.
        let n = first_draw(&camera, 0, 0);
        assert_eq!(n, 0);
        let du = draw(SEED, n);
        let dv = draw(SEED, n + 1);

        // The viewport is `viewport_height = 2` tall and
        // `viewport_width = 2 * (20 / 12) = 10/3` wide, anchored one unit in
        // front of the zero eye, so `lower_left_corner` is (-5/3, -1, -1) and
        // the direction is `lower_left_corner + horizontal * u + vertical * v`:
        //   u = (0 + du) / 20   in [0, 1/20), so x = -5/3 + (10/3) u in [-5/3, -3/2)
        //   v = (0 + dv) / 12   in [0, 1/12),  so y = -1 + 2 v    in [-1, -5/6)
        let u = du / 20.0;
        let v = dv / 12.0;
        let direction = Vec3::new(-5.0 / 3.0 + (10.0 / 3.0) * u, -1.0 + 2.0 * v, -1.0);
        assert_vec_close(
            camera
                .get_ray(0, 0, &mut Rng::seed_from_u64(SEED))
                .direction,
            direction,
        );

        // a = 0.5 * (unit.y + 1): blue is (1 - a) + a = 1, red gives up 0.5a
        // and green 0.3a.
        // `j = 0` is the viewport's bottom row and the mirror puts it at the
        // image's last row, so the pixel lands at index 11 * 20.
        let row = camera.image_height - 1;
        assert_vec_close(
            Vec3::from_rgb(image.pixels[row * image.width]),
            sky_color(direction),
        );
    }

    #[test]
    fn multiple_samples_average_jittered_rays() {
        // An empty world, so each sample costs exactly the two draws
        // `get_ray` makes and `first_draw` still locates every sample.
        let world = empty_world();

        let mut rng = Rng::seed_from_u64(SEED);
        let once = Camera::new(8, 8, 1, 5).render(&world, &mut rng).unwrap();
        let mut rng = Rng::seed_from_u64(SEED);
        let four = Camera::new(8, 8, 4, 5).render(&world, &mut rng).unwrap();

        // One pixel's worth of samples, taken from the same stream `render`
        // walked: sample k of pixel (i, j) starts 2 * k draws after the pixel's
        // own first draw. Every pixel must come out as the mean of its
        // `samples_per_pixel` shaded samples -- summing instead of dividing
        // would be four times as bright and fail every row below.
        let camera = Camera::new(8, 8, 4, 5);
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
                let row = camera.image_height - 1 - j;
                assert_vec_close(
                    Vec3::from_rgb(four.pixels[row * four.width + i]),
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
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);

        let mut rng = Rng::seed_from_u64(SEED);
        let once = Camera::new(8, 8, 1, 5).render(&world, &mut rng).unwrap();
        let mut rng = Rng::seed_from_u64(SEED);
        let four = Camera::new(8, 8, 4, 5).render(&world, &mut rng).unwrap();

        assert_ne!(once.pixels, four.pixels);
    }

    #[test]
    fn max_depth_zero_yields_black() {
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(8, 5, 1, 0);
        let mut rng = Rng::seed_from_u64(SEED);
        let image = camera.render(&world, &mut rng).unwrap();
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
        // An 8x8 camera has `viewport_width = viewport_height = 2`, so the
        // viewport spans x, y in [-1, 1) and the direction is
        // (-1 + 2 * (i + du) / 8, -1 + 2 * (j + dv) / 8, -1). Column 0 puts
        // x in [-1, -3/4) and the mirrored rows put j = 7 at image row 0 and
        // j = 0 at image row 7.
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(8, 8, 1, 5);
        let mut rng = Rng::seed_from_u64(SEED);
        let image = camera.render(&world, &mut rng).unwrap();
        let width = image.width;

        // Row 0 looks up: v = (7 + dv) / 8 puts y in [3/4, 1). All four spheres
        // miss over that whole rectangle, whatever the draw.
        //
        //   ground  half_b = 100.5 y - 1 >= 74.375 and c = 101.25 > 0, so
        //           sqrtd < half_b and both roots in `Sphere::hit` are negative.
        //   glass   the discriminant is -0.75 x^2 - 2 x - 1.75 y^2 - 0.75, and
        //           over x in [-1, -3/4), y in [3/4, 1) its maximum is at
        //           (x, y) = (-1, 3/4): 4 - 4.484375 = -0.484375.
        //   blue    a = x^2 + y^2 + 1 >= 2.125 gives 1 - 0.75 a <= -0.59375.
        //   metal   half_b = -x - 1 is at most 1/4, so half_b^2 <= 0.0625
        //           against 1.75 a >= 3.71875.
        //
        // The sky pins blue at exactly 1.0, because `sky_color` returns
        // (1 - a) + a. Its unit y is y / |d| with y in [3/4, 1) and x^2 in
        // (9/16, 1], so it runs from 0.75 / sqrt(2.5625) = 0.46852129 to
        // 1 / sqrt(2.5625) = 0.62469505 and a = 0.5 (unit.y + 1) over
        // [0.73426064, 0.81234752]. Red gives up 0.5a and green 0.3a, so red is
        // in [0.59382624, 0.63286968] and green in [0.75629574, 0.77972181].
        // The observed pair for seed 1 is (0.60703562, 0.76422137).
        let top = image.pixels[0];
        assert_eq!(top[2], 1.0, "sky blue is exactly 1, got {top:?}");
        assert!(
            top[0] > 0.5935 && top[0] < 0.6332,
            "sky red {top:?}, expected in (0.5935, 0.6332)"
        );
        assert!(
            top[1] > 0.7560 && top[1] < 0.7800,
            "sky green {top:?}, expected in (0.7560, 0.7800)"
        );

        // Row 7 looks down: v = dv / 8 puts y in [-1, -3/4), and the same four
        // checks leave the ground as the only thing there to hit. The glass's
        // discriminant over x in [-1, -3/4), y in [-1, -3/4) still tops out at
        // -0.484375, the blue and metal balls are out on x as above, and the
        // ground's `t` rationalises to 101.25 / (|half_b| + sqrtd), which stays
        // inside [0.495, 0.672] -- comfortably past `t_min` and the only hit.
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

        // Drop `let row = image_height - 1 - j;` and these two rows swap: the
        // sky above is pinned to blue 1.0 and the ground below to blue 0.0, so
        // this is what turns round.
        assert!(
            top[2] > bottom[2],
            "sky should sit above ground, got top {top:?} and bottom {bottom:?}"
        );
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

    #[test]
    fn the_horizon_falls_on_the_vertical_centre_row() {
        // A 9x9 camera has `viewport_width = viewport_height = 2`, so the
        // viewport spans x, y in [-1, 1) and the direction for pixel (i, j) is
        // (-1 + 2 (i + du) / 9, -1 + 2 (j + dv) / 9, -1). Image row `r` comes
        // from viewport row j = 8 - r, so j = 4 -- the viewport centre -- is
        // image row 4: the two run the same way round here only because the
        // height is odd and the mirror is an involution on the middle row.
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(9, 9, 1, 5);
        let mut rng = Rng::seed_from_u64(SEED);
        let image = camera.render(&world, &mut rng).unwrap();
        let width = image.width;

        // Pixel (4, 4) is the centre one, and §11's glass ball sits at
        // column -1 so it is nowhere near it. u = (4 + du) / 9 and
        // v = (4 + dv) / 9 put the sample in [-1/9, 1/9) on both axes, and over
        // that box the middle ball is the only thing in range:
        //   middle  a = x^2 + y^2 + 1 <= 1 + 2/81 and half_b = -1, so
        //           1 - 0.75 a >= 0.23148 > 0, with t in (0.4879, 0.5189].
        //   glass   the left ball's discriminant tops out at -0.34256 at
        //           (-1/9, -1/9) and the right ball's at
        //           (11/9)^2 - 1.75 = -0.51543 at x = -1/9; both negative.
        //   ground  can be hit, but at t >= 5.3 -- the ground's `t`
        //           rationalises to 101.25 / (|half_b| + sqrtd), and the widest
        //           |half_b| in the box is 12.16667 at y = -1/9.
        // So the first material is the matte blue ball, albedo (0.1, 0.2, 0.5).
        //
        // That fixes a cap no other pixel in this column can reach: the ball is
        // the only surface here with a red albedo below 0.8, and everything
        // downstream of it -- glass at 1.0, the ground at 0.8, the metal at 0.8
        // and the sky at no more than 1.0 -- only multiplies. Seed 1's pixel is
        // (0.05500573, 0.13000688, 0.0), a glass bounce, then the yellow ground
        // and the sky; the ground is why blue is exactly 0.
        //
        // The colour ordering is measured, not derived: the ball's own albedo
        // has blue five times red, but a chain of four metal bounces reaches
        // (4/3)^4 = 3.16 on red over green, so no seed-free claim survives.
        // What is derived is the cap.
        let ball = image.pixels[4 * width + 4];
        assert!(
            ball[0] <= 0.1 && ball[1] <= 0.2 && ball[2] <= 0.5,
            "the centre pixel should be the matte ball under its own albedo, got {ball:?}"
        );
        assert!(
            ball.iter().all(|c| c.is_finite()) && ball.iter().all(|c| *c >= 0.0),
            "the centre pixel should stay finite and non-negative, got {ball:?}"
        );

        // Rows 0 and 1, column 0, are sky whatever the draw. Both rectangles sit
        // at y >= 5/9, and over x in [-1, -7/9) the glass ball's discriminant
        // stays below zero above |y| = 0.53452248 (its widest limit, at x = -1,
        // falling to 0.44839514 at x = -7/9). 5/9 is 0.555556, so the glass
        // misses; the blue and metal balls are out on x, where x^2 >= 49/81;
        // and the ground's half_b = 100.5 y - 1 stays positive, so its roots are
        // both behind the eye.
        //
        // Across both rows the unit y runs from 5/9 / sqrt(2.308642) =
        // 0.36563621 to 1 / sqrt(2.604938) = 0.61958555, giving a over
        // [0.68281811, 0.80979278], red in [0.59510361, 0.65859095] and green
        // in [0.75706217, 0.79515457]. Seed 1 gives (0.61003261, 0.76601957)
        // and (0.63765310, 0.78259186).
        for row in 0..=1 {
            let got = image.pixels[row * width];
            assert_eq!(got[2], 1.0, "row {row} should be sky, got {got:?}");
            assert!(
                got[0] > 0.5948 && got[0] < 0.6589,
                "row {row} red {got:?}, expected in (0.5948, 0.6589)"
            );
            assert!(
                got[1] > 0.7567 && got[1] < 0.7955,
                "row {row} green {got:?}, expected in (0.7567, 0.7955)"
            );
        }

        // Rows 7 and 8, column 0, are ground whatever the draw, by the same
        // silhouette limit from below: both sit at |y| >= 5/9 = 0.555556, past
        // the glass ball's 0.53452248, so the only surface left is the ground,
        // at t inside [0.495, 0.672].
        //
        // §11 made that ground (0.8, 0.8, 0.0), and a ground-first pixel is
        // pinned exactly: blue is 0.0 for the same reason the sky's is 1.0,
        // because the attenuation multiplies through and 0 * anything is 0.
        // Red and green are at most 0.8, the ground's own albedo, since every
        // later albedo and the sky itself is at most 1.
        for row in 7..=8 {
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

        // Rows 2 to 6 are the glass ball's silhouette, and unlike the rows on
        // either side of them nothing here is pinned. Rows 3, 4 and 5 have
        // |y| <= 1/3 = 0.33333333, inside even the narrowest silhouette limit,
        // so those three provably start on the glass; rows 2 and 6 run from
        // |y| = 1/3 out to 5/9 and straddle the 0.44839514 to 0.53452248 edge,
        // so the jitter decides. And glass neither tints nor absorbs -- its
        // attenuation is (1, 1, 1) and it has no `None` arm -- so the pixel is
        // just whatever the rest of the chain returns, which is why seed 1
        // gives blue 0 on row 3 (the chain reaches the ground) and blue 1 on
        // rows 4 and 5 (it does not). All that survives is the range.
        for row in 2..=6 {
            let got = image.pixels[row * width];
            assert!(
                got.iter().all(|c| c.is_finite() && *c >= 0.0 && *c <= 1.0),
                "row {row} should stay inside [0, 1], got {got:?}"
            );
        }

        // The horizon is where the two provable blocks meet: sky pinned to blue
        // 1.0 on rows 0 and 1, ground pinned to blue 0.0 on rows 7 and 8, and
        // the silhouette between them. Drop `let row = image_height - 1 - j;`
        // and the two blocks swap ends, so the ordering below is what catches it.
        let sky = image.pixels[0][2];
        let earth = image.pixels[8 * width][2];
        assert_eq!(sky, 1.0);
        assert_eq!(earth, 0.0);
        assert!(
            sky > earth,
            "sky should sit above ground, got {sky} and {earth}"
        );
    }
}
