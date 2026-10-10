//! The pixel loop.

use crate::error::RenderError;
use crate::image::Image;
use crate::ray::Ray;
use crate::sphere::Sphere;
use crate::vec3::Vec3;

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

/// The sky: white at the horizon, blue overhead (§4).
fn sky_color(direction: Vec3) -> Vec3 {
    let unit_direction = direction.unit();
    let a = 0.5 * (unit_direction.y + 1.0);
    Vec3::UNIT * (1.0 - a) + Vec3::new(0.5, 0.7, 1.0) * a
}

/// The book sphere and the sky behind it (§5).
fn render_sphere(width: usize, height: usize) -> Result<Image, RenderError> {
    let mut image = Image::new(width, height)?;

    let viewport_height = 2.0;
    let viewport_width = viewport_height * (width as f64 / height as f64);

    let origin = Vec3::ZERO;
    let horizontal = Vec3::new(viewport_width, 0.0, 0.0);
    let vertical = Vec3::new(0.0, viewport_height, 0.0);
    let lower_left_corner = origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new(0.0, 0.0, 1.0);

    let sphere = Sphere::new(Vec3::new(0.0, 0.0, -1.0), 0.5);

    for j in 0..height {
        eprint!("\rScanlines remaining: {} ", height - j);
        for i in 0..width {
            let u = i as f64 / (width - 1) as f64;
            let v = j as f64 / (height - 1) as f64;
            let ray = Ray::new(
                origin,
                lower_left_corner + horizontal * u + vertical * v - origin,
            );

            let color = match sphere.hit(&ray, 0.001, f64::INFINITY) {
                Some(t) => {
                    let normal = (ray.at(t) - sphere.center).unit();
                    Vec3::new(1.0, 0.2, 0.2) * (0.5 * (normal.dot(Vec3::new(0.0, 0.0, -1.0)) + 1.0))
                }
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

/// Cast one ray per pixel and shade each hit against the sky (§5).
///
/// `width` and `height` must both be at least 2: the loop normalises each axis by
/// `axis - 1`, so a 1-pixel axis divides by zero and every channel becomes NaN.
/// NaN survives `f64::clamp` and truncates to 0 as `u8`, so without this guard a
/// 1xN render silently writes a black image instead of reporting the mistake.
pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
    if width < 2 || height < 2 {
        return Err(RenderError::InvalidDimension { width, height });
    }

    render_sphere(width, height)
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
    fn image_is_the_right_way_up_blue_above_white() {
        let image = render_scene(8, 8).unwrap();
        let width = image.width;

        // Row-major, top row first: [0] is the top-left pixel, [7 * width] the
        // bottom-left one.
        let top = image.pixels[0];
        let bottom = image.pixels[7 * width];

        // Both corners are pure sky, not the sphere. The sphere at (0, 0, -1)
        // with radius 0.5 subtends asin(0.5) = 30 degrees from the origin, and
        // a corner ray is (-1, +/-1, -1), which is 54.7 degrees off the centre
        // axis. Those corner directions are exactly what the camera builds for
        // u = 0 at v = 1 and v = 0.
        //
        // Which row each lands in is the whole claim, so anchor them: the top of
        // the frame must be the ray looking up (+y, the blue tint) and the bottom
        // the ray looking down (-y, white). A mirrored render swaps the two.
        for (got, expected) in [
            (top, sky_color(Vec3::new(-1.0, 1.0, -1.0)).to_rgb()),
            (bottom, sky_color(Vec3::new(-1.0, -1.0, -1.0)).to_rgb()),
        ] {
            close(got[0], expected[0]);
            close(got[1], expected[1]);
            close(got[2], expected[2]);
        }

        // "How blue?" has to be read off red, never blue: sky_color pins blue at
        // 1.0 for both the white and the tint, so blue never discriminates.
        // Red is the channel that moves -- 1.0 - 0.5 * a -- and a rises as the
        // ray tilts up. The top row is therefore red-poor and the bottom nearly
        // white.
        assert!(
            top[0] < bottom[0],
            "top row should be the red-poor sky, got top {top:?} and bottom {bottom:?}"
        );
    }
}
