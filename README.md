# grazing-incidence

A ray tracer built by porting [*Ray Tracing in One
Weekend*](https://raytracing.github.io/books/RayTracingInOneWeekend.html)
(v4.0.2, Peter Shirley) from C++ to Rust — one chapter per commit.

![The current render: the three-sphere scene at §13's framing, with thin-lens defocus](images/final-spheres.png)

*`images/final-spheres.png` — the current renderer, 400×225 at 100 samples per
pixel, the `--scene spheres` default. §13's 20-degree lens from (-2, 2, 1)
behind a 10-degree aperture focused at 3.4, so the middle ball is sharp and the
ground and the two outer balls smear. Rendered by
`cargo run --release -- --out images/final-spheres.png`.*

## Status

All 15 commits are in, and the renderer runs.

| | |
|---|---|
| Chapters landed | 15 of 15 |
| Tests | 166, all passing |
| Dependencies | none, by design |
| Scenes | `spheres` (§6–§13), `cover` (§14) |

See [`docs/superpowers/specs/`](docs/superpowers/specs/) for the full design
and the chapter-to-commit mapping.

## What the renderer does

One evolving program, in the book's order:

- **§2–§4** — writes an image file, a `Vec3`, a `Ray`, and a camera that
  produces a sky gradient.
- **§5–§7** — `Sphere`, `Hittable`, `HitRecord` and `Interval`; the camera
  moves into its own class with a real field of view.
- **§8** — antialiasing by jittered multi-sample averaging over a hand-rolled
  PCG32, so a render is reproducible from `--seed` alone.
- **§9–§11** — Lambertian diffuse with shadow-acne avoidance and gamma
  correction, fuzzy metal, and dielectric glass with Schlick's reflectance.
- **§12–§13** — a positionable camera (`lookfrom`/`lookat`/`vup`) and a thin
  lens: `focus_dist` and `defocus_angle` replace the pinhole.
- **§14** — the book's final image, a field of random spheres, plus a
  hand-rolled PNG encoder (LZ77 + fixed-Huffman DEFLATE) so renders are
  previewable on GitHub.

### Scenes

| Scene | What it is |
|---|---|
| `spheres` | The ground, a matte ball at `(0, 0, -1.2)`, a glass ball at `(-1, 0, -1)` with a hollow glass bubble of radius 0.4 inside it, and a metal ball at `(1, 0, -1)`. |
| `cover` | The §14 listing: a gap-cleared grid of ~484 random spheres on a flat ground, plus three deliberate foreground balls. |

```sh
# the default image
cargo run --release -- --out images/out.png

# the cover, small. The book's own 1200x675 at 500 samples is 405 million
# primary rays against ~488 spheres; start small.
cargo run --release -- --scene cover --width 200 --samples 8 --out images/cover-probe.png
```

![The cover scene at 200×112 and 8 samples per pixel: the grid of random spheres and the three foreground balls](images/cover-probe.png)

*`images/cover-probe.png` — `cover` at 200×112 and 8 samples per pixel. Noisy
on purpose: it is evidence that the cover path and the PNG encoder both work,
not a finished render.*

## Why Rust

The book is C++-only. This port keeps the book's mathematics exactly as
published and replaces its C++ idioms with their Rust equivalents:

- `trait Hittable` returning `Option<HitRecord>` instead of a boolean plus
  out-parameters
- a `Material` **enum** instead of a `shared_ptr<material>` class hierarchy
- `Result` instead of exceptions
- a hand-rolled PCG32 PRNG for deterministic, reproducible renders

There are **no crate dependencies**. Everything — PRNG, PPM and PNG encoding,
argument parsing, error types — is written from scratch.

## Building

Requires a Rust toolchain (stable). No build system beyond Cargo.

```sh
cargo build --release
cargo test
cargo run --release -- --help
```

Windows users without MSVC can install the GNU host:

```sh
rustup-init.exe --default-host x86_64-pc-windows-gnu --default-toolchain stable
```

## Layout

| Path | Contents |
|---|---|
| `src/lib.rs` | renderer modules |
| `src/camera.rs` | the camera frame, the aperture and the pixel loop |
| `src/material.rs` | the `Material` enum: Lambertian, Metal, Dielectric |
| `src/renderer.rs` | the render entry point |
| `src/scene.rs` | named scenes |
| `src/png.rs` | the hand-rolled DEFLATE and PNG encoder |
| `images/` | committed renders — one per milestone, not one per chapter |
| `docs/superpowers/specs/` | design documents |

### A note on `images/`

There is exactly one scene graph, so chapters 6, 7, 8, 10, 11, 12 and 13 all
render the same picture at the same settings. Committing thirteen near-identical
files would record the build process, not the renderer, so `images/` holds what
the renderer actually produces: the current scene, and a small probe of the
cover path.

## The book

This repository contains **no** book text or book code. The book is a local,
copyrighted reference and is gitignored. Chapter commits cite it by section
number so the progression can be followed against the original.

## License

MIT for the code in this repository. *Ray Tracing in One Weekend* is © 2018–2024
Peter Shirley and is neither included nor relicensed here.
