# grazing-incidence

A ray tracer built by porting [*Ray Tracing in One
Weekend*](https://raytracing.github.io/books/RayTracingInOneWeekend.html)
(v4.0.2, Peter Shirley) from C++ to Rust — one chapter per commit.

<p align="center">
  <em>Rendered progressively; see <code>images/</code> and the commit history.</em>
</p>

## Status

Design approved; implementation not yet started. See
[`docs/superpowers/specs/`](docs/superpowers/specs/) for the full design and the
chapter-to-commit mapping.

| | |
|---|---|
| Chapters landed | 0 of 13 |
| Dependencies | none, by design |

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
| `src/lib.rs` | renderer modules, appearing as chapters land |
| `src/renderer.rs` | the pixel loop and progress indicator |
| `src/scene.rs` | named scenes |
| `images/` | committed output, one file per chapter |
| `docs/superpowers/specs/` | design documents |

## The book

This repository contains **no** book text or book code. The book is a local,
copyrighted reference and is gitignored. Chapter commits cite it by section
number so the progression can be followed against the original.

## License

MIT for the code in this repository. *Ray Tracing in One Weekend* is © 2018–2024
Peter Shirley and is neither included nor relicensed here.