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

        // Row 0 looks up: v = (7 + dv) / 8 puts y in [3/4, 1). The ball needs
        // x^2 + y^2 <= 1/3 and x^2 >= 9/16 already, so it is missed; the
        // ground's quadratic has both roots behind the eye, because
        // oc . d = 100.5 y - 1 >= 74.375 > 0 with c = 101.25. So this pixel is
        // `sky_color`, whatever the draw -- which is the point, because §9
        // moved the draw index and the exact value is no longer predictable.
        //
        // Its unit y is y / |d| with y in [3/4, 1) and x^2 in [9/16, 1), so
        // |d| is in [sqrt(17/8), sqrt(3)). The smallest unit y is
        // 0.75 / sqrt(3) = 0.4330127 and the largest is 1 / sqrt(17/8) =
        // 0.6859943, so a = 0.5 * (unit.y + 1) is in [0.7165064, 0.8429972).
        // Blue is (1 - a) + a = 1, red gives up 0.5a and green 0.3a, so red is
        // in [0.5785014, 0.6417468] and green in [0.7471008, 0.7850481]. The
        // observed pair for seed 1 is (0.6268816, 0.7761290), comfortably
        // inside, and the interval holds for every draw.
        let top = image.pixels[0];
        assert_eq!(top[2], 1.0, "sky blue is exactly 1, got {top:?}");
        assert!(top[0] >= 0.5785 && top[0] <= 0.6418, "sky red {top:?}");
        assert!(top[1] >= 0.7471 && top[1] <= 0.7851, "sky green {top:?}");

        // Row 7 looks down: v = dv / 8 puts y in [-1, -3/4) and it hits the
        // ground, again whatever the draw -- x^2 >= 9/16 misses the ball, and
        // oc . d = 100.5 y - 1 <= -76.375 < 0 with c = 101.25 > 0 puts a root
        // ahead of the eye.
        //
        // The ground's albedo is (0.5, 0.5, 0.5), and it is the first thing this
        // ray meets, so the pixel is the rest of the bounce chain scaled by 0.5.
        // Every later albedo in the scene tops out at the metal ball's 0.9 and
        // the sky tops out at 1.0, so the tightest channel is red at
        // 0.5 * 0.9 = 0.45 -- the sky's 1.0 is out of reach and 0.5 is the
        // ground's own albedo, which no chain can exceed. That is where the old
        // "no albedo in the scene exceeds 0.7" argument stopped holding: §10
        // raised the largest albedo to 0.9, and only the ground-first ordering
        // keeps the bound true.
        //
        // The lower bound is `>= 0.0`, not `> 0.0`. §9's Lambertian always
        // scattered, so a ground pixel could not be black; §10's metal absorbs,
        // and the second hop out of the ground can now reach the ball. Zero is
        // a legitimate value here and the 0.5 cap above is what separates
        // ground from sky -- `sky_color` pins a sky pixel's blue to exactly 1.0.
        let bottom = image.pixels[7 * width];
        for (channel, value) in bottom.iter().enumerate() {
            assert!(
                *value >= 0.0 && *value <= 0.5,
                "ground channel {channel} should be in [0, 0.5], got {value} in {bottom:?}"
            );
        }

        // Blue is the channel that tells the two apart: the sky pins it at
        // exactly 1.0, while every ground channel is capped at the ground
        // albedo of 0.5. Drop `let row = image_height - 1 - j;` and these two
        // rows swap, so this comparison is what turns round.
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

        // The ball's silhouette cuts into the horizon row. Pixel (4, 4) is the
        // centre one: u = (4 + du) / 9 and v = (4 + dv) / 9 put the sample in
        // [-1/9, 1/9) on both axes, so x^2 + y^2 <= 2/81, well inside the
        // 1/3 the ball subtends, and it wins the list over the ground 100
        // units below it.
        //
        // Every pixel is a chain of albedos closing on the sky, and the sky
        // terminates at (1 - 0.5a, 1 - 0.3a, 1) for a = 0.5 * (unit.y + 1) in
        // [0, 1], so the terminal ratio is (5/7) <= r/g <= 1 and the terminal
        // g / b = 1 - 0.3a is in [0.7, 1]. Multiplying those by the per-bounce
        // ratios pins the channels:
        //   red / green   = (9/4)^m * (terminal r / terminal g), m metal bounces
        //   green / blue  = 4^m * (terminal g / terminal b)
        // The ground's (0.5, 0.5, 0.5) contributes 1 to both. §9's Lambertian
        // ball contributed 7/3 and 1, so green never led blue; §10's metal
        // contributes 9/4 and 4, which reverses the blue comparison. So:
        //   m = 0 gives r <= g and g <= b -- a sky or ground pixel.
        //   m >= 1 gives r/g >= (9/4)(5/7) = 45/28 and g/b >= 4 * 0.7 = 2.8,
        //   so r strictly leads g and g strictly leads b -- the metal ball.
        // For seed 1 this pixel is (0.23117027, 0.14164541, 0.05000000), a
        // metal bounce followed by a ground bounce and then the sky: blue is
        // 0.1 * 0.5 * 1 = 0.05 exactly, and r/g = 1.632 against the 1.607
        // floor and g/b = 2.833 against the 2.8 floor.
        let ball = image.pixels[4 * width + 4];
        assert!(
            ball[0] > ball[1] && ball[1] > ball[2],
            "the centre pixel should be the metal ball, got {ball:?}"
        );

        // Rows 0 to 4 are all sky, whatever the draw. Along the left edge
        // u = du / 9 puts x in [-1, -7/9), so the ball's x^2 + y^2 <= 1/3 is
        // out of reach, and the ground's quadratic has both roots behind the
        // eye unless 100.5 y - 1 < 0; rows 0 to 4 have y in [-1/9, 1), and at
        // the widest the discriminant 9999 y^2 - 201 y - 100.25 - 101.25 x^2
        // peaks at y = -1/9 with -15.72, so it is negative throughout. Every
        // sky pixel has blue at exactly 1.0, because `sky_color` returns
        // (1 - a) + a.
        for row in 0..=4 {
            let got = image.pixels[row * width];
            assert_eq!(got[2], 1.0, "row {row} should be sky, got {got:?}");
            assert!(
                got[0] <= got[1] && got[1] <= got[2],
                "sky is blue-leading, got {got:?}"
            );
        }

        // Rows 6 to 8 are all ground: their y runs from -1/3 down to -1, clear
        // of the silhouette, and they miss the ball on x. The ground's albedo
        // is a neutral 0.5 and it is the first thing each of these rays meets,
        // so no channel can exceed 0.5. Red is the tight one -- 0.5 * 0.9, the
        // metal ball's red -- and even that stops at 0.45. Under §8's `shade`
        // these pixels had green above 0.999, so the cap is the assertion that
        // fails if the material is dropped.
        //
        // The second hop is no longer pinned down: it can reach the metal ball
        // now, and if the ball absorbs it the pixel goes black. Seed 1 sends
        // row 7 there -- its blue is 0.05, which is the ball's 0.1 times the
        // ground's 0.5 -- so `>= 0.0` is the honest floor here, not `> 0.0`.
        for row in 6..=8 {
            let got = image.pixels[row * width];
            for (channel, value) in got.iter().enumerate() {
                assert!(
                    *value >= 0.0 && *value <= 0.5,
                    "row {row} channel {channel} should be in [0, 0.5], got {value} in {got:?}"
                );
            }
            // Blue never outruns green by more than 3/10, and that holds for
            // every chain. The sky terminates at a green / blue of
            // 1 - 0.3a in [0.7, 1], a ground bounce contributes a ratio of 1,
            // and §10's metal contributes 0.4 / 0.1 = 4. So a chain that never
            // reaches the ball keeps green / blue in [0.7, 1] and one that does
            // lands in [4 * 0.7, 4] = [2.8, 4] -- the first is bounded below by
            // 0.7 and the second by 2.8, so 0.7 is the floor over both.
            //
            // Seed 1 gives 0.7114 on row 6, 3.3249 on row 7 and 0.7233 on row
            // 8: rows 6 and 8 never touch the ball, row 7 does. Under §9 the
            // ball's green / blue was 1, so green never led blue for any pixel
            // at all; §10's metal inverts that for every chain that reaches it,
            // which is why the surviving statement is a floor and not an
            // ordering.
            assert!(
                got[1] >= 0.7 * got[2],
                "green should be at least 0.7 of blue, got {got:?}"
            );
        }

        // Row 5 is the one row the jitter can push either side of the
        // silhouette: j = 3 puts v anywhere in [1/3, 4/9), so y runs from -1/3
        // up to -1/9, and that band straddles the ground's edge between
        // -0.1323 at x = -1 and -0.1174 at x = -7/9. So it is either sky, which
        // pins blue at exactly 1.0, or ground, which caps it at 0.5. That is
        // all this row can say on its own; the row mirror is pinned by rows 0
        // to 4 pinning blue to 1.0 above and rows 6 to 8 capping it at 0.5
        // below, which a dropped mirror would invert.
        let middle = image.pixels[5 * width];
        assert!(
            middle[2] == 1.0 || middle[2] <= 0.5,
            "row 5 is sky or ground, got {middle:?}"
        );
        assert!(
            middle.iter().all(|c| c.is_finite()),
            "row 5 should stay finite, got {middle:?}"
        );
    }
}
