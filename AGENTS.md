# AGENTS.md

Working agreement for `grazing-incidence`: a zero-dependency Rust ray tracer that
ports [*Ray Tracing in One Weekend*](https://raytracing.github.io/books/RayTracingInOneWeekend.html)
chapter by chapter, with one commit per chapter.

`README.md` says what the renderer does and why it is built this way. This file
says what must stay true while you change it, and how to prove it did.

---

## Build

```sh
cargo build --all-targets
cargo test
cargo run --release -- --help
```

On a Windows box without MSVC, `cargo` is not on the default path — prefix every
invocation:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

`rust-toolchain.toml` pins stable. It pins no target on purpose, so Linux CI does
not download a Windows one.

---

## Invariants

Each of these has already broken once. The failure mode is what you are avoiding,
so read that column before changing the code.

### One generator per render

`render_scene` seeds a single `Rng` from `opts.seed` and threads one `&mut` through
both `scene::build` and `Camera::render`. Scene layout and pixel sampling draw
from the same stream, which is what makes the cover reproducible from its seed.

*Fails by:* spending a draw somewhere new. Nothing panics and nothing fails — the
render just quietly differs between runs and the committed images stop meaning
anything. `same_seed_produces_an_identical_image` in `renderer.rs` is the witness.

### The `< 2` dimension guard

Any function dividing by `(dim - 1)` rejects `width < 2 || height < 2` with
`RenderError::InvalidDimension`. The guard belongs in the function that divides,
not only in `Options::parse`, because `render_scene` is `pub` library surface.

*Fails by:* the silent-black trap. `0.0 / 0.0` is NaN; NaN passes `f64::clamp`
untouched; `NaN as u8` is `0`. You get a black channel, no error, and exit code
zero. `Image::new` bounds the product at `MAX_PIXELS` (`1 << 24`) for the same
family of reasons — an unbounded `width * height` used to abort with
`capacity overflow`.

### The mirror follows the anchor

Two camera anchors exist in this repo's history and they need opposite row
handling:

| Anchor | Loop | Row written first |
|---|---|---|
| `lower_left_corner` (bottom-left, §4–§11) | `for j in 0..height` | bottom of the viewport → **mirror the write index** |
| `pixel00_loc` (top-left, §12 onward) | `for j in 0..height` | top of the viewport → **no mirror** |

The current camera uses `pixel00_loc`, so `Camera::render` writes `image.set(i, j, …)`.
Adding a mirror there flips the image upside-down.

*Fails by:* an upside-down picture that nothing panics on. It stayed invisible for
three chapters because a gradient looks the same mirrored and a centred sphere is
symmetric — it only surfaced when a ground plane arrived. If you reintroduce a
bottom-left anchor, mirror its write index, and keep an orientation test.

### Gamma is applied at write time

`Image::write_ppm` and `Image::write_png` apply it, controlled by
`renderer::GAMMA`. `Image::pixels` stays linear.

*Fails by:* gamma applied during accumulation, which reads as a plausible-looking
image until someone compares it to the book's.

### Pixels are `[f64; 3]`, not `Vec3`

`Image` stores `Vec<[f64; 3]>`, because at chapter 2 the `vec3` module did not
exist yet and `image.rs` was never rewritten for it. Convert at the boundary with
`Vec3::to_rgb` / `Vec3::from_rgb`.

### Zero dependencies

`Cargo.toml [dependencies]` stays empty. The PRNG, PPM and PNG encoders, CRC-32,
Adler-32, DEFLATE, argument parsing and error types are all hand-rolled — that is
the educational premise of the project, not an accident.

### Zero warnings

`cargo build --all-targets` and `cargo test` are warning-free. The single
`#[allow]` in the tree is `clippy::too_many_arguments` on `Camera::new`, whose doc
comment argues why. Adding a second one needs the same argument.

---

## The gate

Run all of it, then commit:

1. `cargo fmt --check` — clean.
2. `cargo build --all-targets` — zero warnings.
3. `cargo test` — green, with pristine output. A warning in test output is a
   finding, not noise.
4. `cargo run --release -- --out <path>` — renders, and **you have looked at it**.
   A PNG can pass every structural test and still not decode; the encoder was
   verified that way, against three independent decoders.
5. `git diff` — only what you meant to change.

Done means all five passed. "It compiled" is not done.

---

## Tests

Tests live in `#[cfg(test)] mod tests` at the bottom of the module they test.
There is no `tests/` directory; the book exposes no seams worth testing from
outside.

**Compute every numeric expected value by hand before asserting it.** This is not
theoretical caution — while this crate was being built, eighteen numeric
assertions turned out to be wrong, and they failed in recognisable families:

- measuring an angle from the plane instead of from the normal (`asin` where the
  quantity wanted `acos`)
- a rounded constant asserted at `1e-9` (the true value was 28.1255057)
- an impossible property — unit length past the critical angle, where no refracted
  ray exists and the real length is 1.87
- a bound that could not discriminate, because the channel or component it
  inspected was constant or identically zero
- a "grazing" ray that was not past the threshold it was supposed to cross

Adjust a wrong assertion to what the code should do, and say so in the commit
message. Loosening a tolerance until a stale number passes hides the defect.

**Assert the boundary from both sides.** `render_scene(2, 2)` succeeding is what
proves a `>= 2` guard rather than `> 2`.

**Mutation-check new tests.** Rust does not compile a file in `src/` that no `mod`
statement references, so writing a test before declaring its module gives you a
successful build with *zero tests collected* — not a red test. There is no
compile-time RED in this crate. Break the implementation in a way the test should
catch, confirm it fails, revert, and record what that proved.

**Keep determinism tests cheap.** `cargo test` builds unoptimized; the defaults are
400×225 at 100 samples per pixel. Render small inside the test.

---

## The book

A local, copyrighted copy of the book sits at the repository root as
`raytracing-github-io-books-RayTracingInOneWeekend-html.md`. It is gitignored and
stays that way — it is the input to this project, not part of it.

Cite it by section number (`Ray Tracing in One Weekend` §11). Reproducing its text,
code or images would put copyrighted material in a public repository.

Its chapters 4–6 render upside-down: the camera anchors on the *bottom*-left
viewport corner, counts `j` upward, and PPM writes the first row as the top. This
repo mirrors the write index instead, so images are right-side-up from chapter 4
and the book is correct again from chapter 12. That deviation is deliberate.

---

## Layout

| Path | Contains |
|---|---|
| `src/lib.rs` | module list |
| `src/camera.rs` | the camera frame, the aperture, the pixel loop |
| `src/material.rs` | `Material`: Lambertian, Metal, Dielectric |
| `src/renderer.rs` | `render_scene`, the render entry point |
| `src/scene.rs` | `Scene::Spheres`, `Scene::Cover` |
| `src/image.rs` | pixel buffer, PPM writer, PNG dispatch |
| `src/png.rs` | CRC-32, Adler-32, LZ77 + fixed-Huffman DEFLATE |
| `src/rng.rs` | PCG32 |
| `images/` | committed renders, per milestone |