# grazing-incidence — Core Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the core of a Rust ray tracer by porting chapters 2–11 of *Ray Tracing in One Weekend*, one commit per chapter.

**Architecture:** A single Cargo package with a `lib` (all renderer code) and a `bin` (scene setup + CLI). Modules appear as their chapters land: `vec3`, `ray`, `interval`, `hittable`, `sphere`, `material`, `camera`, `rng`, `image`, `png`, `renderer`, `scene`, `cli`, `error`. Zero dependencies — PRNG, PPM/PNG writing, argument parsing and error types are all hand-rolled.

**Tech Stack:** Rust 1.99 stable (`x86_64-pc-windows-gnu`), Cargo, no crates.

**Source book:** *Ray Tracing in One Weekend* v4.0.2 by Peter Shirley. The book is a local, gitignored file at the repository root and is **not** in git. Each task cites the book section it ports.

**Companion plan:** `2026-10-10-raytracer-plan-b-quality-and-png.md` covers chapters 12–14 (positionable camera, defocus blur, PNG output).

## Global Constraints

These apply to every task. Read them before starting.

- **Zero dependencies.** `Cargo.toml` `[dependencies]` stays empty. Never add a crate. If you feel you need one, stop and report it.
- **Toolchain.** Rust stable, GNU host. Verify with `rustc --version` (expect 1.99.x). If `cargo` is not found, prefix commands with `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"`.
- **Tests.** Unit tests live in `#[cfg(test)] mod tests` at the bottom of the module file they test. No `tests/` directory.
- **Float comparisons** use an explicit epsilon helper `assert_close(a, b, eps)` or a local `close(a, b)` fn. Exact `==` is used only where the value is exactly representable (e.g. `t == 10.5`).
- **Zero warnings.** `cargo build` and `cargo test` must produce no warnings. No `#[allow(dead_code)]`.
- **Commit gate.** Every task ends with: `cargo build` clean, `cargo test` green, image rendered to `images/`, `git commit`. Do not commit a task whose tests fail.
- **Test counts.** Step verification says "all tests pass", not a cumulative number. The earlier drafts of this plan asserted cumulative counts and they were wrong. If a step names a count, treat it as a hint, not a gate — never write an extra test to make a number come out.
- **Verifying RED in Rust.** Rust does not compile a file in `src/` that no `mod` statement references. So creating a module and its tests before declaring it in `lib.rs` produces a *successful build with zero tests collected* — not a compile failure. There is no red phase in the "it won't compile" sense. To actually verify a test has teeth, use a **mutation check**: temporarily break the implementation in a way the test should catch, confirm the test fails, then revert. Say in your report which mutation you used and what it proved.
- **Commit message** cites the book section, e.g. `feat(ch05): ray-sphere intersection (Ray Tracing in One Weekend §5)`.
- **Do not touch `docs/superpowers/specs/`.** It is the approved design; this plan implements it.
- **`Image` stores pixels as `Vec<[f64; 3]>`**, never `Vec3`. `vec3` does not exist at chapter 2, and keeping the buffer type fixed means `image.rs` is never rewritten. Convert with `Vec3::to_rgb` / `Vec3::from_rgb` at the boundary.
- **Gamma is applied at write time**, via a `gamma: bool` parameter on `write_ppm` / `write_png`, controlled by a single `renderer::GAMMA` constant. Never apply gamma during accumulation.
- **The `< 2` dimension guard must survive every rewrite.** Fix commit `15a27ee` established that any entry point computing `i / (width - 1)` must reject `width < 2 || height < 2` with `Err(RenderError::InvalidDimension { .. })`. Without it, `--width 1` produces `0.0 / 0.0` = NaN; NaN survives `f64::clamp`, and `NaN as u8` yields 0, so the image is silently black instead of an error. **Several code blocks below were written before that fix and omit the guard — if you are replacing a render entry point wholesale, carry the guard over.** It belongs in the function that does the division, not only in `Options::parse`.

## File Structure

| File | Responsibility | Lands |
|---|---|---|
| `src/lib.rs` | module declarations, `PI`, `degrees_to_radians` | Task 1 |
| `src/error.rs` | `RenderError` enum, `Display`/`Error`/`From<io::Error>` impls | Task 1 |
| `src/image.rs` | `Image` buffer, `write_ppm` | Task 1 |
| `src/renderer.rs` | `GAMMA` flag, pixel loop, progress | Task 1 |
| `src/cli.rs` | `Options` and argument parsing | Task 1 |
| `src/main.rs` | entry point, error reporting | Task 1 |
| `src/vec3.rs` | `Vec3`, `Color`, ops, `reflect`, `refract` | Task 2 |
| `src/ray.rs` | `Ray` | Task 3 |
| `src/sphere.rs` | `Sphere` and its `Hittable` impl | Task 4 |
| `src/interval.rs` | `Interval` | Task 5 |
| `src/hittable.rs` | `Hittable` trait, `HitRecord`, `HittableList` | Task 5 |
| `src/scene.rs` | `Scene` enum and scene builder | Task 5 |
| `src/camera.rs` | `Camera` | Task 6 |
| `src/rng.rs` | PCG32 | Task 7 |
| `src/material.rs` | `Material` enum, `scatter` | Task 9 |

Dependency direction is acyclic: `vec3` and `error` depend on nothing; `interval` on nothing; `ray` on `vec3`; `rng` on `vec3`; `hittable` on `ray` + `vec3`; `sphere` on `hittable`; `material` on `hittable` + `rng` + `vec3`; `camera` on all of the above; `image` on `error`; `renderer` on `camera` + `image` + `scene`; `scene` on `sphere` + `material` + `hittable`; `cli` on `error` + `scene`.

---

## Task 1: PPM output (§2 Output an Image)

**Files:**
- Create: `src/lib.rs`, `src/error.rs`, `src/image.rs`, `src/renderer.rs`, `src/cli.rs`, `src/main.rs`
- Delete: `src/scene.rs` (the placeholder from commit 0; re-created in Task 5)

**Interfaces:**
- Consumes: nothing (first task)
- Produces:
  - `error::RenderError` — `InvalidDimension { width, height }`, `MissingValue { flag }`, `InvalidParameter { flag, value }`, `UnknownFlag { flag }`, `Io(std::io::Error)`
  - `image::Image { pub width: usize, pub height: usize, pub pixels: Vec<[f64; 3]> }` with `Image::new(width, height) -> Result<Image, RenderError>`, `Image::set(&mut self, x, y, [f64;3])`, `Image::write_ppm(&self, path: &Path, gamma: bool) -> Result<(), RenderError>`
  - `renderer::GAMMA: bool` (`false` at this chapter)
  - `renderer::render_scene(width, height) -> Result<Image, RenderError>`
  - `cli::Options { pub width: usize, pub height: usize }` with `Options::parse<I: Iterator<Item = String>>(args: I) -> Result<Options, RenderError>`
  - `cli::DEFAULT_OUTPUT`, `cli::USAGE`
  - `lib::PI: f64`, `lib::degrees_to_radians(f64) -> f64`

- [ ] **Step 1: Write the failing tests**

Create `src/error.rs`:

```rust
use std::fmt;

/// Everything that can go wrong during a render.
#[derive(Debug)]
pub enum RenderError {
    InvalidDimension { width: usize, height: usize },
    MissingValue { flag: String },
    InvalidParameter { flag: String, value: String },
    UnknownFlag { flag: String },
    Io(std::io::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimension { width, height } => write!(
                f,
                "invalid image dimensions {width}x{height}: both must be non-zero"
            ),
            Self::MissingValue { flag } => write!(f, "{flag} requires a value"),
            Self::InvalidParameter { flag, value } => {
                write!(f, "invalid value for {flag}: {value} is not a positive integer")
            }
            Self::UnknownFlag { flag } => write!(f, "unknown option: {flag}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::InvalidDimension { .. }
            | Self::MissingValue { .. }
            | Self::InvalidParameter { .. }
            | Self::UnknownFlag { .. } => None,
        }
    }
}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
```

Create `src/image.rs`:

```rust
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
        Ok(Self { width, height, pixels: vec![[0.0; 3]; width * height] })
    }

    pub fn set(&mut self, x: usize, y: usize, pixel: [f64; 3]) {
        self.pixels[y * self.width + x] = pixel;
    }

    /// Encode as binary PPM (P6).
    ///
    /// The book's text writes P3, plain ASCII. P6 is the same format with a
    /// binary payload: one quarter the size, and one `write` instead of
    /// per-channel formatting. The book's own released `inOneWeekend` also
    /// emits P6.
    ///
    /// `gamma` applies the sqrt encode from §9 to each linear component.
    pub fn write_ppm(&self, path: &Path, gamma: bool) -> Result<(), RenderError> {
        let mut out = Vec::with_capacity(self.pixels.len() * 3);
        for px in &self.pixels {
            for &c in px {
                let v = if gamma { c.sqrt() } else { c };
                out.push((256.0 * v.clamp(0.0, 0.999)) as u8);
            }
        }
        let header = format!("P6\n{} {}\n255\n", self.width, self.height);
        let mut bytes = header.into_bytes();
        bytes.extend_from_slice(&out);
        std::fs::write(path, bytes)?;
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
        assert!(matches!(err, RenderError::InvalidDimension { width: 0, height: 4 }));
    }

    #[test]
    fn new_rejects_zero_height() {
        let err = Image::new(4, 0).unwrap_err();
        assert!(matches!(err, RenderError::InvalidDimension { width: 4, height: 0 }));
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
```

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: the modules are not yet declared in `lib.rs`, so cargo builds them zero times and reports zero tests collected. That is expected — Rust does not compile a file no `mod` statement references.

To prove these tests actually have teeth before moving on, run a **mutation check**: temporarily break `Image::new`'s zero-dimension guard and `write_ppm`'s gamma branch, confirm the four tests that cover them fail, then revert. Record the mutation and its result as your RED evidence.

- [ ] **Step 2: Write `src/lib.rs`**

```rust
//! `grazing-incidence` — a ray tracer built by porting *Ray Tracing in One
//! Weekend* (v4.0.2) to Rust.
//!
//! Modules appear as their chapters land; see `docs/superpowers/specs/` for the
//! chapter-to-commit mapping.

pub mod cli;
pub mod error;
pub mod image;
pub mod renderer;

/// Pi, as used throughout the book.
pub const PI: f64 = 3.1415926535897932385;

pub fn degrees_to_radians(degrees: f64) -> f64 {
    degrees * PI / 180.0
}
```

- [ ] **Step 3: Write `src/renderer.rs`**

```rust
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
```

- [ ] **Step 4: Write `src/cli.rs`**

Chapter 2 takes only `--width` and `--height`, matching the book's §2 "Creating an Image File". The remaining flags arrive in Task 12.

```rust
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
        Self { width: 256, height: 256 }
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
                    opts.width = v.parse().map_err(|_| {
                        RenderError::InvalidParameter { flag: arg.clone(), value: v.clone() }
                    })?;
                }
                "--height" => {
                    let v = args
                        .next()
                        .ok_or_else(|| RenderError::MissingValue { flag: arg.clone() })?;
                    opts.height = v.parse().map_err(|_| {
                        RenderError::InvalidParameter { flag: arg.clone(), value: v.clone() }
                    })?;
                }
                other => return Err(RenderError::UnknownFlag { flag: other.to_string() }),
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
        assert_eq!(parse(&[]).unwrap(), Options { width: 256, height: 256 });
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
        assert!(matches!(err, RenderError::InvalidDimension { width: 0, .. }));
    }
}
```

- [ ] **Step 5: Write `src/main.rs`**

```rust
use std::path::Path;

use grazing_incidence::cli::{self, Options};
use grazing_incidence::error::RenderError;
use grazing_incidence::renderer;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", cli::USAGE);
        return;
    }

    if let Err(e) = run(args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), RenderError> {
    let opts = Options::parse(args.into_iter())?;
    let image = renderer::render_scene(opts.width, opts.height)?;

    let path = Path::new(cli::DEFAULT_OUTPUT);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    image.write_ppm(path, renderer::GAMMA)?;

    println!("wrote {}", path.display());
    Ok(())
}
```

- [ ] **Step 6: Delete the placeholder scene and build**

```bash
Remove-Item src/scene.rs
```

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests pass.

- [ ] **Step 7: Format**

```bash
cargo fmt
cargo test
```
Expected: all tests pass, zero warnings.

- [ ] **Step 8: Render and commit**

```bash
cargo run --release
```
Expected: `wrote images/rtiow.ppm`

```bash
Move-Item -Force images/rtiow.ppm images/ch02-gradient.ppm
git add -A
git commit -m "feat(ch02): PPM image output (Ray Tracing in One Weekend §2)"
```

---

## Task 2: The vec3 class (§3 The vec3 Class)

**Files:**
- Create: `src/vec3.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `vec3::Vec3 { pub x: f64, pub y: f64, pub z: f64 }`
  - `vec3::Color` = alias for `Vec3`
  - `Vec3::new/zero/unit/dot/cross/length_squared/length/unit/near_zero/clamp/to_rgb/from_rgb`
  - `Vec3::ZERO`, `Vec3::UNIT`
  - Operators: `Add`, `Sub`, `Neg`, `Mul<Vec3>`, `Mul<f64>` (both orders), `Div<f64>`, `AddAssign`, `SubAssign`, `Index<usize>`, `IndexMut<usize>`
  - `vec3::reflect(v, n) -> Vec3`, `vec3::refract(uv, n, eta_ratio) -> Vec3`

**Note on `reflect` and `refract`:** the book introduces `reflect` at §10 and `refract` at §11. They live in `vec3` from chapter 3 because they are pure vector math and testing them early pays off; they go unused until those chapters. No warning, since they are `pub` in a `lib`.

- [ ] **Step 1: Write `src/vec3.rs`**

```rust
//! 3D vectors and colours.

use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, Neg, Sub, SubAssign};

/// A colour is a Vec3 whose components are linear-light intensity in `[0, 1]`.
pub type Color = Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const UNIT: Vec3 = Vec3 { x: 1.0, y: 1.0, z: 1.0 };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// Scale to unit length. A zero vector yields NaN, as in the book.
    pub fn unit(self) -> Self {
        self / self.length()
    }

    /// True when every component is within `1e-8` of zero (§10).
    pub fn near_zero(self) -> bool {
        let s = 1e-8;
        self.x.abs() < s && self.y.abs() < s && self.z.abs() < s
    }

    pub fn clamp(self, min: f64, max: f64) -> Self {
        Self::new(
            self.x.clamp(min, max),
            self.y.clamp(min, max),
            self.z.clamp(min, max),
        )
    }

    pub fn to_rgb(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    pub fn from_rgb(rgb: [f64; 3]) -> Self {
        Self::new(rgb[0], rgb[1], rgb[2])
    }
}

/// Reflect `v` about a unit normal `n` (§10).
pub fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - 2.0 * v.dot(n) * n
}

/// Refract `uv` (unit length, pointing toward the surface) through normal `n`
/// into a medium whose index ratio is `eta_ratio` (§11).
pub fn refract(uv: Vec3, n: Vec3, eta_ratio: f64) -> Vec3 {
    let cos_theta = (-uv).dot(n).min(1.0);
    let r_out_perp = eta_ratio * (uv + cos_theta * n);
    let r_out_parallel = -((1.0 - r_out_perp.length_squared()).abs()).sqrt() * n;
    r_out_perp + r_out_parallel
}

impl Index<usize> for Vec3 {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("index out of bounds for Vec3: {i}"),
        }
    }
}

impl IndexMut<usize> for Vec3 {
    fn index_mut(&mut self, i: usize) -> &mut f64 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!("index out of bounds for Vec3: {i}"),
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<Vec3> for Vec3 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}

impl Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, s: f64) -> Self {
        Self::new(self.x / s, self.y / s, self.z / s)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
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
    fn new_stores_components() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn index_reads_and_writes_components() {
        let mut v = Vec3::ZERO;
        v[1] = 5.0;
        assert_eq!(v[1], 5.0);
    }

    #[test]
    fn arithmetic_operators() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_vec_close(a + b, Vec3::new(5.0, 7.0, 9.0));
        assert_vec_close(b - a, Vec3::new(3.0, 3.0, 3.0));
        assert_vec_close(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
        assert_vec_close(2.0 * a, Vec3::new(2.0, 4.0, 6.0));
        assert_vec_close(a / 2.0, Vec3::new(0.5, 1.0, 1.5));
        assert_vec_close(-a, Vec3::new(-1.0, -2.0, -3.0));
        assert_vec_close(a * b, Vec3::new(4.0, 10.0, 18.0));
    }

    #[test]
    fn assign_operators() {
        let mut a = Vec3::new(1.0, 2.0, 3.0);
        a += Vec3::new(1.0, 1.0, 1.0);
        assert_vec_close(a, Vec3::new(2.0, 3.0, 4.0));
        a -= Vec3::new(2.0, 3.0, 4.0);
        assert_vec_close(a, Vec3::ZERO);
    }

    #[test]
    fn dot_is_symmetric_and_zero_for_orthogonal() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        close(a.dot(b), 32.0);
        close(a.dot(b), b.dot(a));
        close(Vec3::new(1.0, 0.0, 0.0).dot(Vec3::new(0.0, 1.0, 0.0)), 0.0);
    }

    #[test]
    fn cross_follows_the_right_hand_rule() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        assert_vec_close(x.cross(y), Vec3::new(0.0, 0.0, 1.0));
        assert_vec_close(y.cross(x), Vec3::new(0.0, 0.0, -1.0));
        close(x.cross(y).dot(y.cross(x)), -1.0);
    }

    #[test]
    fn length_and_length_squared() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        close(v.length_squared(), 25.0);
        close(v.length(), 5.0);
    }

    #[test]
    fn unit_produces_unit_length() {
        close(Vec3::new(3.0, 4.0, 12.0).unit().length(), 1.0);
    }

    #[test]
    fn near_zero_detects_small_vectors() {
        assert!(Vec3::new(0.0, 1e-10, -1e-10).near_zero());
        assert!(!Vec3::new(0.0, 1e-3, 0.0).near_zero());
    }

    #[test]
    fn clamp_bounds_each_component() {
        assert_vec_close(
            Vec3::new(-1.0, 0.5, 2.0).clamp(0.0, 0.999),
            Vec3::new(0.0, 0.5, 0.999),
        );
    }

    #[test]
    fn rgb_round_trips() {
        let v = Vec3::new(0.1, 0.2, 0.3);
        assert_eq!(v.to_rgb(), [0.1, 0.2, 0.3]);
        assert_vec_close(Vec3::from_rgb([0.1, 0.2, 0.3]), v);
    }

    #[test]
    fn reflect_preserves_length_and_mirrors_about_the_normal() {
        let v = Vec3::new(0.0, -1.0, 0.0);
        let n = Vec3::new(0.0, 1.0, 0.0);
        let r = reflect(v, n);
        assert_vec_close(r, Vec3::new(0.0, 1.0, 0.0));
        close(r.length(), v.length());
    }

    #[test]
    fn refract_entering_glass_bends_toward_the_normal() {
        // 45-degree incidence, air -> glass, eta = 1/1.5.
        let uv = Vec3::new(1.0, -1.0, 0.0).unit();
        let n = Vec3::new(0.0, 1.0, 0.0);
        let t = refract(uv, n, 1.0 / 1.5);

        // Snell: sin(45)/1.5 = sin(theta_t) => theta_t = asin(0.4714).
        let theta_t = (-t.dot(n)).asin().to_degrees();
        close(theta_t, 28.1255);
        close(t.length(), 1.0);
    }

    #[test]
    fn refract_with_equal_indices_is_a_no_op() {
        let uv = Vec3::new(1.0, -1.0, 0.0).unit();
        let n = Vec3::new(0.0, 1.0, 0.0);
        assert_vec_close(refract(uv, n, 1.0), uv);
    }

    #[test]
    fn refract_past_the_critical_angle_stays_finite_and_unit_length() {
        let uv = Vec3::new(1.0, -0.01, 0.0).unit();
        let n = Vec3::new(0.0, 1.0, 0.0);
        let t = refract(uv, n, 1.5);
        assert!(t.x.is_finite() && t.y.is_finite() && t.z.is_finite());
        close(t.length(), 1.0);
    }
}
```

- [ ] **Step 2: Register the module and test**

Add to `src/lib.rs`:

```rust
pub mod vec3;
```

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests pass.

- [ ] **Step 3: Format and commit**

The image is unchanged from chapter 2 — that is the point of this commit.

```bash
cargo fmt
cargo run --release
git add -A
git commit -m "feat(ch03): vec3 class with reflect and refract (Ray Tracing in One Weekend §3)"
```

---

## Task 3: Rays, a simple camera, and background (§4)

**Files:**
- Create: `src/ray.rs`
- Modify: `src/lib.rs`, `src/renderer.rs`

**Interfaces:**
- Consumes: `vec3::Vec3`
- Produces:
  - `ray::Ray { pub origin: Vec3, pub direction: Vec3 }` with `Ray::new(origin, direction)` and `Ray::at(&self, t: f64) -> Vec3`

- [ ] **Step 1: Write `src/ray.rs`**

```rust
//! Rays in linear space.

use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub const fn new(origin: Vec3, direction: Vec3) -> Self {
        Self { origin, direction }
    }

    /// The point at parameter `t` along the ray: `origin + t * direction`.
    pub fn at(&self, t: f64) -> Vec3 {
        self.origin + t * self.direction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_zero_is_the_origin() {
        let r = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(r.at(0.0), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn at_one_is_origin_plus_direction() {
        let r = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(r.at(1.0), Vec3::new(1.0, 3.0, 3.0));
    }

    #[test]
    fn at_two_is_double_the_direction() {
        let r = Ray::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0));
        assert_eq!(r.at(2.0), Vec3::new(2.0, 2.0, 0.0));
    }
}
```

- [ ] **Step 2: Replace `src/renderer.rs` with the sky-gradient render**

```rust
//! The pixel loop.

use crate::error::RenderError;
use crate::image::Image;
use crate::ray::Ray;
use crate::vec3::Vec3;

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

/// The sky: white at the horizon, blue overhead (§4).
fn sky_color(direction: Vec3) -> Vec3 {
    let unit_direction = direction.unit();
    let a = 0.5 * (unit_direction.y + 1.0);
    Vec3::UNIT * (1.0 - a) + Vec3::new(0.5, 0.7, 1.0) * a
}

pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
    let mut image = Image::new(width, height)?;

    let viewport_height = 2.0;
    let viewport_width = viewport_height * (width as f64 / height as f64);

    let origin = Vec3::ZERO;
    let horizontal = Vec3::new(viewport_width, 0.0, 0.0);
    let vertical = Vec3::new(0.0, viewport_height, 0.0);
    let lower_left_corner =
        origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new(0.0, 0.0, 1.0);

    for j in 0..height {
        eprint!("\rScanlines remaining: {} ", height - j);
        for i in 0..width {
            let u = i as f64 / (width - 1) as f64;
            let v = j as f64 / (height - 1) as f64;
            let direction = lower_left_corner + horizontal * u + vertical * v - origin;
            image.set(i, j, sky_color(Ray::new(origin, direction).direction).to_rgb());
        }
    }
    eprintln!("\rDone.                 ");

    Ok(image)
}
```

- [ ] **Step 3: Register `ray` and test**

Add to `src/lib.rs`:

```rust
pub mod ray;
```

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests pass.

- [ ] **Step 4: Format, render, and commit**

```bash
cargo fmt
cargo run --release
```
Expected: `wrote images/rtiow.ppm`, and the image is now a blue-to-white sky.

```bash
git add -A
git commit -m "feat(ch04): rays, camera and sky gradient (Ray Tracing in One Weekend §4)"
```

---

## Task 4: Adding a sphere (§5 Adding a Sphere)

**Files:**
- Create: `src/sphere.rs`
- Modify: `src/lib.rs`, `src/renderer.rs`

**Interfaces:**
- Consumes: `ray::Ray`, `vec3::Vec3`, `image::Image`, `error::RenderError`
- Produces:
  - `sphere::Sphere { pub center: Vec3, pub radius: f64 }` with `Sphere::new(center, radius)`
  - `Sphere::hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<f64>` — returns only the hit parameter; §6 grows this into a full record

- [ ] **Step 1: Write `src/sphere.rs`**

```rust
//! Spheres.

use crate::ray::Ray;
use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f64,
}

impl Sphere {
    pub const fn new(center: Vec3, radius: f64) -> Self {
        Self { center, radius }
    }

    /// Nearest intersection with parameter in `[t_min, t_max]`, if any (§5).
    ///
    /// Uses the book's full quadratic coefficients. §6.2 simplifies these to
    /// the half-b form.
    pub fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<f64> {
        let oc = ray.origin - self.center;

        let a = ray.direction.dot(ray.direction);
        let b = 2.0 * oc.dot(ray.direction);
        let c = oc.dot(oc) - self.radius * self.radius;

        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return None;
        }
        let sqrtd = discriminant.sqrt();

        let mut t = (-b - sqrtd) / (2.0 * a);
        if t < t_min || t > t_max {
            t = (-b + sqrtd) / (2.0 * a);
            if t < t_min || t > t_max {
                return None;
            }
        }
        Some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    #[test]
    fn hits_the_book_example_exactly() {
        // The book's worked example: ray (-11,0,0) + t*(1,0,0) at a unit sphere.
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(sphere.hit(&ray, 0.001, f64::INFINITY), Some(10.5));
    }

    #[test]
    fn misses_when_aimed_away() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(sphere.hit(&ray, 0.001, f64::INFINITY), None);
    }

    #[test]
    fn misses_when_offset_beyond_the_radius() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(sphere.hit(&ray, 0.001, f64::INFINITY), None);
    }

    #[test]
    fn respects_t_max() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        // Roots are 10.5 and 11.5. A t_max of 11.0 rejects the near root and
        // the far root is beyond t_max too, so the whole hit is rejected.
        assert_eq!(sphere.hit(&ray, 0.001, 11.0), None);
        let t = sphere.hit(&ray, 0.001, 12.0).unwrap();
        close(t, 10.5);
    }

    #[test]
    fn ray_starting_inside_hits_the_far_side() {
        let sphere = Sphere::new(Vec3::ZERO, 5.0);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        close(sphere.hit(&ray, 0.001, f64::INFINITY).unwrap(), 5.0);
    }

    #[test]
    fn hit_point_lies_on_the_surface() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let t = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        close(ray.at(t).length(), sphere.radius);
    }
}
```

- [ ] **Step 2: Register and extend the renderer**

Add to `src/lib.rs`:

```rust
pub mod sphere;
```

In `src/renderer.rs`, replace `render_scene`:

```rust
/// The book sphere and the sky behind it (§5).
fn render_sphere(width: usize, height: usize) -> Result<Image, RenderError> {
    let mut image = Image::new(width, height)?;

    let viewport_height = 2.0;
    let viewport_width = viewport_height * (width as f64 / height as f64);

    let origin = Vec3::ZERO;
    let horizontal = Vec3::new(viewport_width, 0.0, 0.0);
    let vertical = Vec3::new(0.0, viewport_height, 0.0);
    let lower_left_corner =
        origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new(0.0, 0.0, 1.0);

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
                    Vec3::new(1.0, 0.2, 0.2)
                        * (0.5 * (normal.dot(Vec3::new(0.0, 0.0, -1.0)) + 1.0))
                }
                None => sky_color(ray.direction),
            };
            image.set(i, j, color.to_rgb());
        }
    }
    eprintln!("\rDone.                 ");

    Ok(image)
}

pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
    render_sphere(width, height)
}
```

Add `use crate::sphere::Sphere;` to `src/renderer.rs`.

- [ ] **Step 3: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests pass.

- [ ] **Step 4: Format, render, and commit**

```bash
cargo fmt
cargo run --release
```
Expected: a shaded red sphere against the sky.

```bash
git add -A
git commit -m "feat(ch05): ray-sphere intersection (Ray Tracing in One Weekend §5)"
```

---

## Task 5: Surface normals and multiple objects (§6)

**Files:**
- Create: `src/interval.rs`, `src/hittable.rs`, `src/scene.rs`
- Modify: `src/sphere.rs` (impl `Hittable`), `src/renderer.rs`, `src/lib.rs`

**Interfaces:**
- Consumes: `ray::Ray`, `vec3::Vec3`
- Produces:
  - `interval::Interval { pub min: f64, pub max: f64 }` with `Interval::new`, `Interval::EMPTY`, `contains(f64) -> bool`, `pad(f64) -> Interval`
  - `hittable::HitRecord { pub t: f64, pub point: Vec3, pub normal: Vec3, pub front_face: bool }`
  - `hittable::Hittable` trait: `fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord>`
  - `hittable::HittableList<'a>` with `new()`, `add(&'a dyn Hittable)`, and a `Hittable` impl
  - `scene::Scene` enum (`Spheres`) and `scene::build(scene: Scene) -> Vec<Box<dyn Hittable>>`

**Note:** `Scene::build` returns owned boxes so callers own the objects while `HittableList` borrows them. This is what lets the crate stay free of `Rc`.

- [ ] **Step 1: Write `src/interval.rs`**

```rust
//! Numeric ranges.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub min: f64,
    pub max: f64,
}

impl Interval {
    pub const EMPTY: Interval = Interval {
        min: f64::INFINITY,
        max: f64::NEG_INFINITY,
    };

    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// True when `x` lies within the interval, endpoints included (§6.8).
    pub fn contains(&self, x: f64) -> bool {
        self.min <= x && x <= self.max
    }

    /// Widen by `pad` on both sides. Padding an empty interval keeps it empty
    /// rather than producing a real range.
    pub fn pad(&self, pad: f64) -> Interval {
        if self.min > self.max {
            return Interval::EMPTY;
        }
        Interval::new(self.min - pad, self.max + pad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_includes_both_endpoints() {
        let i = Interval::new(0.0, 1.0);
        assert!(i.contains(0.0));
        assert!(i.contains(1.0));
        assert!(i.contains(0.5));
    }

    #[test]
    fn contains_excludes_values_outside() {
        let i = Interval::new(0.0, 1.0);
        assert!(!i.contains(-0.001));
        assert!(!i.contains(1.001));
    }

    #[test]
    fn pad_widens_both_sides() {
        let p = Interval::new(0.0, 1.0).pad(0.1);
        assert!((p.min + 0.1).abs() < 1e-9);
        assert!((p.max - 0.1).abs() < 1e-9);
    }

    #[test]
    fn padding_empty_stays_empty() {
        assert!(Interval::EMPTY.pad(1.0).min > Interval::EMPTY.max);
    }
}
```

- [ ] **Step 2: Write `src/hittable.rs`**

```rust
//! The intersection abstraction.

use crate::ray::Ray;
use crate::vec3::Vec3;

/// Everything known about one ray-surface intersection (§6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRecord {
    pub t: f64,
    pub point: Vec3,
    pub normal: Vec3,
    pub front_face: bool,
}

/// Anything a ray can intersect (§6.3).
pub trait Hittable {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord>;
}

/// An ordered set of hittables; the closest hit wins (§6.5).
#[derive(Default)]
pub struct HittableList<'a> {
    objects: Vec<&'a dyn Hittable>,
}

impl<'a> HittableList<'a> {
    pub fn new() -> Self {
        Self { objects: Vec::new() }
    }

    pub fn add(&mut self, object: &'a dyn Hittable) {
        self.objects.push(object);
    }
}

impl Hittable for HittableList<'_> {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        let mut closest = t_max;
        let mut hit = None;

        for object in &self.objects {
            if let Some(record) = object.hit(ray, t_min, closest) {
                closest = record.t;
                hit = Some(record);
            }
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere::Sphere;

    #[test]
    fn empty_list_hits_nothing() {
        let world = HittableList::new();
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert!(world.hit(&ray, 0.001, f64::INFINITY).is_none());
    }

    #[test]
    fn list_returns_the_closest_of_two() {
        let far = Sphere::new(Vec3::ZERO, 0.5);
        let near = Sphere::new(Vec3::new(-5.0, 0.0, 0.0), 0.5);
        let mut world = HittableList::new();
        world.add(&far);
        world.add(&near);

        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let rec = world.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert_eq!(rec.t, 5.5);
    }

    #[test]
    fn front_face_is_true_when_the_ray_faces_the_normal() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let rec = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(rec.front_face);
        // The outward normal at the entry point points back along the ray.
        assert!((rec.normal.x + 1.0).abs() < 1e-9);
    }

    #[test]
    fn back_face_normal_flips_and_front_face_is_false() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        let rec = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(!rec.front_face);
        // The normal always opposes the incoming ray.
        assert!(rec.normal.dot(ray.direction) < 0.0);
    }

    #[test]
    fn hit_point_matches_the_ray_parameter() {
        let sphere = Sphere::new(Vec3::new(0.0, 0.0, -1.0), 0.5);
        let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
        let rec = sphere.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert_eq!(rec.point, ray.at(rec.t));
    }
}
```

- [ ] **Step 3: Rewrite `src/sphere.rs` to implement `Hittable`, with the §6.2 simplification**

Replace the whole file:

```rust
//! Spheres.

use crate::hittable::{HitRecord, Hittable};
use crate::ray::Ray;
use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f64,
}

impl Sphere {
    pub const fn new(center: Vec3, radius: f64) -> Self {
        Self { center, radius }
    }
}

impl Hittable for Sphere {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        // §6.2: divide the quadratic through by 4 and track half of b, so the
        // discriminant is half_b^2 - a*c instead of b^2 - 4*a*c.
        let oc = ray.origin - self.center;
        let a = ray.direction.dot(ray.direction);
        let half_b = oc.dot(ray.direction);
        let c = oc.dot(oc) - self.radius * self.radius;

        let discriminant = half_b * half_b - a * c;
        if discriminant < 0.0 {
            return None;
        }
        let sqrtd = discriminant.sqrt();

        let mut t = (-half_b - sqrtd) / a;
        if t < t_min || t > t_max {
            t = (-half_b + sqrtd) / a;
            if t < t_min || t > t_max {
                return None;
            }
        }

        let point = ray.at(t);
        let outward = (point - self.center) / self.radius;

        // §6.4: the normal always opposes the incoming ray, whichever face.
        let front_face = ray.direction.dot(outward) < 0.0;
        let normal = if front_face { outward } else { -outward };

        Some(HitRecord { t, point, normal, front_face })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    #[test]
    fn hits_the_book_example_exactly() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(sphere.hit(&ray, 0.001, f64::INFINITY).unwrap().t, 10.5);
    }

    #[test]
    fn misses_when_aimed_away() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0));
        assert!(sphere.hit(&ray, 0.001, f64::INFINITY).is_none());
    }

    #[test]
    fn misses_when_offset_beyond_the_radius() {
        let sphere = Sphere::new(Vec3::ZERO, 0.5);
        let ray = Ray::new(Vec3::new(-11.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert!(sphere.hit(&ray, 0.001, f64::INFINITY).is_none());
    }

    #[test]
    fn ray_starting_inside_hits_the_far_side() {
        let sphere = Sphere::new(Vec3::ZERO, 5.0);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        close(sphere.hit(&ray, 0.001, f64::INFINITY).unwrap().t, 5.0);
    }

    #[test]
    fn half_b_form_agrees_with_the_books_full_quadratic() {
        // The book's own worked numbers through both formulations.
        let sphere = Sphere::new(Vec3::new(0.0, 0.0, -1.0), 0.5);
        let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));

        let oc = ray.origin - sphere.center;
        let a = ray.direction.dot(ray.direction);
        let half_b = oc.dot(ray.direction);
        let c = oc.dot(oc) - sphere.radius * sphere.radius;

        // full form: b = 2*half_b, so b^2 - 4ac == 4*(half_b^2 - a*c)
        let b = 2.0 * half_b;
        close(b * b - 4.0 * a * c, 4.0 * (half_b * half_b - a * c));

        close(sphere.hit(&ray, 0.001, f64::INFINITY).unwrap().t, 0.5);
    }
}
```

**Important:** `hit` moved from an inherent method to a trait method. Every call site now needs `use crate::hittable::Hittable;` in scope. `sphere.hit(&ray, t_min, t_max)` still resolves once the trait is in scope.

- [ ] **Step 4: Write `src/scene.rs`**

```rust
//! Named scenes.

use crate::hittable::Hittable;
use crate::sphere::Sphere;
use crate::vec3::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// The ground sphere plus one ball (§6).
    Spheres,
}

/// Build the objects for a scene. Returns owned boxes so the caller can keep
/// them alive while `HittableList` borrows them.
pub fn build(scene: Scene) -> Vec<Box<dyn Hittable>> {
    match scene {
        Scene::Spheres => {
            let ground = Sphere::new(Vec3::new(0.0, -1000.0, 0.0), 1000.0);
            let ball = Sphere::new(Vec3::new(0.0, 0.5, 0.0), 0.5);
            vec![Box::new(ground), Box::new(ball)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hittable::HittableList;
    use crate::ray::Ray;

    #[test]
    fn spheres_scene_contains_two_objects() {
        assert_eq!(build(Scene::Spheres).len(), 2);
    }

    #[test]
    fn spheres_scene_hits_the_ball() {
        let objects = build(Scene::Spheres);
        let mut world = HittableList::new();
        for o in &objects {
            world.add(o.as_ref());
        }

        // A ray straight down from above the ball must hit it.
        let ray = Ray::new(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let rec = world.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(rec.t < 5.0);
    }
}
```

- [ ] **Step 5: Register modules and rewire the renderer**

Add to `src/lib.rs`:

```rust
pub mod hittable;
pub mod interval;
pub mod scene;
```

Replace `src/renderer.rs` in full:

```rust
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

pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
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
    let lower_left_corner =
        origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new(0.0, 0.0, 1.0);

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
            image.set(i, j, color.to_rgb());
        }
    }
    eprintln!("\rDone.                 ");

    Ok(image)
}
```

- [ ] **Step 6: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests pass.

- [ ] **Step 7: Format, render, and commit**

```bash
cargo fmt
cargo run --release
```
Expected: a large ground sphere and a small ball, both shaded by surface normal.

```bash
git add -A
git commit -m "feat(ch06): hittable abstraction, interval, normals, scene list (Ray Tracing in One Weekend §6)"
```

---

## Task 6: Camera in its own class (§7)

**Files:**
- Create: `src/camera.rs`
- Modify: `src/lib.rs`, `src/renderer.rs`

**Interfaces:**
- Consumes: `hittable::Hittable`, `ray::Ray`, `vec3::Vec3`, `image::Image`
- Produces:
  - `camera::Camera { pub image_width: usize, pub image_height: usize, pub samples_per_pixel: usize, pub max_depth: usize }` with `Camera::new(width, height, samples, depth)` and `Camera::render(&self, world: &dyn Hittable) -> Result<Image, RenderError>`

**This commit is a pure refactor.** The rendered image must be visually identical to Task 5. If it differs, the extraction changed behaviour — fix it before committing.

- [ ] **Step 1: Write `src/camera.rs`**

```rust
//! The camera and the recursive shading loop.

use crate::error::RenderError;
use crate::hittable::Hittable;
use crate::image::Image;
use crate::ray::Ray;
use crate::vec3::Vec3;

/// A pinhole camera (§7). Position and orientation become settable in §12.
pub struct Camera {
    pub image_width: usize,
    pub image_height: usize,
    pub samples_per_pixel: usize,
    pub max_depth: usize,

    origin: Vec3,
    horizontal: Vec3,
    vertical: Vec3,
    lower_left_corner: Vec3,
}

impl Camera {
    pub fn new(
        image_width: usize,
        image_height: usize,
        samples_per_pixel: usize,
        max_depth: usize,
    ) -> Self {
        let viewport_height = 2.0;
        let viewport_width = viewport_height * (image_width as f64 / image_height as f64);
        let origin = Vec3::ZERO;
        let horizontal = Vec3::new(viewport_width, 0.0, 0.0);
        let vertical = Vec3::new(0.0, viewport_height, 0.0);
        let lower_left_corner =
            origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new(0.0, 0.0, 1.0);

        Self {
            image_width,
            image_height,
            samples_per_pixel,
            max_depth,
            origin,
            horizontal,
            vertical,
            lower_left_corner,
        }
    }

    pub fn render(&self, world: &dyn Hittable) -> Result<Image, RenderError> {
        let mut image = Image::new(self.image_width, self.image_height)?;

        for j in 0..self.image_height {
            eprint!("\rScanlines remaining: {} ", self.image_height - j);
            for i in 0..self.image_width {
                let mut color = Vec3::ZERO;
                for _ in 0..self.samples_per_pixel {
                    let u = i as f64 / (self.image_width - 1) as f64;
                    let v = j as f64 / (self.image_height - 1) as f64;
                    let ray = Ray::new(
                        self.origin,
                        self.lower_left_corner
                            + self.horizontal * u
                            + self.vertical * v
                            - self.origin,
                    );
                    color += self.ray_color(&ray, self.max_depth, world);
                }
                image.set(i, j, (color / self.samples_per_pixel as f64).to_rgb());
            }
        }
        eprintln!("\rDone.                 ");

        Ok(image)
    }

    fn ray_color(&self, ray: &Ray, depth: usize, world: &dyn Hittable) -> Vec3 {
        if depth == 0 {
            return Vec3::ZERO;
        }

        if let Some(rec) = world.hit(ray, 0.001, f64::INFINITY) {
            // Until §9, shading is a fixed bounce rather than a material.
            let scattered = Ray::new(rec.point, rec.normal);
            return 0.5 * self.ray_color(&scattered, depth - 1, world);
        }

        let unit_direction = ray.direction.unit();
        let a = 0.5 * (unit_direction.y + 1.0);
        Vec3::UNIT * (1.0 - a) + Vec3::new(0.5, 0.7, 1.0) * a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hittable::HittableList;
    use crate::scene::{self, Scene};

    fn test_world() -> (Vec<Box<dyn Hittable>>, HittableList<'_>) {
        let objects = scene::build(Scene::Spheres);
        let mut list = HittableList::new();
        for o in &objects {
            list.add(o.as_ref());
        }
        (objects, list)
    }

    #[test]
    fn render_produces_the_requested_pixel_count() {
        let (_objects, world) = test_world();
        let camera = Camera::new(20, 12, 1, 2);
        let image = camera.render(&world).unwrap();
        assert_eq!(image.pixels.len(), 20 * 12);
        assert_eq!(image.width, 20);
        assert_eq!(image.height, 12);
    }

    #[test]
    fn max_depth_zero_yields_black() {
        let (_objects, world) = test_world();
        let camera = Camera::new(8, 5, 1, 0);
        let image = camera.render(&world).unwrap();
        // A zero-depth camera absorbs every ray, so the image is entirely black.
        assert!(image.pixels.iter().all(|p| p == &[0.0, 0.0, 0.0]));
    }
}
```

**Note on `samples_per_pixel`:** at this chapter it multiplies an *identical* ray, so it changes nothing. It exists because §8 gives each sample its own jittered ray. Leaving it at 1 keeps the image identical to Task 5, which is what makes this commit a verified no-op. Do not set it above 1 yet.

- [ ] **Step 2: Slim `renderer.rs` down to a shim**

Replace `src/renderer.rs` in full:

```rust
//! Rendering entry point.

use crate::camera::Camera;
use crate::error::RenderError;
use crate::hittable::{Hittable, HittableList};
use crate::image::Image;
use crate::scene::{self, Scene};

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
    let objects = scene::build(Scene::Spheres);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    let camera = Camera::new(width, height, 1, 5);
    camera.render(&world)
}
```

The private `sky_color` and `shade` helpers move into `camera.rs` — `ray_color` now owns both.

- [ ] **Step 3: Register and test**

Add to `src/lib.rs`:

```rust
pub mod camera;
```

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests pass.

- [ ] **Step 4: Verify the render is unchanged from Task 5**

```bash
cargo fmt
cargo run --release
```

Compare `images/rtiow.ppm` against the Task 5 artifact. The two must be the same picture. If you kept the Task 5 file, diff them:

```bash
(Get-FileHash images/rtiow.ppm -Algorithm SHA256).Hash
```

If they differ, the extraction changed behaviour. Find and fix it before committing.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(ch07): move camera and shading loop into its own class (Ray Tracing in One Weekend §7)"
```