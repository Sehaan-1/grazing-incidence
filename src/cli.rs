//! Command-line argument parsing.

use crate::error::RenderError;

/// Where renders go until `--out` arrives in Task 12.
pub const DEFAULT_OUTPUT: &str = "images/rtiow.ppm";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub width: usize,
    pub height: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
        }
    }
}

impl Options {
    pub fn parse<I: Iterator<Item = String>>(args: I) -> Result<Self, RenderError> {
        let mut opts = Self::default();
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--width" => {
                    let v = args
                        .next()
                        .ok_or_else(|| RenderError::MissingValue { flag: arg.clone() })?;
                    opts.width = v.parse().map_err(|_| RenderError::InvalidParameter {
                        flag: arg.clone(),
                        value: v.clone(),
                    })?;
                }
                "--height" => {
                    let v = args
                        .next()
                        .ok_or_else(|| RenderError::MissingValue { flag: arg.clone() })?;
                    opts.height = v.parse().map_err(|_| RenderError::InvalidParameter {
                        flag: arg.clone(),
                        value: v.clone(),
                    })?;
                }
                other => {
                    return Err(RenderError::UnknownFlag {
                        flag: other.to_string(),
                    });
                }
            }
        }

        if opts.width == 0 || opts.height == 0 {
            return Err(RenderError::InvalidDimension {
                width: opts.width,
                height: opts.height,
            });
        }

        Ok(opts)
    }
}

pub const USAGE: &str = "\
grazing-incidence — a ray tracer

USAGE:
    grazing-incidence [OPTIONS]

OPTIONS:
    --width <N>    Image width in pixels  [default: 256]
    --height <N>   Image height in pixels [default: 256]
    --help         Print this message
";

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, RenderError> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_to_256_square() {
        assert_eq!(
            parse(&[]).unwrap(),
            Options {
                width: 256,
                height: 256
            }
        );
    }

    #[test]
    fn reads_width_and_height() {
        let opts = parse(&["--width", "640", "--height", "480"]).unwrap();
        assert_eq!(opts.width, 640);
        assert_eq!(opts.height, 480);
    }

    #[test]
    fn rejects_an_unknown_flag() {
        let err = parse(&["--nope"]).unwrap_err();
        assert!(matches!(err, RenderError::UnknownFlag { ref flag } if flag == "--nope"));
    }

    #[test]
    fn rejects_a_flag_with_no_value() {
        let err = parse(&["--width"]).unwrap_err();
        assert!(matches!(err, RenderError::MissingValue { .. }));
    }

    #[test]
    fn rejects_a_non_numeric_value() {
        let err = parse(&["--width", "wide"]).unwrap_err();
        assert!(matches!(err, RenderError::InvalidParameter { .. }));
    }

    #[test]
    fn rejects_zero_dimensions() {
        let err = parse(&["--width", "0"]).unwrap_err();
        assert!(matches!(
            err,
            RenderError::InvalidDimension { width: 0, .. }
        ));
    }
}
