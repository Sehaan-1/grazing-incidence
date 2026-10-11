# grazing-incidence

A ray tracer in Rust — diffuse, metal and dielectric materials, depth of field,
antialiasing, and a hand-rolled PNG encoder. **Zero dependencies.**

![The current render: three spheres on a ground plane, the middle one sharp and the rest out of focus](images/final-spheres.png)

*`images/final-spheres.png` — 400×225 at 100 samples per pixel, the `--scene
spheres` default. A 20-degree lens at `(-2, 2, 1)` behind a 10-degree aperture
focused at 3.4, so the middle ball is sharp while the ground and the outer balls
smear. Rendered by `cargo run --release -- --out images/final-spheres.png`.*

## Status

| | |
|---|---|
| Milestones landed | 15 of 15 |
| Tests | 173, all passing |
| Warnings | none |
| Dependencies | none, by design |
| Scenes | `spheres`, `cover` |

### Milestone → commit

Each row is one commit in `git log`, gated on a clean build, a green suite, and
a render you have actually looked at. Read it top to bottom as the build order.

| # | Lands | Visible change |
|---|---|---|
| 1 | `image` (P6 PPM), `renderer` | a vertical gradient |
| 2 | `vec3` | none — arithmetic only |
| 3 | `ray` | sky gradient |
| 4 | `sphere` | one sphere against sky |
| 5 | `hittable`, `interval` | ground plus two balls |
| 6 | `camera` | none — a pure refactor |
| 7 | `rng` | antialiasing, smoothed silhouettes |
| 8 | `Material::Lambertian` | soft shadows, gamma on |
| 9 | `Material::Metal` | fuzzy reflections |
| 10 | `Material::Dielectric` | refracting glass |
| 11 | camera rewrite, full CLI | reframed |
| 12 | camera aperture | out-of-focus background |
| 13 | `image` (deflate), `png` | the same image, as `.png` |
| 14 | `Scene::Cover` | a field of random spheres |

Two commits render identically to their predecessor by design. Isolating a
refactor from any visual change is what makes it reviewable at all.

## What the renderer does

One evolving program, built bottom-up:

- **Image output** — a PPM writer first, then a PNG encoder, so renders are
  previewable in a browser.
- **Geometry** — `Vec3`, `Ray`, `Sphere`, a `Hittable` trait, `HitRecord` and
  `Interval`, and a camera with a real field of view.
- **Antialiasing** — jittered multi-sample averaging over a hand-rolled PCG32, so
  a render is reproducible from `--seed` alone.
- **Materials** — Lambertian diffuse with shadow-acne avoidance and gamma
  correction, fuzzy metal, and dielectric glass with Schlick's approximation to
  the Fresnel reflectance.
- **The lens** — a positionable camera (`lookfrom`/`lookat`/`vup`) and a thin
  lens, where `focus_dist` and `defocus_angle` replace the pinhole.
- **Scenes** — a small showcase, and a field of hundreds of random spheres.

### Scenes

| Scene | What it is |
|---|---|
| `spheres` | A ground plane, a matte ball at `(0, 0, -1.2)`, a glass ball at `(-1, 0, -1)` with a hollow glass bubble of radius 0.4 inside it, and a metal ball at `(1, 0, -1)`. |
| `cover` | A gap-cleared grid of ~484 randomly placed and randomly materialed spheres, plus three deliberate foreground balls. |

```sh
# the default image
cargo run --release -- --out images/out.png

# the cover, small. At its full 1200x675 and 500 samples that is 405 million
# primary rays against ~488 spheres -- start small.
cargo run --release -- --scene cover --width 200 --samples 8 --out images/cover-probe.png
```

![The cover scene: a grid of random spheres over a flat ground, with three larger foreground balls](images/cover-probe.png)

*`images/cover-probe.png` — `cover` at 200×112 and 8 samples per pixel. Noisy
on purpose: it is evidence that the cover path and the PNG encoder both work,
not a finished render.*

## How it is built

| Choice | What it buys |
|---|---|
| `trait Hittable` returning `Option<HitRecord>` | No boolean-plus-out-parameter intersection, which is the easiest thing in this kind of renderer to get wrong |
| `Material` as a flat **enum** | Owned, movable, exhaustively matched — and `HitRecord` stays free of lifetimes and reference counting |
| `Result` instead of exceptions | Errors carry the flag or value that caused them |
| A hand-rolled PCG32 | Deterministic, reproducible renders with no dependency |
| `scatter` returning `Option<(Ray, Color)>` | Attenuation ships in the same signature from the first material, so adding metal later is not a breaking change |
| **Zero dependencies** | A ray tracer needs no crates, and writing the PRNG, both image encoders, CRC-32, Adler-32, DEFLATE, argument parsing and error types by hand is the point |

## Design decisions

The ones where a reasonable person might have chosen differently:

| Decision | Why |
|---|---|
| **One evolving program, one commit per milestone** | One program that grows, rather than a tree of small binaries left behind. Git already provides replayability. |
| **Zero dependencies** | Everything is written here, which is both the honest answer and the educational one. |
| **Deterministic by default** | A fixed seed, overridable with `--seed`, so identical code yields byte-identical images. A diff in `images/` always means a real change. |
| **P6 PPM, not P3** | The same image a quarter of the size: binary, one `write` instead of ~270,000 separately formatted numbers. |
| **Unit tests on the mathematics** | Not golden images. There is no stable output to pin, and golden PPMs break under any refactor. Committed images are visual artifacts, not fixtures. |
| **Right-side-up images** | The naive camera — anchor on the viewport's bottom-left corner, count rows upward — puts the bottom of the scene in the image's top row, because PPM writes the first row as the top. The camera here anchors top-left and `pixel_delta_v` points down, so rows fall where they should with no correction at all. See `AGENTS.md`. |
| **Hand-rolled PNG** | Committed PPMs are a few hundred KB and do not preview on GitHub. LZ77 plus a fixed-Huffman DEFLATE gets them under a tenth of that, from scratch. |

## Not in scope

- Quads, triangles, textures, transforms, emissive lights, volumetric or
  participating-media rendering.
- Parallel or multithreaded rendering.
- A reusable general-purpose renderer library — this is an application, not a crate
  other projects should depend on.

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
| `src/lib.rs` | module list |
| `src/camera.rs` | the camera frame, the aperture and the pixel loop |
| `src/material.rs` | `Material`: Lambertian, Metal, Dielectric |
| `src/renderer.rs` | the render entry point |
| `src/scene.rs` | named scenes |
| `src/image.rs` | pixel buffer, PPM writer, encoder dispatch |
| `src/png.rs` | CRC-32, Adler-32, LZ77 and fixed-Huffman DEFLATE |
| `src/rng.rs` | PCG32 |
| `images/` | committed renders — per milestone, not per commit |
| `AGENTS.md` | working agreement for coding agents: invariants, traps, the gate |

### A note on `images/`

There is one scene graph, so most of the later milestones all render the same
picture at the same settings. Committing a dozen near-identical files would
record the build order rather than the renderer, so `images/` holds what the
renderer actually produces: the current scene, and a small probe of the cover
path.

## License

MIT — see [`LICENSE`](LICENSE).