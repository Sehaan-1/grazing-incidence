//! Command-line argument parsing.

use std::path::Path;

use crate::camera::{Camera, PINHOLE_ASPECT};
use crate::error::RenderError;
use crate::scene::Scene;

/// Where renders go when `--out` is omitted.
pub const DEFAULT_OUTPUT: &str = "images/render.ppm";

/// The default camera's setup, used when no flags are given.
pub const DEFAULT_WIDTH: usize = crate::camera::PINHOLE_WIDTH;
pub const DEFAULT_SAMPLES: usize = crate::camera::PINHOLE_SAMPLES;
pub const DEFAULT_SEED: u64 = 0;

/// The two image encoders `--out` can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    Ppm,
    Png,
}

/// The extensions `encoder_for` accepts, longest first so the help text can
/// print them straight out.
pub const OUTPUT_EXTENSIONS: &[&str] = &["ppm", "png"];

/// Pick an encoder from a path's extension.
///
/// `.ppm` and `.png` select the two encoders, case-insensitively. **Anything
/// else is an error**, including a path with no extension at all. The earlier
/// code asked only "is this `.png`?" and fell back to PPM for every other name,
/// so `--out x.txt` silently wrote a *binary* P6 file under a name that promises
/// text -- a mislabelling the caller had no way to see from the program's exit
/// code or output.
///
/// The flag is reported as `--out` and the value names the path *and* what was
/// expected of it, so the message reads "invalid value for --out: x.txt
/// (expected .ppm or .png)" rather than pretending a path failed to parse as a
/// number.
pub fn encoder_for(path: &Path) -> Result<Encoder, RenderError> {
    match path.extension().and_then(|e| e.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("png") => Ok(Encoder::Png),
        Some(extension) if extension.eq_ignore_ascii_case("ppm") => Ok(Encoder::Ppm),
        _ => Err(RenderError::InvalidParameter {
            flag: "--out".to_string(),
            value: format!("{} (expected .ppm or .png)", path.display()),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub width: usize,
    pub samples_per_pixel: usize,
    pub seed: u64,
    pub scene: Scene,
    pub out: String,
    pub help: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            samples_per_pixel: DEFAULT_SAMPLES,
            seed: DEFAULT_SEED,
            scene: Scene::Spheres,
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
                    opts.scene = Scene::parse(&v)?;
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
        // in the error is the one the height rule would have derived for that
        // width,
        // rather than a made-up zero: `Camera::derived_height` is the only place
        // that rule is written down.
        if opts.width == 0 {
            return Err(RenderError::InvalidDimension {
                width: 0,
                height: Camera::derived_height(0, PINHOLE_ASPECT),
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
    --out <PATH>    Output path               [default: images/render.ppm]
    --help, -h      Print this message

Height is derived from a 16:9 aspect ratio, so --width 400 gives 400x225.
--out takes a .ppm or a .png path and picks that encoder; any other
extension is an error.
Scenes: spheres, cover. The cover scene is the largest image: 1200x675
at 500 samples per pixel, a field of 484 randomly placed spheres. It is
hours of CPU time at that size, so start smaller --
--scene cover --width 80 --samples 4 --out images/tmp.png.
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::COVER_WIDTH as COVER_WIDTH_SIZE;
    use crate::scene::SCENE_NAMES;

    fn parse(args: &[&str]) -> Result<Options, RenderError> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_match_the_default_camera() {
        let opts = parse(&[]).unwrap();
        assert_eq!(opts.width, 400);
        assert_eq!(opts.samples_per_pixel, 100);
        assert_eq!(opts.seed, 0);
        assert_eq!(opts.scene, Scene::Spheres);
        assert_eq!(opts.out, "images/render.ppm");
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
        // The default scene is the *first advertised name*, not a literal. That
        // is what keeps `--scene`'s help text, `SCENE_NAMES` and the default
        // from drifting apart when a scene is added or reordered.
        assert_eq!(Options::default().scene, Scene::Spheres);
        assert_eq!(Scene::parse(SCENE_NAMES[0]).unwrap(), Scene::Spheres);
        // The default camera really is 400 wide at 100 samples.
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
            "cover",
            "--out",
            "images/x.ppm",
        ])
        .unwrap();
        assert_eq!(opts.width, 800);
        assert_eq!(opts.samples_per_pixel, 16);
        assert_eq!(opts.seed, 7);
        assert_eq!(opts.scene, Scene::Cover);
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
        assert!(matches!(err, RenderError::UnknownScene(ref name) if name == "nope"));
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
        // The height is the one the rule derives for a zero width: 0 / (16/9)
        // = 0, floored at 1.
        assert_eq!(Camera::derived_height(0, PINHOLE_ASPECT), 1);
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
            assert_eq!(opts.scene, Scene::parse(name).unwrap());
        }
        // The two names are the two scenes; a name that parsed to something
        // unreachable from `Scene::parse` would not be caught by the loop above.
        assert_eq!(SCENE_NAMES.len(), 2);
    }

    #[test]
    fn usage_agrees_with_the_defaults() {
        // The help text quotes the defaults in prose. If a default moves, the
        // help should not quietly start lying.
        assert!(USAGE.contains("[default: 400]"));
        assert!(USAGE.contains("[default: 100]"));
        assert!(USAGE.contains("[default: images/render.ppm]"));
        assert!(USAGE.contains("[default: spheres]"));
        assert!(USAGE.contains("400x225"));
    }

    /// The cover is 1200x675, not 1200x500. The help used to say "1200x500",
    /// which is `COVER_WIDTH x COVER_SAMPLES` -- width by *samples*, read as
    /// width by height. The two are different numbers and only one of them is a
    /// dimension, so the text is checked against the constants rather than
    /// trusted.
    #[test]
    fn usage_quotes_the_cover_size_the_constants_actually_produce() {
        assert_eq!(crate::camera::COVER_WIDTH, 1200);
        assert_eq!(crate::camera::COVER_SAMPLES, 500);
        // 1200 / (16/9) = 1200 * 9 / 16 = 675 exactly.
        assert_eq!(
            Camera::derived_height(COVER_WIDTH_SIZE, crate::camera::COVER_ASPECT),
            675
        );
        assert!(USAGE.contains("1200x675"));
        assert!(USAGE.contains("500 samples per pixel"));
        assert!(
            !USAGE.contains("1200x500"),
            "1200x500 is width by samples, not a size"
        );
    }

    /// The encoder comes from the extension, and only from the two extensions
    /// that exist.
    #[test]
    fn the_extension_picks_the_encoder() {
        assert_eq!(encoder_for(Path::new("a.png")).unwrap(), Encoder::Png);
        assert_eq!(encoder_for(Path::new("a.ppm")).unwrap(), Encoder::Ppm);
        // A directory prefix and a dot in the stem are both fine.
        assert_eq!(
            encoder_for(Path::new("images/out.png")).unwrap(),
            Encoder::Png
        );
        assert_eq!(
            encoder_for(Path::new("images/my.render.ppm")).unwrap(),
            Encoder::Ppm
        );
        // Case-insensitive, because Windows filesystems are.
        assert_eq!(encoder_for(Path::new("A.PNG")).unwrap(), Encoder::Png);
        assert_eq!(encoder_for(Path::new("A.Ppm")).unwrap(), Encoder::Ppm);
    }

    /// Every other extension is an error. The bug this pins is the *silent*
    /// part: `--out x.txt` used to write a binary P6 file under a name that
    /// promises text, and the program reported success.
    #[test]
    fn an_unknown_output_extension_is_an_error() {
        for bad in ["x.txt", "x.jpg", "x.ppmx", "x.png.txt", "x", "x."] {
            let err = encoder_for(Path::new(bad)).unwrap_err();
            assert!(
                matches!(err, RenderError::InvalidParameter { ref flag, .. } if flag == "--out"),
                "{bad} should have been refused, got {err}"
            );
            assert!(
                err.to_string().contains(bad),
                "the message should name the path, got `{err}`"
            );
        }
        // And it should say what would have been accepted, rather than the
        // "is not a positive integer" that `InvalidParameter`'s other user --
        // a number -- prompted.
        let err = encoder_for(Path::new("x.txt")).unwrap_err();
        assert!(err.to_string().contains(".ppm"), "got `{err}`");
        assert!(err.to_string().contains(".png"), "got `{err}`");
        // And the two that work are still the two the help advertises.
        assert_eq!(OUTPUT_EXTENSIONS.len(), 2);
        assert!(OUTPUT_EXTENSIONS.contains(&"ppm") && OUTPUT_EXTENSIONS.contains(&"png"));
        for extension in OUTPUT_EXTENSIONS {
            // `Path::new("png")` has no *extension* -- it is a bare stem -- so
            // the advertised names are checked as `out.png`, which is how they
            // appear in the help.
            let advertised = format!("images/render.{extension}");
            assert_eq!(
                encoder_for(Path::new(&advertised)).is_ok(),
                true,
                "{advertised} is advertised but refused"
            );
        }
    }
}
