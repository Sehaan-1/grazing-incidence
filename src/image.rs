//! Image buffer and encoders.

use std::path::Path;

use crate::error::RenderError;

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
        Ok(Self {
            width,
            height,
            pixels: vec![[0.0; 3]; width * height],
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
    pub fn write_png(&self, path: &Path, gamma: bool) -> Result<(), RenderError> {
        let rgb = self.to_rgb_bytes(gamma);
        let png = crate::png::encode_rgb(self.width as u32, self.height as u32, &rgb);
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
}
