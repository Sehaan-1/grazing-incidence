//! The pixel loop.

use crate::error::RenderError;
use crate::hittable::{Hittable, HittableList};
use crate::image::Image;
use crate::ray::Ray;
use crate::scene::{self, Scene};
use crate::vec3::Vec3;

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

/// The sky: white at the horizon, blue overhead (§4).
fn sky_color(direction: Vec3) -> Vec3 {
    let unit_direction = direction.unit();
    let a = 0.5 * (unit_direction.y + 1.0);
    Vec3::UNIT * (1.0 - a) + Vec3::new(0.5, 0.7, 1.0) * a
}

/// Shade a hit by how much it faces the camera (§6.1).
fn shade(normal: Vec3) -> Vec3 {
    Vec3::new(0.2, 0.3, 0.5) * (0.5 * (normal.dot(Vec3::new(0.0, 0.0, -1.0)) + 1.0))
}

/// Cast one ray per pixel and shade each hit against the sky (§6).
///
/// `width` and `height` must both be at least 2: the loop normalises each axis by
/// `axis - 1`, so a 1-pixel axis divides by zero and every channel becomes NaN.
/// NaN survives `f64::clamp` and truncates to 0 as `u8`, so without this guard a
/// 1xN render silently writes a black image instead of reporting the mistake.
pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
    if width < 2 || height < 2 {
        return Err(RenderError::InvalidDimension { width, height });
    }

    let mut image = Image::new(width, height)?;

    let objects = scene::build(Scene::Spheres);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    let viewport_height = 2.0;
    let viewport_width = viewport_height * (width as f64 / height as f64);
    let origin = Vec3::ZERO;
    let horizontal = Vec3::new(viewport_width, 0.0, 0.0);
    let vertical = Vec3::new(0.0, viewport_height, 0.0);
    let lower_left_corner = origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new(0.0, 0.0, 1.0);

    for j in 0..height {
        eprint!("\rScanlines remaining: {} ", height - j);
        for i in 0..width {
            let u = i as f64 / (width - 1) as f64;
            let v = j as f64 / (height - 1) as f64;
            let ray = Ray::new(
                origin,
                lower_left_corner + horizontal * u + vertical * v - origin,
            );

            let color = match world.hit(&ray, 0.001, f64::INFINITY) {
                Some(rec) => shade(rec.normal),
                None => sky_color(ray.direction),
            };
            // The book's chapter 4-6 camera anchors on the bottom-left corner and
            // counts j upward, putting the bottom of the viewport in the image's
            // top row. Mirror the write index so every image is the right way
            // up, as chapter 7's camera onward produces naturally.
            let row = height - 1 - j;
            image.set(i, row, color.to_rgb());
        }
    }
    eprintln!("\rDone.                 ");

    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    fn assert_vec_close(a: Vec3, b: Vec3) {
        close(a.x, b.x);
        close(a.y, b.y);
        close(a.z, b.z);
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

    #[test]
    fn sky_is_above_and_ground_below() {
        let image = render_scene(8, 8).unwrap();
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
        // there has z = (1 - t) / 100, and `shade` scales by 0.5 * (1 - n.z).
        let (a, half_b, c): (f64, f64, f64) = (3.0, -101.5, 101.25);
        let t = (-half_b - (half_b * half_b - a * c).sqrt()) / a;
        let k = 0.5 * (1.0 - (1.0 - t) / 100.0);
        assert_vec_close(Vec3::from_rgb(bottom), Vec3::new(0.2, 0.3, 0.5) * k);

        // Blue is the channel that tells the two apart: the sky pins it at 1.0
        // while `shade` holds it under 0.5. Drop `let row = height - 1 - j;`
        // and these two rows swap, so this comparison is what turns round.
        assert!(
            top[2] > bottom[2],
            "sky should sit above ground, got top {top:?} and bottom {bottom:?}"
        );
    }

    #[test]
    fn the_horizon_falls_on_the_vertical_centre_row() {
        // 9 is odd, so image row 4 is j = 4, v = 0.5 exactly and the left-edge
        // ray there is exactly horizontal (direction.y == 0). Nothing in the §6
        // world is hit by it: the ball at (0,0,-1) r = 0.5 needs dx^2 <= 1/3 and
        // the left edge has dx = -1, and the ground at (0,-100.5,-1) r = 100 is
        // 100.5 below the eye. So this pixel is `sky_color` with a = 0.5, giving
        // 0.5 * (1,1,1) + 0.5 * (0.5, 0.7, 1.0) = (0.75, 0.85, 1.0).
        let image = render_scene(9, 9).unwrap();
        let width = image.width;

        assert_vec_close(
            Vec3::from_rgb(image.pixels[4 * width]),
            Vec3::new(0.75, 0.85, 1.0),
        );

        // The ball's silhouette cuts into that horizon row: the centre ray
        // (0, 0, -1) meets it head on at t = 1 - 0.5 = 0.5, where the outward
        // normal is (0, 0, 1). That faces away from the camera, so `shade`
        // returns 0.5 * (1 - 1) = 0 and the pixel is black. The old geometry had
        // the eye on the ball's surface, so nothing could be drawn here.
        assert_eq!(image.pixels[4 * width + 4], [0.0, 0.0, 0.0]);

        // Along the left edge the horizon lands between rows 4 and 5: every row
        // above it tilts up and misses both spheres, so its blue is the sky's
        // exact 1.0; every row below tilts down into the ground, whose `shade`
        // caps blue at 0.5 * (1 - 0) = 0.25.
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
            assert!(got[2] < 0.5, "row {row} should be ground, got {got:?}");
        }
    }
}
