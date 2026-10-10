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
    fn image_is_the_right_way_up_ground_below_ball() {
        let image = render_scene(8, 8).unwrap();
        let width = image.width;

        // Row-major, top row first: [0] is the top-left pixel and
        // [7 * width] the bottom-left one, i.e. the two viewport corner rays.
        //
        // The §6 eye sits exactly on the surface of *both* spheres: the origin
        // is 0.5 from the ball's centre and 1000 from the ground's, which is
        // each sphere's own radius. So a ray enters the ball whenever it tilts
        // up and the ground whenever it tilts down, the upper half of the frame
        // is ball and the lower half is ground, and the row mirror in
        // `render_scene` is what keeps the ground at the bottom.
        //
        // Up-corner ray (-1, 1, -1): the ball at (0, 0.5, 0) r 0.5 has roots
        // t = 0 -- the origin itself, rejected by t_min -- and t = 1/3. The
        // outward normal there is (-2/3, -1/3, -2/3); the ray arrives from
        // inside, so the normal is flipped to (2/3, 1/3, 2/3), which faces the
        // camera by -2/3. `shade` multiplies by 0.5 * (1 - 2/3) = 1/6.
        let expected = Vec3::new(0.2, 0.3, 0.5) / 6.0;
        assert_vec_close(Vec3::from_rgb(image.pixels[0]), expected);
        assert_vec_close(Vec3::from_rgb(image.pixels[7 * width]), expected);

        // The down-corner ray hits the ground and mirrors that exactly, which is
        // why the two corners share a colour: this scene makes the frame
        // vertically symmetric, so a colour check can no longer tell a mirrored
        // write index from a correct one. `the_horizon_falls_on_the_vertical_
        // centre_row` is the observable half of the layout.
        assert!(image.pixels.iter().all(|p| p[2] < 0.5));
    }

    #[test]
    fn the_horizon_falls_on_the_vertical_centre_row() {
        // 9 is odd, so j = 4 gives v = 0.5 exactly and the camera ray there is
        // exactly horizontal (direction.y == 0). Both spheres are tangent to
        // the origin, so a horizontal ray enters neither -- it misses both and
        // takes the sky. `sky_color` gives a horizontal ray a = 0.5, hence
        // 0.5 + 0.5 * (0.5, 0.7, 1.0) = (0.75, 0.85, 1.0).
        let image = render_scene(9, 9).unwrap();
        let width = image.width;

        let horizon = 4 * width;
        for x in 0..width {
            let got = image.pixels[horizon + x];
            for (c, expected) in [0.75, 0.85, 1.0].iter().enumerate() {
                close(got[c], *expected);
            }
        }

        // The rows either side are the ball (looking up) and the ground (looking
        // down). Blue is the channel that separates them from sky: `sky_color`
        // pins it at 1.0, while `shade` scales it by 0.5 * (1 - n.z) <= 0.5.
        assert!(image.pixels[3 * width][2] < 0.5);
        assert!(image.pixels[5 * width][2] < 0.5);
    }
}
