//! `grazing-incidence` — a ray tracer built by porting *Ray Tracing in One
//! Weekend* (v4.0.2) to Rust.
//!
//! `README.md` maps each module back to the chapter it came from; `AGENTS.md`
//! records the invariants a change has to preserve.

pub mod camera;
pub mod cli;
pub mod error;
pub mod hittable;
pub mod image;
pub mod interval;
pub mod material;
pub mod png;
pub mod ray;
pub mod renderer;
pub mod rng;
pub mod scene;
pub mod sphere;
pub mod vec3;

/// Pi, as used throughout the book.
pub const PI: f64 = 3.1415926535897932385;

pub fn degrees_to_radians(degrees: f64) -> f64 {
    degrees * PI / 180.0
}
