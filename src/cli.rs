//! Command-line argument parsing.

use crate::camera::{CHAPTER_TWELVE_ASPECT, Camera};
use crate::error::RenderError;
use crate::scene::SCENE_NAMES;

/// Where renders go when `--out` is omitted.
pub const DEFAULT_OUTPUT: &str = "images/rtiow.ppm";

/// The book's chapter-12/13 setup, used when no flags are given.
pub const DEFAULT_WIDTH: usize = crate::camera::CHAPTER_TWELVE_WIDTH;
pub const DEFAULT_SAMPLES: usize = crate::camera::CHAPTER_TWELVE_SAMPLES;
pub const DEFAULT_SEED: u64 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub width: usize,
    pub samples_per_pixel: usize,
    pub seed: u64,
    pub scene: String,
    pub out: String,
    pub help: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            samples_per_pixel: DEFAULT_SAMPLES,
            seed: DEFAULT_SEED,
            scene: SCENE_NAMES[0].to_string(),
            out: DEFAULT_OUTPUT.to_string(),
            help: false,
        }
    }
}

impl Options {
    pub fn parse<I: Iterator<Item = String>>(args: I) -> Result<Self, RenderError> {
        let mut opts = Self::default();
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => opts.help = true,
                "--width" => {
                    opts.width = parse_usize(&mut args, "--width")?;
                }
                "--samples" => {
                    opts.samples_per_pixel = parse_usize(&mut args, "--samples")?;
                }
                "--seed" => {
                    opts.seed = parse_u64(&mut args, "--seed")?;
                }
                "--scene" => {
                    let v = next_value(&mut args, "--scene")?;
                    if !SCENE_NAMES.contains(&v.as_str()) {
                        return Err(RenderError::UnknownScene { name: v });
                    }
                    opts.scene = v;
                }
                "--out" => {
                    opts.out = next_value(&mut args, "--out")?;
                }
                other => {
                    return Err(RenderError::UnknownFlag {
                        flag: other.to_string(),
                    });
                }
            }
        }

        // `--width 0` never reaches a camera, so it is caught here. The height
        // in the error is the one §12's rule would have derived for that width,
        // rather than a made-up zero: `Camera::derived_height` is the only place
        // that rule is written down.
        if opts.width == 0 {
            return Err(RenderError::InvalidDimension {
                width: 0,
                height: Camera::derived_height(0, CHAPTER_TWELVE_ASPECT),
            });
        }
        if opts.samples_per_pixel == 0 {
            return Err(RenderError::InvalidParameter {
                flag: "--samples".to_string(),
                value: opts.samples_per_pixel.to_string(),
            });
        }

        Ok(opts)
    }
}

fn next_value<I: Iterator<Item = String>>(
    args: &mut std::iter::Peekable<I>,
    flag: &str,
) -> Result<String, RenderError> {
    args.next().ok_or_else(|| RenderError::MissingValue {
        flag: flag.to_string(),
    })
}

fn parse_usize<I: Iterator<Item = String>>(
    args: &mut std::iter::Peekable<I>,
    flag: &str,
) -> Result<usize, RenderError> {
    let v = next_value(args, flag)?;
    v.parse().map_err(|_| RenderError::InvalidParameter {
        flag: flag.to_string(),
        value: v,
    })
}

fn parse_u64<I: Iterator<Item = String>>(
    args: &mut std::iter::Peekable<I>,
    flag: &str,
) -> Result<u64, RenderError> {
    let v = next_value(args, flag)?;
    v.parse().map_err(|_| RenderError::InvalidParameter {
        flag: flag.to_string(),
        value: v,
    })
}

pub const USAGE: &str = "\
grazing-incidence — a ray tracer

USAGE:
    grazing-incidence [OPTIONS]

OPTIONS:
    --width <N>     Image width in pixels     [default: 400]
    --samples <N>   Samples per pixel         [default: 100]
    --seed <N>      PRNG seed                 [default: 0]
    --scene <NAME>  Scene to render           [default: spheres]
    --out <PATH>    Output path               [default: images/rtiow.ppm]
    --help, -h      Print this message

Height is derived from a 16:9 aspect ratio, so --width 400 gives 400x225.
";

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, RenderError> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_match_the_book() {
        let opts = parse(&[]).unwrap();
        assert_eq!(opts.width, 400);
        assert_eq!(opts.samples_per_pixel, 100);
        assert_eq!(opts.seed, 0);
        assert_eq!(opts.scene, "spheres");
        assert_eq!(opts.out, "images/rtiow.ppm");
        assert!(!opts.help);
    }

    #[test]
    fn the_defaults_are_the_ones_the_constants_promise() {
        // `Options::default` is built out of these, so a test that hardcodes
        // 400/100 would keep passing if someone retuned the defaults. These
        // read the constants back instead.
        assert_eq!(Options::default().width, DEFAULT_WIDTH);
        assert_eq!(Options::default().samples_per_pixel, DEFAULT_SAMPLES);
        assert_eq!(Options::default().seed, DEFAULT_SEED);
        assert_eq!(Options::default().out, DEFAULT_OUTPUT);
        assert_eq!(Options::default().scene, SCENE_NAMES[0]);
        // The book really does say 400 wide at 100 samples.
        assert_eq!(DEFAULT_WIDTH, 400);
        assert_eq!(DEFAULT_SAMPLES, 100);
    }

    #[test]
    fn reads_every_flag() {
        let opts = parse(&[
            "--width",
            "800",
            "--samples",
            "16",
            "--seed",
            "7",
            "--scene",
            "spheres",
            "--out",
            "images/x.ppm",
        ])
        .unwrap();
        assert_eq!(opts.width, 800);
        assert_eq!(opts.samples_per_pixel, 16);
        assert_eq!(opts.seed, 7);
        assert_eq!(opts.scene, "spheres");
        assert_eq!(opts.out, "images/x.ppm");
    }

    #[test]
    fn later_flags_win() {
        let opts = parse(&["--width", "800", "--width", "64"]).unwrap();
        assert_eq!(opts.width, 64);
    }

    #[test]
    fn help_short_and_long() {
        assert!(parse(&["--help"]).unwrap().help);
        assert!(parse(&["-h"]).unwrap().help);
    }

    #[test]
    fn rejects_an_unknown_flag() {
        let err = parse(&["--nope"]).unwrap_err();
        assert!(matches!(err, RenderError::UnknownFlag { ref flag } if flag == "--nope"));
    }

    #[test]
    fn rejects_an_unknown_scene() {
        let err = parse(&["--scene", "nope"]).unwrap_err();
        assert!(matches!(err, RenderError::UnknownScene { ref name } if name == "nope"));
    }

    #[test]
    fn rejects_a_flag_with_no_value() {
        assert!(matches!(
            parse(&["--width"]).unwrap_err(),
            RenderError::MissingValue { ref flag } if flag == "--width"
        ));
        assert!(matches!(
            parse(&["--samples"]).unwrap_err(),
            RenderError::MissingValue { ref flag } if flag == "--samples"
        ));
        assert!(matches!(
            parse(&["--seed"]).unwrap_err(),
            RenderError::MissingValue { ref flag } if flag == "--seed"
        ));
        assert!(matches!(
            parse(&["--scene"]).unwrap_err(),
            RenderError::MissingValue { ref flag } if flag == "--scene"
        ));
        assert!(matches!(
            parse(&["--out"]).unwrap_err(),
            RenderError::MissingValue { ref flag } if flag == "--out"
        ));
    }

    #[test]
    fn rejects_non_numeric_values() {
        assert!(matches!(
            parse(&["--seed", "abc"]).unwrap_err(),
            RenderError::InvalidParameter { ref flag, ref value } if flag == "--seed" && value == "abc"
        ));
        assert!(matches!(
            parse(&["--width", "wide"]).unwrap_err(),
            RenderError::InvalidParameter { ref flag, ref value } if flag == "--width" && value == "wide"
        ));
        // A negative width does not fit a usize at all.
        assert!(matches!(
            parse(&["--width", "-4"]).unwrap_err(),
            RenderError::InvalidParameter { .. }
        ));
    }

    #[test]
    fn rejects_zero_width() {
        // The height is the one §12's rule derives for a zero width: 0 / (16/9)
        // = 0, floored at 1.
        assert_eq!(Camera::derived_height(0, CHAPTER_TWELVE_ASPECT), 1);
        assert!(matches!(
            parse(&["--width", "0"]).unwrap_err(),
            RenderError::InvalidDimension {
                width: 0,
                height: 1
            }
        ));
    }

    #[test]
    fn rejects_zero_samples() {
        assert!(matches!(
            parse(&["--samples", "0"]).unwrap_err(),
            RenderError::InvalidParameter { ref flag, ref value }
                if flag == "--samples" && value == "0"
        ));
    }

    #[test]
    fn every_advertised_scene_name_parses() {
        for name in SCENE_NAMES {
            let opts = parse(&["--scene", name]).unwrap();
            assert_eq!(&opts.scene, name);
        }
    }

    #[test]
    fn usage_agrees_with_the_defaults() {
        // The help text quotes the defaults in prose. If a default moves, the
        // help should not quietly start lying.
        assert!(USAGE.contains("[default: 400]"));
        assert!(USAGE.contains("[default: 100]"));
        assert!(USAGE.contains("[default: images/rtiow.ppm]"));
        assert!(USAGE.contains("[default: spheres]"));
        assert!(USAGE.contains("400x225"));
    }
}
