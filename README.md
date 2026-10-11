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
| Tests | 173, all passing |
| Dependencies | none, by design |
| Scenes | `spheres` (§6–§13), `cover` (§14) |

### Chapter → commit

The book has 14 numbered chapters; 1 ("Overview") and 14 ("Where Next?") are
prose. Twelve are code, and the last is ours. Each row is one commit in `git log`,
gated on a clean build, a green suite, and a render.

| Commit | Book chapter | Lands | Visible change |
|---|---|---|---|
| 1 | §2 Output an Image | `image` (P6 PPM), `renderer` | the vertical gradient |
| 2 | §3 The vec3 Class | `vec3` | none — arithmetic only |
| 3 | §4 Rays, Camera, Background | `ray` | sky gradient |
| 4 | §5 Adding a Sphere | `sphere` | one sphere on sky |
| 5 | §6 Normals, Multiple Objects | `hittable`, `interval` | ground plus two balls |
| 6 | §7 Camera Into Its Own Class | `camera` | none — pure refactor |
| 7 | §8 Antialiasing | `rng` | smoothed silhouettes |
| 8 | §9 Diffuse Materials | `Material::Lambertian` | soft shadows, gamma on |
| 9 | §10 Metal | `Material::Metal` | fuzzy reflections |
| 10 | §11 Dielectrics | `Material::Dielectric` | refracting glass |
| 11 | §12 Positionable Camera | camera rewrite | reframed |
| 12 | §13 Defocus Blur | camera aperture | out-of-focus background |
| 13 | — (ours) PNG Output | `image` (deflate), `png` | same image, `.png` |
| 14 | §14 cover scene | `Scene::Cover` | the field of random spheres |

Commits 2 and 6 render identically to their predecessor by design — isolating the
refactor from any visual change is what makes them reviewable at all.

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

## Design decisions

The ones where a reasonable person might have chosen differently:

| Decision | Why |
|---|---|
| **One evolving program, one commit per chapter** | Mirrors how the book actually teaches — one program that grows. Avoids the thirteen dead binaries a per-chapter-layout would leave behind. Git already provides replayability. |
| **Zero dependencies** | A ray tracer needs no crates, and hand-rolling is the most educational option available. |
| **Deterministic by default** | A fixed seed, overridable with `--seed`, so identical code yields byte-identical images. A diff in `images/` always means a real change. |
| **P6 PPM, not the book's P3** | Same image, a quarter of the size: binary, one `write` instead of ~270,000 formatted numbers. The book's own released `inOneWeekend` also emits P6. |
| **Unit tests on the mathematics** | Not golden images. The book has no stable output to pin, and golden PPMs would break under any refactor. Committed images are visual artifacts, not fixtures. |
| **`Material` as an enum, `Hittable` as a trait** | Replaces the book's `shared_ptr` hierarchy and its boolean-plus-out-parameter `hit`, which is its most error-prone idiom. `scatter` returns `Option<(Ray, Color)>` from the first commit that has materials, so chapter 10's metal does not force a breaking signature change. |
| **Right-side-up images, unlike the book's chapters 4–6** | See below. |

### On the orientation

The book's chapters 4–6 render **upside-down**. Its camera anchors on the
*bottom*-left corner of the viewport and counts `j` upward, while PPM writes the
first row as the *top* — so the bottom of the viewport lands in the image's top
row. The book fixes this at chapter 12 by switching the anchor to `pixel00_loc`.

This port does not wait: it mirrors the write index, so every image is right-side-up
from chapter 4, and the mirror comes out when chapter 12 switches anchors. The
deviation is deliberate and documented in `AGENTS.md`.

## Not in scope

- *The Next Week* (quads, textures, transforms, lights, motion blur) and Book 3.
- Parallel or multithreaded rendering.
- A reusable general-purpose renderer library.
- Redistributing the book's text, code or images.

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
| `AGENTS.md` | working agreement for coding agents: invariants, traps, the gate |

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
