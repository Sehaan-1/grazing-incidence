//! The camera and the shading loop.

use crate::error::RenderError;
use crate::hittable::Hittable;
use crate::image::Image;
use crate::ray::Ray;
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

    /// Cast one ray per pixel through the viewport and shade it (§6, §7).
    ///
    /// `image_width` and `image_height` must both be at least 2, because the
    /// loop normalises each axis by `axis - 1`. `renderer::render_scene` owns
    /// that check: `new` returns `Self`, not a `Result`.
    pub fn render(&self, world: &dyn Hittable) -> Result<Image, RenderError> {
        let mut image = Image::new(self.image_width, self.image_height)?;

        for j in 0..self.image_height {
            eprint!("\rScanlines remaining: {} ", self.image_height - j);
            for i in 0..self.image_width {
                let mut color = Vec3::ZERO;
                for _ in 0..self.samples_per_pixel {
                    let u = i as f64 / (self.image_width - 1) as f64;
                    let v = j as f64 / (self.image_height - 1) as f64;
                    let ray = Ray::new(
                        self.origin,
                        self.lower_left_corner + self.horizontal * u + self.vertical * v
                            - self.origin,
                    );
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

    #[test]
    fn render_produces_the_requested_pixel_count() {
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(20, 12, 1, 2);
        let image = camera.render(&world).unwrap();
        assert_eq!(image.pixels.len(), 20 * 12);
        assert_eq!(image.width, 20);
        assert_eq!(image.height, 12);
    }

    #[test]
    fn the_viewport_follows_the_image_aspect_ratio() {
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(20, 12, 1, 5);
        let image = camera.render(&world).unwrap();

        // Row 0 is j = 11, so v = 1, and i = 0 gives u = 0: the top-left pixel
        // is the upper-left viewport corner. `viewport_width` is
        // 2 * (20/12) = 10/3, so that ray is (-10/3 / 2, 1, -1) = (-5/3, 1, -1).
        // Its length is sqrt(25/9 + 1 + 1) = sqrt(43)/3, giving unit.y = 3/sqrt(43).
        // It misses both spheres -- the ball's discriminant is 1 - (43/9)(3/4),
        // which is negative, and the ground's nearest root is negative -- so the
        // pixel is `sky_color`. A square viewport would give unit.y = 1/sqrt(3)
        // instead, so this pins the aspect ratio.
        let sky_a = 0.5 * (1.0 + 3.0 / 43.0_f64.sqrt());
        assert_vec_close(
            Vec3::from_rgb(image.pixels[0]),
            Vec3::new(1.0 - 0.5 * sky_a, 1.0 - 0.3 * sky_a, 1.0),
        );
    }

    #[test]
    fn repeated_samples_average_instead_of_summing() {
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);

        let once = Camera::new(8, 8, 1, 5).render(&world).unwrap();
        let four = Camera::new(8, 8, 4, 5).render(&world).unwrap();

        // Until §8 every sample casts the identical ray, so the mean of four of
        // them is the one-sample colour. Summing instead of dividing by
        // `samples_per_pixel` would make every channel four times as bright.
        assert_vec_close(
            Vec3::from_rgb(four.pixels[0]),
            Vec3::from_rgb(once.pixels[0]),
        );
    }

    #[test]
    fn max_depth_zero_yields_black() {
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(8, 5, 1, 0);
        let image = camera.render(&world).unwrap();
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
        let image = camera.render(&world).unwrap();
        let width = image.width;

        // Row-major, top row first: [0] is the top-left pixel and [7 * width]
        // the bottom-left one, i.e. the two viewport corner rays.
        let top = image.pixels[0];
        let bottom = image.pixels[7 * width];

        // The top row looks up. The corner direction (-1, 1, -1) misses both
        // spheres -- for an upward ray the ground's quadratic has two negative
        // roots, and the ball is behind the eye -- so this is `sky_color`. Its
        // unit y is 1/sqrt(3), giving a = (1 + 1/sqrt(3)) / 2; blue is
        // (1 - a) + a = 1 and red and green give up 0.5a and 0.3a respectively.
        let sky_a = 0.5 * (1.0 + 1.0 / 3.0_f64.sqrt());
        assert_vec_close(
            Vec3::from_rgb(top),
            Vec3::new(1.0 - 0.5 * sky_a, 1.0 - 0.3 * sky_a, 1.0),
        );

        // The bottom row looks down and hits the ground. Direction (-1,-1,-1)
        // against (0,-100.5,-1) r = 100 gives a = 3, half_b = -101.5 and
        // c = 101.25, so t = (101.5 - sqrt(9998.5)) / 3. The outward normal
        // there is (t*(-1,-1,-1) - (0,-100.5,-1)) / 100, and the ray meets it
        // head on (d . outward = -0.999925 < 0), so the hit normal is:
        //   n.x = -t / 100,  n.y = (100.5 - t) / 100,  n.z = (1 - t) / 100
        // `shade` is 0.5 * n + 0.5, so each channel carries its own constant
        // plus the same -t / 200:
        //   0.5 * (-t / 100)       + 0.5 = 0.5     - t / 200
        //   0.5 * ((100.5 - t)/100) + 0.5 = 1.0025 - t / 200
        //   0.5 * ((1 - t) / 100)  + 0.5 = 0.505  - t / 200
        // t = 0.50250009 gives (0.4974875, 0.9999875, 0.5024875).
        let (a, half_b, c): (f64, f64, f64) = (3.0, -101.5, 101.25);
        let t = (-half_b - (half_b * half_b - a * c).sqrt()) / a;
        let k = t / 200.0;
        assert_vec_close(
            Vec3::from_rgb(bottom),
            Vec3::new(0.5 - k, 1.0025 - k, 0.505 - k),
        );

        // Blue is the channel that tells the two apart: the sky pins it at
        // exactly 1.0, while the ground's is 0.5 + n.z / 2 and its normal points
        // nearly straight up, so it lands near 0.5 (0.5024875 here). Drop
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
        // 9 is odd, so the mirror maps the middle row to itself: image row 4 is
        // j = 4, v = 0.5 exactly and the left-edge ray there is exactly
        // horizontal (direction.y == 0). Nothing in the §6 world is hit by it:
        // the ball at (0,0,-1) r = 0.5 needs dx^2 <= 1/3 and the left edge has
        // dx = -1, and the ground at (0,-100.5,-1) r = 100 is 100.5 below the
        // eye. So this pixel is `sky_color` with a = 0.5, giving
        // 0.5 * (1,1,1) + 0.5 * (0.5, 0.7, 1.0) = (0.75, 0.85, 1.0).
        let objects = scene::build(Scene::Spheres);
        let world = test_world(&objects);
        let camera = Camera::new(9, 9, 1, 5);
        let image = camera.render(&world).unwrap();
        let width = image.width;

        assert_vec_close(
            Vec3::from_rgb(image.pixels[4 * width]),
            Vec3::new(0.75, 0.85, 1.0),
        );

        // The ball's silhouette cuts into that horizon row: the centre ray
        // (0, 0, -1) meets it head on at t = 1 - 0.5 = 0.5, at the point
        // (0, 0, -0.5), whose outward normal is (0, 0, 1). The ray meets that
        // head on too (d . outward = -1 < 0), so the hit normal is (0, 0, 1),
        // and `shade` gives 0.5 * (0, 0, 1) + 0.5 * (1, 1, 1) = (0.5, 0.5, 1.0):
        // red and green sit at the midpoint because those normal components are
        // 0, and blue is pinned at 1.0 because n.z = +1. The old geometry had the
        // eye on the ball's surface, so nothing could be drawn here.
        assert_eq!(image.pixels[4 * width + 4], [0.5, 0.5, 1.0]);

        // Along the left edge the horizon lands between rows 4 and 5: every row
        // above it tilts up and misses both spheres, so its blue is the sky's
        // exact 1.0; every row below tilts down into the ground, whose blue is
        // 0.5 + n.z / 2. The ground's normal is nearly (0, 1, 0) everywhere, so
        // that blue sits near 0.5 -- by hand, from the quadratic at each row's
        // dy = 1 - row/4: 0.494429 (row 5), 0.499949, 0.501648, 0.502487
        // (row 8). The gap between 0.502487 and the sky's 1.0 is what makes
        // 0.75 a safe discriminator for sky against ground; the old `shade`'s
        // "under 0.5" no longer holds, since row 8 lands just above it.
        for row in 0..=4 {
            assert_eq!(
                image.pixels[row * width][2],
                1.0,
                "row {row} should be sky, got {:?}",
                image.pixels[row * width]
            );
        }
        for row in 5..=8 {
            let got = image.pixels[row * width];
            assert!(got[2] < 0.75, "row {row} should be ground, got {got:?}");

            // The same pale ground everywhere: green is 0.5 * n.y + 0.5, and
            // n.y never drops below 0.99971, so it never falls under 0.9998.
            assert!(
                got[1] > 0.999,
                "row {row} should be near-vertical ground, got {got:?}"
            );
        }
    }
}
