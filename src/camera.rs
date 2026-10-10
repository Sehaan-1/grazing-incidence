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
                    color += self.ray_color(&ray, self.max_depth, world);
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

    /// Shade one ray: a hit against the sky behind it (§6).
    ///
    /// `depth` counts the bounces still available, and a camera with none left
    /// renders black. Until the material trait lands in §9 a hit is shaded
    /// directly rather than bounced, which is why `depth` is read but not yet
    /// decremented: at §9 a hit returns
    /// `attenuation * self.ray_color(&scattered, depth - 1, world)`.
    fn ray_color(&self, ray: &Ray, depth: usize, world: &dyn Hittable) -> Vec3 {
        if depth == 0 {
            return Vec3::ZERO;
        }

        match world.hit(ray, 0.001, f64::INFINITY) {
            Some(rec) => shade(rec.normal),
            None => sky_color(ray.direction),
        }
    }
}

/// The sky: white at the horizon, blue overhead (§4).
fn sky_color(direction: Vec3) -> Vec3 {
    let unit_direction = direction.unit();
    let a = 0.5 * (unit_direction.y + 1.0);
    Vec3::UNIT * (1.0 - a) + Vec3::new(0.5, 0.7, 1.0) * a
}

/// The book's normal colour map: each normal component maps from [-1, 1] to
/// [0, 1] (§6.1, unchanged through §7).
fn shade(normal: Vec3) -> Vec3 {
    normal * 0.5 + Vec3::UNIT * 0.5
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
    /// one sample can be reproduced without rendering the whole image.
    fn draw(seed: u64, n: usize) -> f64 {
        let mut rng = Rng::seed_from_u64(seed);
        for _ in 0..n {
            rng.f64();
        }
        rng.f64()
    }

    /// Where pixel (i, j)'s first draw begins in that stream.
    fn first_draw(camera: &Camera, i: usize, j: usize) -> usize {
        2 * ((j * camera.image_width + i) * camera.samples_per_pixel)
    }

    /// Sample `k` of pixel (i, j), from the generator state `render` had
    /// reached at that point.
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
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(20, 12, 1, 5);
        let mut rng = Rng::seed_from_u64(SEED);
        let image = camera.render(&world, &mut rng).unwrap();

        // Row 0 is j = 11 and column 0 is i = 0, and one sample per pixel, so
        // pixel [0] is the 221st pixel the loop visits and used draws 440 and
        // 441 of the stream.
        let n = first_draw(&camera, 0, 11);
        let du = draw(SEED, n);
        let dv = draw(SEED, n + 1);

        // The viewport is `viewport_height = 2` tall and
        // `viewport_width = 2 * (20 / 12) = 10/3` wide, anchored one unit in
        // front of the zero eye, so `lower_left_corner` is (-5/3, -1, -1) and
        // the direction is `lower_left_corner + horizontal * u + vertical * v`:
        //   u = (0 + du) / 20   in [0, 1/20), so x = -5/3 + (10/3) u in [-5/3, -3/2)
        //   v = (11 + dv) / 12  in [11/12, 1), so y = -1 + 2 v in [5/6, 1)
        let u = du / 20.0;
        let v = (11.0 + dv) / 12.0;
        let direction = Vec3::new(-5.0 / 3.0 + (10.0 / 3.0) * u, -1.0 + 2.0 * v, -1.0);

        // That ray misses both spheres. The ball's discriminant is
        // 1 - 3a/4 with a = x^2 + y^2 + 1 >= 9/4 + 25/36 + 1 = 71/18, so
        // 1 - (3/4)(71/18) = -47/24 is negative; the ground's quadratic has
        // both roots behind the eye because oc . d = 100.5 * y - 1 >= 82.75
        // with c = 101.25. So the pixel is `sky_color`.
        let ray = Ray::new(Vec3::ZERO, direction);
        assert!(world.hit(&ray, 0.001, f64::INFINITY).is_none());

        // a = 0.5 * (unit.y + 1): blue is (1 - a) + a = 1, red gives up 0.5a
        // and green 0.3a.
        let a = 0.5 * (direction.unit().y + 1.0);
        assert_vec_close(
            Vec3::from_rgb(image.pixels[0]),
            Vec3::new(1.0 - 0.5 * a, 1.0 - 0.3 * a, 1.0),
        );
    }

    #[test]
    fn multiple_samples_average_jittered_rays() {
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);

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
                    color += camera.ray_color(&ray, camera.max_depth, &world);
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
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(8, 8, 1, 5);
        let mut rng = Rng::seed_from_u64(SEED);
        let image = camera.render(&world, &mut rng).unwrap();
        let width = image.width;

        // Row-major, top row first: [0] is the top-left pixel and [7 * width]
        // the bottom-left one, so the viewport rows are j = 7 and j = 0. An
        // 8x8 camera has `viewport_width = viewport_height = 2`, so the
        // viewport spans x, y in [-1, 1) and the direction is
        // (-1 + 2 * (i + du) / 8, -1 + 2 * (j + dv) / 8, -1).
        let corner = |j: usize| -> Vec3 {
            let n = first_draw(&camera, 0, j);
            Vec3::new(
                -1.0 + 2.0 * draw(SEED, n) / 8.0,
                -1.0 + 2.0 * (j as f64 + draw(SEED, n + 1)) / 8.0,
                -1.0,
            )
        };

        // The top row looks up: v = (7 + dv) / 8 puts y in [0.75, 1). The ball
        // needs x^2 + y^2 <= 1/3 and |x| >= 0.75, so it is missed; the ground's
        // quadratic has both roots behind the eye, since
        // oc . d = 100.5 y - 1 >= 74.375 > 0 with c = 101.25. So this pixel is
        // `sky_color`. Its unit y is y / |d|, and blue is (1 - a) + a = 1
        // while red and green give up 0.5a and 0.3a.
        let top = image.pixels[0];
        let top_direction = corner(7);
        assert!(
            world
                .hit(&Ray::new(Vec3::ZERO, top_direction), 0.001, f64::INFINITY)
                .is_none()
        );
        let sky_a = 0.5 * (top_direction.unit().y + 1.0);
        assert_vec_close(
            Vec3::from_rgb(top),
            Vec3::new(1.0 - 0.5 * sky_a, 1.0 - 0.3 * sky_a, 1.0),
        );

        // The bottom row looks down: v = dv / 8 puts y in [-1, -0.75) and it
        // hits the ground. The eye is the origin, so with the ground at
        // (0, -100.5, -1) and r = 100:
        //   oc    = (0, 100.5, 1)
        //   a     = d . d                half_b = oc . d = 100.5 y - 1
        //   c     = oc . oc - 100^2     = 101.25
        //   t     = (-half_b - sqrt(half_b^2 - a c)) / a
        // and the outward normal is (t * d - (0, -100.5, -1)) / 100. `shade`
        // is 0.5 * n + 0.5, so each channel is 0.5 * n + 0.5.
        let bottom = image.pixels[7 * width];
        let bottom_direction = corner(0);
        let center = Vec3::new(0.0, -100.5, -1.0);
        let oc = Vec3::ZERO - center;
        let a = bottom_direction.dot(bottom_direction);
        let half_b = oc.dot(bottom_direction);
        let c = oc.dot(oc) - 100.0 * 100.0;
        let t = (-half_b - (half_b * half_b - a * c).sqrt()) / a;
        let outward = (bottom_direction * t - center) / 100.0;
        // The ray meets the near side of the ground, so the outward normal is
        // the hit normal: d . outward = (t (dx^2 + dy^2) - (1 - t)) / 100, and
        // t (dx^2 + dy^2) is far below 1 - t for a near hit.
        assert!(
            bottom_direction.dot(outward) < 0.0,
            "the eye should meet the ground head on"
        );
        assert_vec_close(Vec3::from_rgb(bottom), shade(outward));

        // Blue is the channel that tells the two apart: the sky pins it at
        // exactly 1.0, while the ground's is 0.5 + n.z / 2 and its normal
        // points nearly straight up, so it lands near 0.5. Drop
        // `let row = image_height - 1 - j;` and these two rows swap, so this
        // comparison is what turns round.
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
        // units below it. With the eye at the origin and the ball at
        // (0, 0, -1) r = 0.5: oc = (0, 0, 1), half_b = oc . d = -1 and
        // c = oc . oc - 1/4 = 3/4, so the near root is
        //   t = (1 - sqrt(1 - 3a / 4)) / a,  a = d . d
        // and the outward normal, (t d - (0, 0, -1)) / 0.5, is
        //   (2 t x, 2 t y, 2 (1 - t)).
        // The ray meets it head on, since d . outward = 2 (t a - 1) = -2 sqrt(1
        // - 3a / 4), which is negative.
        let centre = sample_ray(&camera, SEED, 4, 4, 0).direction;
        let a = centre.dot(centre);
        let t = (1.0 - (1.0 - 0.75 * a).sqrt()) / a;
        assert_vec_close(
            Vec3::from_rgb(image.pixels[4 * width + 4]),
            shade(Vec3::new(
                2.0 * t * centre.x,
                2.0 * t * centre.y,
                2.0 * (1.0 - t),
            )),
        );

        // Rows 0 to 4 are all sky, whatever the draw. Along the left edge
        // u = du / 9 puts x in [-1, -7/9), so the ball's x^2 + y^2 <= 1/3 is
        // out of reach, and the ground -- hit only when its smaller root is
        // ahead of the eye, i.e. when y <= the silhouette -- tops out at
        // y = -0.1174 here, below row 4's y in [-1/9, 1/9). Every sky pixel
        // has blue at exactly 1.0, because `sky_color` returns (1 - a) + a.
        for row in 0..=4 {
            assert_eq!(
                image.pixels[row * width][2],
                1.0,
                "row {row} should be sky, got {:?}",
                image.pixels[row * width]
            );
        }

        // Rows 6 to 8 are all ground: their y runs from -1/3 down to -1, clear
        // of the silhouette, and they miss the ball on x. The ground's normal
        // is nearly (0, 1, 0), so blue is 0.5 + n.z / 2 and green is 0.5 * n.y
        // + 0.5, with n.y never dropping below 0.99994 -- the gap between that
        // green and the sky's 0.85 is what keeps the two apart.
        for row in 6..=8 {
            let got = image.pixels[row * width];
            assert!(got[2] < 0.75, "row {row} should be ground, got {got:?}");
            assert!(
                got[1] > 0.999,
                "row {row} should be near-vertical ground, got {got:?}"
            );
        }

        // Row 5 is the one row the jitter can push either side of the
        // silhouette: j = 3 puts v anywhere in [1/3, 4/9), so y runs from -1/3
        // up to -1/9, and that band straddles the ground's edge between
        // -0.1323 at x = -1 and -0.1174 at x = -7/9. So it is checked against
        // its own draw. The ground quadratic is the same one as in
        // `sky_is_above_and_ground_below`; the ball is missed on x.
        let j = 3;
        let n = first_draw(&camera, 0, j);
        let direction = Vec3::new(
            -1.0 + 2.0 * draw(SEED, n) / 9.0,
            -1.0 + 2.0 * (j as f64 + draw(SEED, n + 1)) / 9.0,
            -1.0,
        );
        let center = Vec3::new(0.0, -100.5, -1.0);
        let oc = Vec3::ZERO - center;
        let a = direction.dot(direction);
        let half_b = oc.dot(direction);
        let c = oc.dot(oc) - 100.0 * 100.0;
        let discriminant = half_b * half_b - a * c;
        let expected = if discriminant < 0.0 {
            sky_color(direction)
        } else {
            let t = (-half_b - discriminant.sqrt()) / a;
            shade((direction * t - center) / 100.0)
        };
        assert_vec_close(Vec3::from_rgb(image.pixels[5 * width]), expected);
    }
}
