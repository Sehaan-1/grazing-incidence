# grazing-incidence — Design

**Date:** 2026-10-10
**Status:** Approved
**Subject:** Port *Ray Tracing in One Weekend* (v4.0.2) to Rust, chapter by chapter.

## Context

The working directory held a single file: a local copy of *Ray Tracing in One
Weekend* v4.0.2, scraped from <https://raytracing.github.io/books/RayTracingInOneWeekend.html>
to markdown (`raytracing-github-io-books-RayTracingInOneWeekend-html.md`).

That file is copyrighted material and is **gitignored** — it stays a local
reference and is never committed. It is the input to this project, not part of
it. The repository therefore does not contain the book, and this spec cites it
by section number (`Ray Tracing in One Weekend` §6.3) rather than reproducing
its text or code.

The book is C++-only. This project is a Rust port, chosen by the project owner.

## Goals

1. Build a working ray tracer by porting the book chapter by chapter, with each
   chapter landing as one commit that compiles, passes tests, and renders an
   image.
2. Use Rust idioms where they genuinely simplify the book's C++ idioms, and stay
   faithful to the book's mathematics everywhere else.
3. Ship a public repository whose commit history is the pedagogical progression
   and whose images are viewable inline on GitHub.
4. Verify the mathematics with unit tests rather than by eyeballing renders.

## Non-goals

- Book 2 (*The Next Week*: quads, textures, motion blur, transforms, lights,
  volumetric rendering) and Book 3. Out of scope.
- Parallel/multithreaded rendering. Out of scope.
- A usable general-purpose renderer library. Out of scope.
- Committing the book, book code snippets, or book images.

## Key decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | Rust | Project owner's choice. The port is the work; the language is a deliberate upgrade over the book's C++. |
| 2 | Single crate, `lib` + `bin` | The book's single `main.cpp` reaches ~800 lines. Splitting `lib.rs` from `main.rs` lets `cargo test` cover the whole renderer, which is what makes unit-test verification cheap. |
| 3 | One evolving program, one commit per chapter | Mirrors how the book actually teaches — one program that grows. Avoids the 13 dead binaries a per-chapter-binaries layout would create. Git already provides replayability. |
| 4 | Zero dependencies | A ray tracer needs no crates. PRNG, PPM/PNG writing, argument parsing and error types are all hand-rolled, which is also the most educational option. |
| 5 | `trait Hittable`; `Material` as an enum | See "Rust translations" below. The enum removes the book's `shared_ptr<material>` machinery entirely. |
| 6 | Unit tests on mathematics; no golden images | The book has no stable output to pin, and golden PPMs would be brittle under any refactor. Committed images are visual artifacts, not fixtures. |
| 7 | Deterministic by default | Fixed PRNG seed, overridable via `--seed`, so identical code yields byte-identical images and a diff in `images/` always means a real change. |
| 8 | PNG with hand-rolled deflate | Committed PPMs are ~336 KB and GitHub will not preview them. A hand-written LZ77 + fixed-Huffman deflate gives ~50 KB files that render inline. Accepted as scope beyond what the book teaches. |

## Environment

- Host: Windows, `x86_64-pc-windows-gnu`.
- Rust installed via `rustup-init.exe --default-host x86_64-pc-windows-gnu`,
  giving stable `x86_64-pc-windows-gnu` (rustc/cargo 1.99.0).
- **No MSVC** on this machine (`cl.exe` absent), which is why the GNU host was
  chosen. The project itself is host-agnostic and will build on MSVC and Linux.
- `rust-toolchain.toml` pins `channel = "stable"` with `profile = "minimal"`.
  It deliberately does **not** pin a target, so Linux CI does not download a
  Windows target.

## Repository layout

```
grazing-incidence/
├── .gitignore
├── Cargo.toml
├── rust-toolchain.toml
├── README.md
├── docs/
│   └── superpowers/specs/
│       └── 2026-10-10-grazing-incidence-design.md
├── images/                      # committed output, one file per chapter
└── src/
    ├── lib.rs                   # renderer modules
    ├── renderer.rs              # pixel loop + progress indicator
    ├── scene.rs                 # named scenes
    └── ...                      # modules appear as chapters land
```

`images/` is deliberately **not** gitignored. Those files are the visual record
of the progression.

## Module layout

The book's single translation unit is split so each file stays independently
readable.

| Module | Lands | Contains |
|---|---|---|
| `image` | ch 2 | PPM writer; PNG writer from ch 14 |
| `vec3` | ch 3 | `Vec3`, colour utilities |
| `ray` | ch 4 | `Ray` |
| `sphere` | ch 5 | `Sphere` |
| `hittable` | ch 6 | `trait Hittable`, `HitRecord`, `HittableList` |
| `interval` | ch 6 | `Interval` |
| `camera` | ch 7 | `Camera` (rewritten in ch 12, gains aperture in ch 13) |
| `rng` | ch 8 | PCG32 |
| `material` | ch 8–10 | `Material` enum |
| `renderer` | ch 2 | the pixel loop, progress indicator |
| `scene` | ch 5 | named scenes |

## Rust translations from the book's C++

### `Hittable`: abstract class → trait

```rust
pub trait Hittable {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord>;
}
```

The book uses a boolean return plus out-parameters (`hit(..., &hit)`). Returning
`Option<HitRecord>` removes that idiom, which is the book's most error-prone
pattern. `HittableList` holds `Vec<&dyn Hittable>`; the borrow, not a
`shared_ptr`, owns the referents.

### `Material`: class hierarchy → enum

```rust
pub enum Material {
    Lambertian { albedo: Vec3 },
    Metal { albedo: Vec3, fuzz: f64 },
    Dielectric { ior: f64 },
}
```

The book builds `lambertian`, `metal` and `dielectric` behind a
`shared_ptr<material>` base class. An enum makes the material owned, movable and
exhaustively matched, and keeps `HitRecord` free of lifetime parameters and
reference-counting churn.

Variants are added as their chapters arrive — `Lambertian` in chapter 9,
`Metal` in 10, `Dielectric` in 11 — so every commit compiles.

`scatter` returns `Option<(Ray, Vec3)>` from the moment `Material` exists, even
though chapter 9 does not yet need the attenuation. This is a deliberate
divergence from the book, whose `lambertian::scatter` returns only a `ray`;
adopting the two-tuple later would mean a breaking signature change in chapter
10 for no gain.

### PPM: P3 in the book, P6 here

The book's text writes **P3**, plain-ASCII PPM. At 400×225 that is roughly 270,000
space-separated numbers, about 1.1 MB of text per image. This project writes
**P6**, the binary form of the same format: identical image, 270,000 bytes, and
one `write` call instead of per-channel formatting. The book's own released
`inOneWeekend` program also emits P6. The header test in the testing strategy
asserts `P6` for this reason.

### Colour

`type Color = Vec3;` keeps all colour arithmetic on `Vec3`, matching the book's
maths exactly. A thin `Color` newtype is reserved for *output encoding only*
(gamma correction and quantisation), giving that logic one obvious home.

### Errors

A hand-rolled `RenderError` enum implementing `std::error::Error`, since there
are no dependencies. Where the book throws C++ exceptions, this project returns
`Result`. `main` prints the error and exits non-zero rather than panicking.

## Chapter → commit mapping

The book has 14 numbered chapters; 1 ("Overview") and 14 ("Where Next?") are
prose. That leaves 12 code chapters, plus one of ours.

| Commit | Book chapter | New module(s) | Rendered |
|---|---|---|---|
| 0 | — | scaffolding, spec, README | none |
| 1 | §2 Output an Image | `image` (PPM), `renderer` | gradient PPM |
| 2 | §3 The vec3 Class | `vec3` | unchanged image |
| 3 | §4 Rays, Camera, Background | `ray` | sky gradient |
| 4 | §5 Adding a Sphere | `sphere` | one sphere on sky |
| 5 | §6 Normals & Multiple Objects | `hittable`, `interval` | two ground spheres + ball |
| 6 | §7 Camera Into Its Own Class | `camera`, `HitRecord` | same scene, reframed |
| 7 | §8 Antialiasing | `rng` | smoothed edges |
| 8 | §9 Diffuse Materials | `Material::Lambertian` | shadows + gamma |
| 9 | §10 Metal | `Material::Metal` | reflective spheres |
| 10 | §11 Dielectrics | `Material::Dielectric` | glass with refraction |
| 11 | §12 Positionable Camera | `camera` rewrite | reframed scene |
| 12 | §13 Defocus Blur | `camera` aperture | final book image |
| 13 | — (ours) PNG Output | `image` (deflate) | same image as `.png` |

Each commit is gated on `cargo test` passing and its message cites the book
section. Chapters 3 and 7 render a visually identical image to their
predecessor by design — the refactor is isolated from any visual change, which
is exactly what makes them reviewable.

Commit 0 carries placeholder `renderer.rs` and `scene.rs` modules so the crate
compiles before chapter 2 replaces them. `images/` is created empty at commit 0
and filled from commit 1 onward.

## Testing strategy

`#[cfg(test)] mod tests` inside each module. No integration-test directory: the
book exposes no seams worth testing from outside.

- **`vec3`** — operators; `length`; `normalize` yields unit length; `dot` and
  `cross` identities; `reflect` preserves length and flips the normal component;
  `refract` checked against a hand-computed Snell's-law case and against
  total internal reflection; `random_unit_vector` lands on the unit sphere over
  many samples.
- **`interval`** — `contains` at both endpoints; `pad` widens without crossing;
  the empty-interval guard returns `Interval::EMPTY`.
- **`sphere`** — a ray with a known answer returns the **exact expected `t`**; a
  ray pointing away from the sphere returns `None`; a negative discriminant
  returns `None`; a ray originating inside the sphere returns the near root.
- **`material`** — `Metal` fuzz is clamped to `[0, 1]`; `Dielectric` with
  `ior = 1.0` returns an unchanged direction.
- **`image`** — PPM output has a correct `P6` header followed by exactly
  `width * height * 3` bytes; PNG output round-trips back to the same pixel
  count as its input buffer.
- **`renderer`** — returns exactly `width * height` pixels.

Floating-point comparisons use an explicit epsilon. Exact equality is asserted
only where it is exactly representable (the `t = 2.0` sphere case above).

## CLI contract

Argument parsing arrives in two stages, mirroring what the book itself can
motivate at each point:

- **Chapter 2** parses `--width` and `--height` only, exactly as the book's
  §2 "Creating an Image File" does. Output path is hardcoded.
- **Chapter 13** completes the CLI with `--samples`, `--seed`, `--scene`,
  `--out` and `--help`, once every one of them has a meaning.

The binary is `grazing-incidence`, the name of the Cargo package. (`rtiow` is
the placeholder name the book uses and is **not** used here.) Argument parsing is
hand-rolled; there is no `clap`.

```
grazing-incidence [--width N] [--height N] [--samples N] [--seed N]
                  [--out PATH] [--scene NAME] [--help]
```

- Defaults are the book's chapter-13 image: **400×225, 100 samples per pixel,
  max depth 50**, fixed default seed, `--scene defocus`.
- `--scene cover` is the book's full final render (Listing 88): **1200×675, 500
  samples**. This is many hours of CPU time and is never produced by default.
- Unknown flags are a hard error, never silently ignored.
- Invalid dimensions or a non-positive sample count return `Err`, exit non-zero.
- `--out` infers the encoder from the extension: `.ppm` → PPM, `.png` → PNG.
  Requesting `.png` before chapter 14 has landed returns `Err`, because the PNG
  encoder does not exist yet.
- `--scene` accepts only scene names defined in `scene.rs`; an unknown name is
  an error listing the valid names.

### Image dimensions per chapter

The book's own numbers, used verbatim. Chapters 2–3 are 256×256; every chapter
from 4 onward is 16:9 at width 400, which the camera rounds to 400×225.

| Chapters | Width × height | Aspect | Notes |
|---|---|---|---|
| 2–3 | 256 × 256 | 1:1 | first images; no camera class yet |
| 4–11 | 400 × 225 | 16:9 | samples per pixel grows 10 → 100 |
| 12–13 | 400 × 225 | 16:9 | camera gains position, then aperture |
| 14 (ours) | 400 × 225 | 16:9 | same image, PNG instead of PPM |
| `cover` scene | 1200 × 675 | 16:9 | 500 spp, hours of CPU time |

## Verification gate per chapter

Before each commit:

1. `cargo build` is clean (no warnings).
2. `cargo test` is green.
3. The chapter's image is rendered and committed to `images/`. Chapters 2–12
   render to the hardcoded path `images/<chapter>.ppm`; from chapter 13 onward,
   `cargo run --release -- --scene <name> --out images/<chapter>.png`.
4. The image is opened and inspected.

Step 4 is a human check and is the point of the exercise; steps 1–3 are
mechanical.

## Out of scope

Book 2 and Book 3 features; parallel rendering; a reusable library API; any
redistribution of the book's text, code, or images.