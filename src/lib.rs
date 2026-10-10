//! `grazing-incidence` — a ray tracer built by porting *Ray Tracing in One
//! Weekend* (v4.0.2) to Rust.
//!
//! The renderer is built chapter by chapter; see `docs/superpowers/specs/` for
//! the design and the chapter-to-commit mapping. Modules appear as their
//! chapters land.

/// Ray-tracing core.
pub mod renderer;

/// Scene description and the built-in scenes.
pub mod scene;