//! `grazing-incidence` — a ray tracer built by porting *Ray Tracing in One
//! Weekend* (v4.0.2) to Rust.
//!
//! Modules appear as their chapters land; see `docs/superpowers/specs/` for the
//! chapter-to-commit mapping.

pub mod camera;
pub mod cli;
pub mod error;
pub mod hittable;
pub mod image;
pub mod interval;
pub mod ray;
pub mod renderer;
pub mod scene;
pub mod sphere;
pub mod vec3;

/// Pi, as used throughout the book.
pub const PI: f64 = 3.1415926535897932385;

pub fn degrees_to_radians(degrees: f64) -> f64 {
    degrees * PI / 180.0
}
