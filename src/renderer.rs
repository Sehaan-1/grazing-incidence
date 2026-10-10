//! The pixel loop.

use crate::error::RenderError;
use crate::image::Image;

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

/// The book's first image: a red/green gradient.
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

    for j in 0..height {
        eprint!("\rScanlines remaining: {} ", height - j);
        for i in 0..width {
            let r = i as f64 / (width - 1) as f64;
            let g = j as f64 / (height - 1) as f64;
            let b = 0.0;
            image.set(i, j, [r + 0.5, g + 0.5, b + 0.5]);
        }
    }
    eprintln!("\rDone.                 ");

    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
