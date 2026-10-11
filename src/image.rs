//! Image buffer and encoders.

use std::path::Path;

use crate::error::RenderError;

/// The largest image this renderer will allocate, in pixels.
///
/// Every image is a `Vec<[f64; 3]>`, so the buffer is `pixels * 24` bytes. The
/// cap exists for two separate reasons and both have to hold:
///
/// - **The multiply.** `width * height` is computed on `usize`. `--width
///   4000000000` at the derived 16:9 height gives `4e9 * 2250000000`, which
///   overflows and panics with `capacity overflow` -- an abort, not an `Err`.
///   `checked_mul` turns that into the `InvalidDimension` the CLI contract
///   promises.
/// - **The allocation.** Even where the product fits, it does not mean the
///   memory does. `1 << 28` pixels would be 6.4 GB of `f64`, so a cap that
///   generous would not stop the abort it was meant to prevent.
///
/// `1 << 24` = 16,777,216 pixels is 402.7 MB, and it covers every render worth
/// committing: this project's largest is the §14 cover at 1200 x 675 = 810,000
/// pixels (19.4 MB), and it reaches 4K-class frames -- 3840 x 2160 = 8,294,400
/// and 4096 x 2160 = 8,847,360 -- while turning away 8K's 7680 x 4320 = 33,177,600.
///
/// It also closes the `width as u32` on the PNG path: both axes are below
/// `2^24 < u32::MAX`, so `encode_rgb`'s dimensions cannot silently truncate.
pub const MAX_PIXELS: usize = 1 << 24;

/// A linear-light RGB image. Pixels are stored row-major, top row first.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f64; 3]>,
}

impl Image {
    pub fn new(width: usize, height: usize) -> Result<Self, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::InvalidDimension { width, height });
        }
        let count = width
            .checked_mul(height)
            .ok_or(RenderError::InvalidDimension { width, height })?;
        if count > MAX_PIXELS {
            return Err(RenderError::InvalidDimension { width, height });
        }
        Ok(Self {
            width,
            height,
            pixels: vec![[0.0; 3]; count],
        })
    }

    pub fn set(&mut self, x: usize, y: usize, pixel: [f64; 3]) {
        self.pixels[y * self.width + x] = pixel;
    }

    /// The gamma-encoded 8-bit RGB payload both encoders share.
    ///
    /// `gamma` applies the sqrt encode from §9 to each linear component.
    /// Components are clamped just under 1.0 before scaling, so a fully lit
    /// surface lands on 255 rather than wrapping to 0.
    pub fn to_rgb_bytes(&self, gamma: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 3);
        for px in &self.pixels {
            for &c in px {
                let v = if gamma { c.sqrt() } else { c };
                out.push((256.0 * v.clamp(0.0, 0.999)) as u8);
            }
        }
        out
    }

    /// Encode as binary PPM (P6).
    ///
    /// The book's text writes P3, plain ASCII. P6 is the same format with a
    /// binary payload: one quarter the size, and one `write` instead of
    /// per-channel formatting. The book's own released `inOneWeekend` also
    /// emits P6.
    pub fn write_ppm(&self, path: &Path, gamma: bool) -> Result<(), RenderError> {
        let header = format!("P6\n{} {}\n255\n", self.width, self.height);
        let mut bytes = header.into_bytes();
        bytes.extend_from_slice(&self.to_rgb_bytes(gamma));
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Encode as PNG. Same pixels as `write_ppm`, roughly a twentieth of the
    /// size, and previewable on GitHub.
    ///
    /// The dimensions go to `png::encode_rgb` as `u32`, which is where a
    /// `usize` wider than 32 bits would truncate silently and write a file whose
    /// header disagrees with its own payload. `new` already bounds both axes
    /// below `u32::MAX`, but `width` and `height` are public, so this converts
    /// rather than trusting that.
    pub fn write_png(&self, path: &Path, gamma: bool) -> Result<(), RenderError> {
        let width = u32::try_from(self.width).map_err(|_| RenderError::InvalidDimension {
            width: self.width,
            height: self.height,
        })?;
        let height = u32::try_from(self.height).map_err(|_| RenderError::InvalidDimension {
            width: self.width,
            height: self.height,
        })?;
        let rgb = self.to_rgb_bytes(gamma);
        let png = crate::png::encode_rgb(width, height, &rgb);
        std::fs::write(path, png)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn new_rejects_zero_width() {
        let err = Image::new(0, 4).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension {
                width: 0,
                height: 4
            }
        ));
    }

    #[test]
    fn new_rejects_zero_height() {
        let err = Image::new(4, 0).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension {
                width: 4,
                height: 0
            }
        ));
    }

    #[test]
    fn new_allocates_width_times_height() {
        let img = Image::new(7, 3).unwrap();
        assert_eq!(img.pixels.len(), 21);
    }

    #[test]
    fn set_writes_at_row_major_offset() {
        let mut img = Image::new(4, 3).unwrap();
        img.set(2, 1, [1.0, 0.0, 0.0]);
        assert_eq!(img.pixels[1 * 4 + 2], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn write_ppm_emits_p6_header_then_exact_byte_count() {
        let path = temp("gi_test_header.ppm");
        Image::new(4, 3).unwrap().write_ppm(&path, false).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let header = b"P6\n4 3\n255\n";
        assert_eq!(&bytes[..header.len()], header);
        assert_eq!(bytes.len() - header.len(), 4 * 3 * 3);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn write_ppm_scales_and_clamps_components() {
        let path = temp("gi_test_scale.ppm");
        let mut img = Image::new(2, 1).unwrap();
        img.set(0, 0, [0.5, 1.0, 2.0]);
        img.set(1, 0, [-1.0, 0.0, 0.25]);
        img.write_ppm(&path, false).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let body = &bytes["P6\n2 1\n255\n".len()..];
        // 0.5 -> 128, 1.0 -> 255, 2.0 clamps to 255
        // -1.0 clamps to 0, 0.0 -> 0, 0.25 -> 64
        assert_eq!(body, &[128, 255, 255, 0, 0, 64]);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn gamma_true_applies_sqrt_before_scaling() {
        let path = temp("gi_test_gamma.ppm");
        let mut img = Image::new(1, 1).unwrap();
        img.set(0, 0, [0.25, 0.25, 0.25]);
        img.write_ppm(&path, true).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let body = &bytes["P6\n1 1\n255\n".len()..];
        // sqrt(0.25) = 0.5 -> 128
        assert_eq!(body, &[128, 128, 128]);

        std::fs::remove_file(&path).unwrap();
    }

    /// The reachable panic this guards. `width * height` used to be evaluated
    /// and then handed straight to `vec!`, so any product that overflowed
    /// `usize` aborted with `capacity overflow` instead of returning `Err` --
    /// exit 101, not the non-zero exit the spec promises.
    ///
    /// `usize::MAX / 4 * 4` is the largest multiple of 4 below `usize::MAX`;
    /// one more and it wraps. On this host `usize` is 64 bits, so
    /// `usize::MAX = 18446744073709551615` and `usize::MAX / 4 = 4611686018427387903`,
    /// which times 4 is `18446744073709551612`, exactly `usize::MAX - 3`.
    #[test]
    fn new_rejects_a_pixel_count_that_overflows_usize() {
        let err = Image::new(usize::MAX / 4 + 1, 4).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension {
                width: _,
                height: 4
            }
        ));

        // And the real one from the command line: `--width 4000000000` at the
        // 16:9 height 2250000000 gives 9e18, which is inside `usize` on a
        // 64-bit host -- so this case is caught by the cap, not the overflow,
        // and it must still be an `Err`.
        let err = Image::new(4_000_000_000, 2_250_000_000).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension {
                width: 4_000_000_000,
                height: 2_250_000_000
            }
        ));
    }

    #[test]
    fn new_rejects_more_pixels_than_the_cap_allows() {
        // `2^24` is the cap, `2^24 + 1` the smallest product over it. Both fit
        // in `usize` comfortably, so only the cap can reject these.
        assert!(Image::new(MAX_PIXELS / 16, 16).is_ok());
        assert!(Image::new(MAX_PIXELS / 16, 17).is_err());

        // A square over it: 4097 x 4097 = 16,785,409 > 16,777,216.
        assert_eq!(4096 * 4096, MAX_PIXELS);
        assert!(Image::new(4096, 4096).is_ok());
        assert!(Image::new(4097, 4097).is_err());

        // The §14 cover, the largest render this project makes, is 1200 x 675
        // and must still go through untouched.
        let cover = Image::new(1200, 675).unwrap();
        assert_eq!(cover.pixels.len(), 810_000);
        assert!(810_000 < MAX_PIXELS);
    }

    /// `write_png` narrows the dimensions to `u32`. `new` bounds them well
    /// below that, but the fields are public, so a caller can put one back.
    /// Before this was checked, `width as u32` truncated silently and the file's
    /// IHDR disagreed with its own IDAT payload.
    #[test]
    fn write_png_refuses_dimensions_wider_than_a_u32() {
        let mut img = Image::new(2, 2).unwrap();
        img.width = u32::MAX as usize + 1;
        let err = img
            .write_png(&temp("gi_test_toowide.png"), false)
            .unwrap_err();
        assert!(matches!(err, RenderError::InvalidDimension { .. }));
    }
}
