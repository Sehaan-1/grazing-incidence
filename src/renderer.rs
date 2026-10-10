//! The pixel loop.

use crate::error::RenderError;
use crate::image::Image;

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

/// The book's first image: a red/green gradient.
pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
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
