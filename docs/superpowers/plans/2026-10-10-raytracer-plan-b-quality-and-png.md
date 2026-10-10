# grazing-incidence — Quality, Materials, and PNG Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the ray tracer by porting chapters 8–14 of *Ray Tracing in One Weekend*, then add hand-rolled PNG output.

**Architecture:** Continues the crate built by `2026-10-10-raytracer-plan-a-core.md`. Task 1 seeds a PCG32 generator so every later chapter is deterministic. Materials arrive as one `Material` enum variant per chapter. The final task adds `png.rs`, a self-contained DEFLATE/PNG encoder, so committed images render inline on GitHub.

**Tech Stack:** Rust 1.99 stable (`x86_64-pc-windows-gnu`), Cargo, no crates.

**Prerequisite:** Plan A must be complete and committed. Chapter 7's `Camera` and the `Hittable`/`Sphere` types are the foundation for everything here.

**Source book:** *Ray Tracing in One Weekend* v4.0.2 by Peter Shirley, a local gitignored file. Each task cites its section.

## Global Constraints

Identical to Plan A, and equally binding:

- **Zero dependencies.** Never add a crate.
- **Toolchain.** Prefix commands with `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"` if `cargo` is not found.
- **Tests.** `#[cfg(test)] mod tests` at the bottom of each module. No `tests/` directory.
- **Float comparisons** use an epsilon. Exact `==` only where exactly representable.
- **Zero warnings.** No `#[allow(dead_code)]`.
- **Commit gate.** `cargo build` clean, `cargo test` green, image rendered, then commit.
- **Commit message** cites the book section.
- **Do not touch `docs/superpowers/specs/`.**
- **`Image` stores `Vec<[f64; 3]>`.** Never store `Vec3` in the buffer.
- **Gamma is applied at write time**, controlled by `renderer::GAMMA`.
- **Determinism is not optional.** Every render seeds from `renderer::DEFAULT_SEED` unless `--seed` overrides it. If two runs with the same seed and same dimensions differ, something consumed randomness in a new place — find it.

---

## Task 1: PCG32 random numbers (§8, part one)

**Files:**
- Create: `src/rng.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `vec3::Vec3`
- Produces:
  - `rng::Rng::seed_from_u64(seed: u64) -> Rng`
  - `Rng::next_u32(&mut self) -> u32`
  - `Rng::f64(&mut self) -> f64` — uniform in `[0, 1)`
  - `Rng::range(&mut self, min: f64, max: f64) -> f64`
  - `Rng::unit_vector(&mut self) -> Vec3`
  - `Rng::in_unit_disk(&mut self) -> Vec3`

The book's `random_double()` becomes `Rng::f64()`. This is the one place the port departs from the book's `std::rand`, because the spec requires deterministic renders from a fixed seed.

- [ ] **Step 1: Write `src/rng.rs`**

```rust
//! PCG32, a small hand-rolled generator with a reproducible seed.

use crate::vec3::Vec3;

const MULTIPLIER: u64 = 6364136223846793005;
const INCREMENT: u64 = 11634580027462260723;

#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    pub fn seed_from_u64(seed: u64) -> Self {
        let mut rng = Self { state: 0, inc: (seed << 1) | 1 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old_state = self.state;
        self.state = old_state.wrapping_mul(MULTIPLIER).wrapping_add(self.inc);

        let xorshifted = (((old_state >> 18) ^ old_state) >> 27) as u32;
        let rot = (old_state >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform in `[0, 1)`, using the top 24 bits so the result is exactly
    /// representable as an `f64`.
    pub fn f64(&mut self) -> f64 {
        let bits = self.next_u32() >> 8;
        f64::from(bits) * (1.0 / 16_777_216.0)
    }

    pub fn range(&mut self, min: f64, max: f64) -> f64 {
        min + (max - min) * self.f64()
    }

    /// A uniformly distributed point on the unit sphere, by rejection (§8).
    pub fn unit_vector(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            );
            if 1e-6 < v.length_squared() {
                return v.unit();
            }
        }
    }

    /// A uniformly distributed point in the unit disk, by rejection (§8).
    pub fn in_unit_disk(&mut self) -> Vec3 {
        loop {
            let p = Vec3::new(
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                0.0,
            );
            if p.length_squared() < 1.0 {
                return p;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_the_same_sequence() {
        let mut a = Rng::seed_from_u64(42);
        let mut b = Rng::seed_from_u64(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::seed_from_u64(1);
        let mut b = Rng::seed_from_u64(2);
        assert!((0..10).any(|_| a.next_u32() != b.next_u32()));
    }

    #[test]
    fn f64_stays_in_the_unit_interval() {
        let mut rng = Rng::seed_from_u64(7);
        for _ in 0..10_000 {
            let v = rng.f64();
            assert!((0.0..1.0).contains(&v), "f64 out of range: {v}");
        }
    }

    #[test]
    fn f64_is_reasonably_uniform() {
        let mut rng = Rng::seed_from_u64(99);
        let mut buckets = [0usize; 10];
        for _ in 0..100_000 {
            buckets[(rng.f64() * 10.0) as usize] += 1;
        }
        for count in buckets {
            assert!(count > 9_000 && count < 11_000, "buckets skewed: {buckets:?}");
        }
    }

    #[test]
    fn range_respects_its_bounds() {
        let mut rng = Rng::seed_from_u64(5);
        for _ in 0..10_000 {
            let v = rng.range(-0.5, 0.5);
            assert!((-0.5..0.5).contains(&v));
        }
    }

    #[test]
    fn unit_vector_has_unit_length() {
        let mut rng = Rng::seed_from_u64(2024);
        for _ in 0..1_000 {
            assert!((rng.unit_vector().length() - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn in_unit_disk_is_flat_and_inside() {
        let mut rng = Rng::seed_from_u64(11);
        for _ in 0..1_000 {
            let p = rng.in_unit_disk();
            assert!(p.length_squared() <= 1.0);
            assert_eq!(p.z, 0.0);
        }
    }
}
```

- [ ] **Step 2: Register and test**

Add to `src/lib.rs`:

```rust
pub mod rng;
```

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: 56 tests PASS.

- [ ] **Step 3: Format and commit**

```bash
cargo fmt
git add -A
git commit -m "feat(ch08): hand-rolled PCG32 for deterministic renders (Ray Tracing in One Weekend §8)"
```

---

## Task 2: Antialiasing (§8, part two)

**Files:**
- Modify: `src/camera.rs`, `src/renderer.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `rng::Rng`
- Produces:
  - `Camera::render(&self, world: &dyn Hittable, rng: &mut Rng) -> Result<Image, RenderError>` (signature changes to take a seed source)
  - `Camera::get_ray(&self, i: usize, j: usize, rng: &mut Rng) -> Ray`
  - `renderer::DEFAULT_SEED: u64`

- [ ] **Step 1: Thread an `&mut Rng` through `Camera::render`**

In `src/camera.rs`, add `use crate::rng::Rng;` and replace `render`:

```rust
    pub fn render(&self, world: &dyn Hittable, rng: &mut Rng) -> Result<Image, RenderError> {
        let mut image = Image::new(self.image_width, self.image_height)?;

        for j in 0..self.image_height {
            eprint!("\rScanlines remaining: {} ", self.image_height - j);
            for i in 0..self.image_width {
                let mut color = Vec3::ZERO;
                for _ in 0..self.samples_per_pixel {
                    let ray = self.get_ray(i, j, rng);
                    color += self.ray_color(&ray, self.max_depth, world);
                }
                image.set(i, j, (color / self.samples_per_pixel as f64).to_rgb());
            }
        }
        eprintln!("\rDone.                 ");

        Ok(image)
    }

    /// A camera ray for pixel (i, j), jittered within the pixel (§8).
    fn get_ray(&self, i: usize, j: usize, rng: &mut Rng) -> Ray {
        let du = rng.range(0.0, 1.0);
        let dv = rng.range(0.0, 1.0);
        let u = (i as f64 + du) / self.image_width as f64;
        let v = (j as f64 + dv) / self.image_height as f64;

        Ray::new(
            self.origin,
            self.lower_left_corner + self.horizontal * u + self.vertical * v - self.origin,
        )
    }
```

Note the divisor changes from `image_width - 1` to `image_width`: jittered samples should fill the pixel, not land on its far edge. This is the book's change too.

Update the two tests in `camera.rs` to pass a generator:

```rust
        let mut rng = Rng::seed_from_u64(1);
        let image = camera.render(&world, &mut rng).unwrap();
```

- [ ] **Step 2: Seed a generator in the renderer**

In `src/renderer.rs`:

```rust
use crate::rng::Rng;

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = false;

/// The seed every committed image is rendered with, so reruns are identical.
pub const DEFAULT_SEED: u64 = 0;

pub fn render_scene(width: usize, height: usize) -> Result<Image, RenderError> {
    let objects = scene::build(Scene::Spheres);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    let mut rng = Rng::seed_from_u64(DEFAULT_SEED);
    let camera = Camera::new(width, height, 100, 5);
    camera.render(&world, &mut rng)
}
```

- [ ] **Step 3: Add a determinism test to `renderer.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_produces_an_identical_image() {
        let a = render_scene(32, 20).unwrap();
        let b = render_scene(32, 20).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }
}
```

**This test is the guarantee behind every committed image.** If it fails, randomness is being consumed from somewhere outside the seeded generator.

- [ ] **Step 4: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: 57 tests PASS.

- [ ] **Step 5: Format, render, and commit**

```bash
cargo fmt
cargo run --release
```
Expected: visibly smoother edges on the sphere silhouette.

```bash
git add -A
git commit -m "feat(ch08): antialias by jittered multi-sample averaging (Ray Tracing in One Weekend §8)"
```

---

## Task 3: Diffuse materials (§9 Diffuse Materials)

**Files:**
- Create: `src/material.rs`
- Modify: `src/lib.rs`, `src/hittable.rs`, `src/sphere.rs`, `src/camera.rs`, `src/renderer.rs`, `src/vec3.rs`, `src/scene.rs`

**Interfaces:**
- Consumes: `hittable::{HitRecord, Hittable}`, `ray::Ray`, `rng::Rng`, `vec3::{Vec3, Color}`
- Produces:
  - `material::Material::Lambertian { albedo: Color }` and `Material::lambertian(albedo)`
  - `Material::scatter(&self, ray: &Ray, rec: &HitRecord, rng: &mut Rng) -> Option<(Ray, Color)>`
  - `HitRecord` gains `pub material: Material`
  - `Sphere` gains `pub material: Material`; `Sphere::new(center, radius, material)`
  - `Color::random(rng) -> Color`, `Color::random_range(rng, min, max) -> Color`
  - `renderer::GAMMA` becomes `true`

The two-tuple return from `scatter` is the documented divergence: the book's `lambertian` returns only a ray, but §10's `metal` needs attenuation from the same signature. Adopting the tuple now avoids a breaking change later.

- [ ] **Step 1: Write `src/material.rs`**

```rust
//! Surface materials.

use crate::hittable::HitRecord;
use crate::ray::Ray;
use crate::rng::Rng;
use crate::vec3::{Color, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    Lambertian { albedo: Color },
}

impl Material {
    pub fn lambertian(albedo: Color) -> Self {
        Self::Lambertian { albedo }
    }

    /// Choose a scattered ray and its attenuation (§9). `None` absorbs the ray.
    pub fn scatter(
        &self,
        ray: &Ray,
        rec: &HitRecord,
        rng: &mut Rng,
    ) -> Option<(Ray, Color)> {
        match self {
            // §9: normal + a random unit vector approximates the Lambertian
            // distribution. The near_zero guard catches the case where the two
            // cancel exactly, which would otherwise yield a zero direction.
            Material::Lambertian { albedo } => {
                let mut direction = rec.normal + rng.unit_vector();
                if direction.near_zero() {
                    direction = rec.normal;
                }
                Some((Ray::new(rec.point, direction), *albedo))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(point: Vec3, normal: Vec3, material: Material) -> HitRecord {
        HitRecord { t: 1.0, point, normal, front_face: true, material }
    }

    fn straight_down(normal: Vec3) -> Ray {
        Ray::new(Vec3::ZERO, -normal)
    }

    #[test]
    fn lambertian_returns_the_albedo_unchanged() {
        let albedo = Vec3::new(0.4, 0.2, 0.1);
        let material = Material::lambertian(albedo);
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let rec = record(Vec3::ZERO, normal, material);
        let (_, attenuation) = material
            .scatter(&straight_down(normal), &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        assert_eq!(attenuation, albedo);
    }

    #[test]
    fn lambertian_scatters_from_the_hit_point() {
        let material = Material::lambertian(Vec3::UNIT);
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let point = Vec3::new(1.0, 2.0, 3.0);
        let rec = record(point, normal, material);
        let (scattered, _) = material
            .scatter(&straight_down(normal), &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        assert_eq!(scattered.origin, point);
    }

    #[test]
    fn lambertian_never_scatters_below_the_surface() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::lambertian(Vec3::UNIT);
        let rec = record(Vec3::ZERO, normal, material);
        let mut rng = Rng::seed_from_u64(7);
        for _ in 0..10_000 {
            let (scattered, _) = material.scatter(&straight_down(normal), &rec, &mut rng).unwrap();
            assert!(scattered.direction.dot(normal) > 0.0);
        }
    }

    #[test]
    fn lambertian_scatter_direction_is_never_zero_length() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::lambertian(Vec3::UNIT);
        let rec = record(Vec3::ZERO, normal, material);
        let mut rng = Rng::seed_from_u64(3);
        for _ in 0..10_000 {
            let (scattered, _) = material.scatter(&straight_down(normal), &rec, &mut rng).unwrap();
            assert!(!scattered.direction.near_zero());
        }
    }

    #[test]
    fn scattering_is_reproducible_for_a_given_seed() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::lambertian(Vec3::UNIT);
        let rec = record(Vec3::ZERO, normal, material);
        let ray = straight_down(normal);
        let a = material.scatter(&ray, &rec, &mut Rng::seed_from_u64(4)).unwrap().0;
        let b = material.scatter(&ray, &rec, &mut Rng::seed_from_u64(4)).unwrap().0;
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 2: Add `material` to `HitRecord` and `Sphere`**

In `src/hittable.rs`:

```rust
use crate::material::Material;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRecord {
    pub t: f64,
    pub point: Vec3,
    pub normal: Vec3,
    pub front_face: bool,
    pub material: Material,
}
```

In `src/sphere.rs`:

```rust
use crate::material::Material;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f64,
    pub material: Material,
}

impl Sphere {
    pub const fn new(center: Vec3, radius: f64, material: Material) -> Self {
        Self { center, radius, material }
    }
}
```

and add `material: self.material` to the `HitRecord` literal in `hit`:

```rust
        Some(HitRecord {
            t,
            point,
            normal,
            front_face,
            material: self.material,
        })
```

The `sphere.rs` tests must be updated too. Add `use crate::material::Material;` to the
test module and give each `Sphere::new` a material:

```rust
    fn test_sphere() -> Sphere {
        Sphere::new(Vec3::ZERO, 0.5, Material::lambertian(Vec3::UNIT))
    }
```

then change each existing `Sphere::new(...)` call in that module to
`Sphere::new(<same args>, Material::lambertian(Vec3::UNIT))`.

The same applies to the four `Sphere::new` calls in `hittable.rs`'s test module,
which also needs `use crate::material::Material;`.

- [ ] **Step 3: Give the scene real materials**

In `src/scene.rs`:

```rust
use crate::material::Material;

pub fn build(scene: Scene) -> Vec<Box<dyn Hittable>> {
    match scene {
        Scene::Spheres => {
            let ground = Sphere::new(
                Vec3::new(0.0, -1000.0, 0.0),
                1000.0,
                Material::lambertian(Vec3::new(0.5, 0.5, 0.5)),
            );
            let ball = Sphere::new(
                Vec3::new(0.0, 0.5, 0.0),
                0.5,
                Material::lambertian(Vec3::new(0.7, 0.3, 0.3)),
            );
            vec![Box::new(ground), Box::new(ball)]
        }
    }
}
```

- [ ] **Step 4: Add random colour helpers to `vec3.rs`**

Add a second `impl Color` block (a second block is required — `Color` is an alias, so the methods must live in `impl Color`, not `impl Vec3`):

```rust
impl Color {
    /// A uniformly random colour with each component in `[0, 1)` (§10).
    pub fn random(rng: &mut crate::rng::Rng) -> Self {
        Self::new(rng.f64(), rng.f64(), rng.f64())
    }

    /// A uniformly random colour with each component in `[min, max)` (§10).
    pub fn random_range(rng: &mut crate::rng::Rng, min: f64, max: f64) -> Self {
        Self::new(
            rng.range(min, max),
            rng.range(min, max),
            rng.range(min, max),
        )
    }
}
```

- [ ] **Step 5: Wire materials into the shading loop**

In `src/camera.rs`, replace `ray_color`:

```rust
    fn ray_color(
        &self,
        ray: &Ray,
        depth: usize,
        world: &dyn Hittable,
        rng: &mut Rng,
    ) -> Vec3 {
        if depth == 0 {
            return Vec3::ZERO;
        }

        // §9: a t_min of 0.001 skips the self-intersection that floating-point
        // rounding introduces. This is the "shadow acne" the book fixes here.
        if let Some(rec) = world.hit(ray, 0.001, f64::INFINITY) {
            if let Some((scattered, attenuation)) = rec.material.scatter(ray, &rec, rng) {
                return attenuation * self.ray_color(&scattered, depth - 1, world, rng);
            }
            return Vec3::ZERO;
        }

        let unit_direction = ray.direction.unit();
        let a = 0.5 * (unit_direction.y + 1.0);
        Vec3::UNIT * (1.0 - a) + Vec3::new(0.5, 0.7, 1.0) * a
    }
```

and pass `rng` at the call site in `render`:

```rust
                    color += self.ray_color(&ray, self.max_depth, world, rng);
```

- [ ] **Step 6: Register `material` and flip gamma on**

Add to `src/lib.rs`:

```rust
pub mod material;
```

In `src/renderer.rs`:

```rust
/// Gamma correction landed with diffuse materials in §9.
pub const GAMMA: bool = true;
```

- [ ] **Step 7: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: 62 tests PASS, no warnings.

- [ ] **Step 8: Format, render, and commit**

```bash
cargo fmt
cargo run --release
```
Expected: soft diffuse shadows, and the scene visibly brighter now that gamma is applied.

```bash
git add -A
git commit -m "feat(ch09): lambertian material, shadow acne fix, gamma correction (Ray Tracing in One Weekend §9)"
```

---

## Task 4: Metal (§10 Metal)

**Files:**
- Modify: `src/material.rs`, `src/scene.rs`

**Interfaces:**
- Consumes: `vec3::reflect` (Plan A, Task 2)
- Produces:
  - `Material::Metal { albedo: Color, fuzz: f64 }` and `Material::metal(albedo, fuzz)`

- [ ] **Step 1: Add the variant and its scatter arm**

In `src/material.rs`, add to the enum:

```rust
    Metal { albedo: Color, fuzz: f64 },
```

and:

```rust
    pub fn metal(albedo: Color, fuzz: f64) -> Self {
        Self::Metal { albedo, fuzz }
    }
```

and extend `scatter`:

```rust
            // §10: mirror the ray about the normal, then blur it by `fuzz`.
            // A ray scattered below the surface is absorbed, which is what
            // gives metal surfaces their hard-edged look.
            Material::Metal { albedo, fuzz } => {
                let reflected = Vec3::reflect(ray.direction.unit(), rec.normal);
                let fuzz = fuzz.clamp(0.0, 1.0);
                let scattered =
                    Ray::new(rec.point, reflected + fuzz * rng.unit_vector());
                if scattered.direction.dot(rec.normal) < 0.0 {
                    None
                } else {
                    Some((scattered, *albedo))
                }
            }
```

- [ ] **Step 2: Add the metal tests**

Append to `material.rs`'s test module:

```rust
    fn metal_record(normal: Vec3) -> HitRecord {
        record(Vec3::ZERO, normal, Material::metal(Vec3::UNIT, 0.0))
    }

    #[test]
    fn metal_with_zero_fuzz_mirrors_the_ray_exactly() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(Vec3::UNIT, 0.0);
        let rec = metal_record(normal);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0).unit());

        let (scattered, _) = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        let expected = Vec3::reflect(ray.direction.unit(), normal);
        assert!((scattered.direction - expected).length() < 1e-12);
    }

    #[test]
    fn metal_fuzz_is_clamped_to_the_unit_interval() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(Vec3::UNIT, 50.0);
        let rec = metal_record(normal);
        let ray = straight_down(normal);

        let mut absorbed = 0;
        let mut rng = Rng::seed_from_u64(9);
        for _ in 0..1_000 {
            if material.scatter(&ray, &rec, &mut rng).is_none() {
                absorbed += 1;
            }
        }
        // Clamped to 1.0 the sphere of possible directions covers the whole
        // hemisphere, so roughly half of these grazing rays are absorbed.
        assert!(absorbed > 300 && absorbed < 700, "absorbed {absorbed} of 1000");
    }

    #[test]
    fn metal_never_scatters_below_its_surface() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(Vec3::UNIT, 0.5);
        let rec = metal_record(normal);
        let ray = straight_down(normal);
        let mut rng = Rng::seed_from_u64(13);
        for _ in 0..10_000 {
            if let Some((scattered, _)) = material.scatter(&ray, &rec, &mut rng) {
                assert!(scattered.direction.dot(normal) > 0.0);
            }
        }
    }

    #[test]
    fn metal_attenuates_by_its_albedo() {
        let albedo = Vec3::new(0.9, 0.4, 0.1);
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::metal(albedo, 0.0);
        let rec = record(Vec3::ZERO, normal, material);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0).unit());
        let (_, attenuation) = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        assert_eq!(attenuation, albedo);
    }
```

- [ ] **Step 3: Put a metal sphere in the scene**

In `src/scene.rs`, change the ball's material. The whole `Scene::Spheres` arm becomes:

```rust
pub fn build(scene: Scene) -> Vec<Box<dyn Hittable>> {
    match scene {
        Scene::Spheres => {
            let ground = Sphere::new(
                Vec3::new(0.0, -1000.0, 0.0),
                1000.0,
                Material::lambertian(Vec3::new(0.5, 0.5, 0.5)),
            );
            let ball = Sphere::new(
                Vec3::new(0.0, 0.5, 0.0),
                0.5,
                Material::metal(Vec3::new(0.9, 0.4, 0.1), 0.0),
            );
            vec![Box::new(ground), Box::new(ball)]
        }
    }
}
```

Keep the ground Lambertian so the contrast is visible.

- [ ] **Step 4: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: 66 tests PASS.

- [ ] **Step 5: Format, render, and commit**

```bash
cargo fmt
cargo run --release
```
Expected: a mirror-like sphere reflecting the ground and sky.

```bash
git add -A
git commit -m "feat(ch10): metal with fuzzy reflection (Ray Tracing in One Weekend §10)"
```

---

## Task 5: Dielectrics (§11 Dielectrics)

**Files:**
- Modify: `src/material.rs`, `src/scene.rs`

**Interfaces:**
- Consumes: `vec3::refract` (Plan A, Task 2)
- Produces:
  - `Material::Dielectric { ior: f64 }` and `Material::dielectric(ior)`
  - `reflectance(cosine: f64, refraction_ratio: f64) -> f64` (Schlick; module-private)

- [ ] **Step 1: Add the variant, the scatter arm, and Schlick's approximation**

In `src/material.rs`, add to the enum:

```rust
    Dielectric { ior: f64 },
```

and:

```rust
    pub fn dielectric(ior: f64) -> Self {
        Self::Dielectric { ior }
    }
```

and extend `scatter`:

```rust
            // §11: Snell's law, with total internal reflection handled first
            // and Schlick's approximation deciding the reflect/refract split.
            Material::Dielectric { ior } => {
                let refraction_ratio = if rec.front_face { 1.0 / ior } else { *ior };

                let unit_direction = ray.direction.unit();
                let cos_theta = (-unit_direction).dot(rec.normal).min(1.0);
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

                // Beyond the critical angle there is no refracted solution.
                let cannot_refract = refraction_ratio * sin_theta > 1.0;

                let direction = if cannot_refract
                    || reflectance(cos_theta, refraction_ratio) > rng.f64()
                {
                    Vec3::reflect(unit_direction, rec.normal)
                } else {
                    Vec3::refract(unit_direction, rec.normal, refraction_ratio)
                };

                Some((Ray::new(rec.point, direction), Vec3::UNIT))
            }
```

and add the free function at module level:

```rust
/// Schlick's approximation of Fresnel reflectance (§11).
fn reflectance(cosine: f64, refraction_ratio: f64) -> f64 {
    let mut r0 = (1.0 - refraction_ratio) / (1.0 + refraction_ratio);
    r0 *= r0;
    r0 + (1.0 - r0) * (1.0 - cosine).powi(5)
}
```

- [ ] **Step 2: Add the dielectric tests**

Append to `material.rs`'s test module. `dielectric_record` builds on the `record`
helper added in Task 3, overriding `front_face` because exiting glass is a
back-face event:

```rust
    fn dielectric_record(normal: Vec3, front_face: bool) -> HitRecord {
        let mut rec = record(Vec3::ZERO, normal, Material::dielectric(1.5));
        rec.front_face = front_face;
        rec
    }

    #[test]
    fn reflectance_at_normal_incidence_matches_r0() {
        // r0 = ((1 - eta) / (1 + eta))^2 with eta = 1/1.5 gives 0.04.
        let r = reflectance(1.0, 1.0 / 1.5);
        assert!((r - 0.04).abs() < 1e-12);
    }

    #[test]
    fn reflectance_rises_toward_grazing_angles() {
        let eta = 1.0 / 1.5;
        let r_normal = reflectance(1.0, eta);
        let r_grazing = reflectance(0.0, eta);
        assert!(r_grazing > r_normal);
        assert!(r_grazing <= 1.0);
    }

    #[test]
    fn glass_does_not_dim_the_ray() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        let rec = dielectric_record(normal, true);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0).unit());
        let (_, attenuation) = material
            .scatter(&ray, &rec, &mut Rng::seed_from_u64(1))
            .unwrap();
        assert_eq!(attenuation, Vec3::UNIT);
    }

    #[test]
    fn glass_refracts_a_head_on_ray_without_deflecting_it() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        let rec = dielectric_record(normal, true);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0));

        let mut rng = Rng::seed_from_u64(1);
        let mut straight = false;
        for _ in 0..100 {
            if let Some((scattered, _)) = material.scatter(&ray, &rec, &mut rng) {
                // At normal incidence reflectance is 0.04, so most samples
                // refract straight through.
                if (scattered.direction - ray.direction).length() < 1e-9 {
                    straight = true;
                }
            }
        }
        assert!(straight);
    }

    #[test]
    fn total_internal_reflection_always_reflects() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        // front_face is false because we are exiting the glass.
        let rec = dielectric_record(normal, false);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, -0.01, 0.0).unit());

        let expected = Vec3::reflect(ray.direction.unit(), normal);
        let mut rng = Rng::seed_from_u64(2);
        for _ in 0..1_000 {
            let (scattered, _) = material.scatter(&ray, &rec, &mut rng).unwrap();
            assert!((scattered.direction - expected).length() < 1e-9);
        }
    }

    #[test]
    fn dielectric_scatter_is_reproducible() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let material = Material::dielectric(1.5);
        let rec = dielectric_record(normal, true);
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0).unit());
        let a = material.scatter(&ray, &rec, &mut Rng::seed_from_u64(5)).unwrap().0;
        let b = material.scatter(&ray, &rec, &mut Rng::seed_from_u64(5)).unwrap().0;
        assert_eq!(a, b);
    }
```

- [ ] **Step 3: Put a glass sphere in the scene**

In `src/scene.rs`, the three-sphere scene from §11's listing:

```rust
use crate::material::Material;

pub fn build(scene: Scene) -> Vec<Box<dyn Hittable>> {
    match scene {
        Scene::Spheres => {
            let ground = Sphere::new(
                Vec3::new(0.0, -1000.0, 0.0),
                1000.0,
                Material::lambertian(Vec3::new(0.8, 0.8, 0.0)),
            );
            let left = Sphere::new(
                Vec3::new(-1.0, 0.0, -1.0),
                0.5,
                Material::dielectric(1.5),
            );
            let middle = Sphere::new(
                Vec3::new(0.0, 0.0, -1.0),
                0.5,
                Material::lambertian(Vec3::new(0.1, 0.2, 0.5)),
            );
            let right = Sphere::new(
                Vec3::new(1.0, 0.0, -1.0),
                0.5,
                Material::metal(Vec3::new(0.8, 0.6, 0.2), 0.0),
            );
            vec![
                Box::new(ground),
                Box::new(left),
                Box::new(middle),
                Box::new(right),
            ]
        }
    }
}
```

Update the `spheres_scene_contains_two_objects` test in `scene.rs` to assert 4, and rename it `spheres_scene_contains_four_objects`.

- [ ] **Step 4: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: 72 tests PASS.

- [ ] **Step 5: Format, render, and commit**

```bash
cargo fmt
cargo run --release
```
Expected: glass on the left refracting the scene behind it, a diffuse blue ball in the middle, a metal ball on the right, on a yellow ground.

```bash
git add -A
git commit -m "feat(ch11): dielectric glass with Schlick reflectance (Ray Tracing in One Weekend §11)"
```

---

## Task 6: Positionable camera and the full CLI (§12)

**Files:**
- Modify: `src/camera.rs`, `src/cli.rs`, `src/renderer.rs`, `src/error.rs`, `src/scene.rs`, `src/main.rs`, `src/lib.rs`

**Interfaces:**
- Consumes: `lib::degrees_to_radians`, `rng::Rng`, `scene::Scene`
- Produces:
  - `Camera` becomes positionable: public `aspect_ratio`, `vfov`, `lookfrom`, `lookat`, `vup`; `image_height` derived from `image_width / aspect_ratio` (floored at 1)
  - `camera::Camera::new(image_width, samples_per_pixel, max_depth, aspect_ratio, vfov, lookfrom, lookat, vup) -> Self`
  - `camera::Camera::chapter_twelve() -> Self` — the book's §12 listing
  - `Camera::get_ray(&self, i: usize, j: usize, du: f64, dv: f64) -> Ray`
  - `cli::Options { pub width: usize, pub samples_per_pixel: usize, pub seed: u64, pub scene: &'static str, pub out: String, pub help: bool }`
  - `cli::DEFAULT_SAMPLES`, `cli::DEFAULT_SEED`, `cli::DEFAULT_WIDTH`
  - `scene::SCENE_NAMES: &[&str]`
  - `RenderError::UnknownScene { name: String }`

**This is a breaking change to `Camera::new`'s arity.** Update every call site, including `renderer.rs` and the `camera.rs` tests.

- [ ] **Step 1: Make `Camera` positionable**

Replace the struct definition and constructor in `src/camera.rs`:

```rust
/// A pinhole camera (§7), positionable from §12.
pub struct Camera {
    pub image_width: usize,
    pub samples_per_pixel: usize,
    pub max_depth: usize,
    pub aspect_ratio: f64,
    pub vfov: f64,
    pub lookfrom: Vec3,
    pub lookat: Vec3,
    pub vup: Vec3,

    pub image_height: usize,
    pixel00_loc: Vec3,
    pixel_delta_u: Vec3,
    pixel_delta_v: Vec3,
    u: Vec3,
    v: Vec3,
    w: Vec3,
}

impl Camera {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        image_width: usize,
        samples_per_pixel: usize,
        max_depth: usize,
        aspect_ratio: f64,
        vfov: f64,
        lookfrom: Vec3,
        lookat: Vec3,
        vup: Vec3,
    ) -> Self {
        // §12: height follows from the aspect ratio, floored at 1.
        let image_height = ((image_width as f64 / aspect_ratio) as usize).max(1);

        // Camera frame (§12).
        let focal_length = (lookfrom - lookat).length();
        let theta = crate::degrees_to_radians(vfov);
        let h = (theta / 2.0).tan();
        let viewport_height = 2.0 * h * focal_length;
        let viewport_width = viewport_height * (image_width as f64 / image_height as f64);

        let w = (lookfrom - lookat).unit();
        let u = vup.cross(w).unit();
        let v = w.cross(u);

        let viewport_u = viewport_width * u;
        let viewport_v = viewport_height * -v;

        let pixel_delta_u = viewport_u / image_width as f64;
        let pixel_delta_v = viewport_v / image_height as f64;
        let viewport_upper_left = lookfrom - focal_length * w - viewport_u / 2.0 - viewport_v / 2.0;
        let pixel00_loc = viewport_upper_left + 0.5 * (pixel_delta_u + pixel_delta_v);

        Self {
            image_width,
            samples_per_pixel,
            max_depth,
            aspect_ratio,
            vfov,
            lookfrom,
            lookat,
            vup,
            image_height,
            pixel00_loc,
            pixel_delta_u,
            pixel_delta_v,
            u,
            v,
            w,
        }
    }

    /// The §12 listing: 400px wide, 16:9, a 90-degree lens pulled back.
    pub fn chapter_twelve() -> Self {
        Self::new(
            400,
            100,
            50,
            16.0 / 9.0,
            90.0,
            Vec3::new(-2.0, 2.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
        )
    }
```

Replace `render` and `get_ray`:

```rust
    pub fn render(&self, world: &dyn Hittable, rng: &mut Rng) -> Result<Image, RenderError> {
        let mut image = Image::new(self.image_width, self.image_height)?;

        for j in 0..self.image_height {
            eprint!("\rScanlines remaining: {} ", self.image_height - j);
            for i in 0..self.image_width {
                let mut color = Vec3::ZERO;
                for _ in 0..self.samples_per_pixel {
                    let du = rng.range(0.0, 1.0);
                    let dv = rng.range(0.0, 1.0);
                    let ray = self.get_ray(i, j, du, dv);
                    color += self.ray_color(&ray, self.max_depth, world, rng);
                }
                image.set(i, j, (color / self.samples_per_pixel as f64).to_rgb());
            }
        }
        eprintln!("\rDone.                 ");

        Ok(image)
    }

    /// A camera ray through pixel (i, j), offset by the (du, dv) sample.
    fn get_ray(&self, i: usize, j: usize, du: f64, dv: f64) -> Ray {
        let pixel_sample = self.pixel00_loc
            + self.pixel_delta_u * (i as f64 + du)
            + self.pixel_delta_v * (j as f64 + dv);
        Ray::new(self.lookfrom, pixel_sample - self.lookfrom)
    }
```

Remove the now-unused `origin`, `horizontal`, `vertical`, and `lower_left_corner` fields.

- [ ] **Step 2: Update the `camera.rs` tests for the new arity**

Replace the test module's helpers and tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hittable::HittableList;
    use crate::scene::{self, Scene};

    fn test_camera(width: usize, samples: usize, depth: usize, aspect: f64) -> Camera {
        Camera::new(
            width,
            samples,
            depth,
            aspect,
            90.0,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
        )
    }

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
        let camera = test_camera(20, 1, 2, 20.0 / 12.0);
        let image = camera.render(&world, &mut Rng::seed_from_u64(1)).unwrap();
        assert_eq!(image.pixels.len(), 20 * 12);
        assert_eq!(image.width, 20);
        assert_eq!(image.height, 12);
    }

    #[test]
    fn max_depth_zero_yields_black() {
        let (_objects, world) = test_world();
        let camera = test_camera(8, 1, 0, 8.0 / 5.0);
        let image = camera.render(&world, &mut Rng::seed_from_u64(1)).unwrap();
        assert!(image.pixels.iter().all(|p| p == &[0.0, 0.0, 0.0]));
    }

    #[test]
    fn height_follows_from_the_aspect_ratio() {
        let camera = Camera::chapter_twelve();
        assert_eq!(camera.image_width, 400);
        // 400 / (16/9) = 225
        assert_eq!(camera.image_height, 225);
    }

    #[test]
    fn camera_frame_vectors_are_orthonormal() {
        let camera = Camera::chapter_twelve();
        let (u, v, w) = (camera.u, camera.v, camera.w);
        assert!((u.length() - 1.0).abs() < 1e-9);
        assert!((v.length() - 1.0).abs() < 1e-9);
        assert!((w.length() - 1.0).abs() < 1e-9);
        assert!(u.dot(v).abs() < 1e-9);
        assert!(u.dot(w).abs() < 1e-9);
        assert!(v.dot(w).abs() < 1e-9);
    }

    #[test]
    fn aspect_ratio_never_yields_a_zero_height() {
        let camera = test_camera(1, 1, 1, 1000.0);
        assert_eq!(camera.image_height, 1);
    }

    #[test]
    fn a_narrow_field_of_view_yields_a_smaller_viewport() {
        let lookfrom = Vec3::new(-2.0, 2.0, 1.0);
        let lookat = Vec3::new(0.0, 0.0, -1.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let narrow = Camera::new(400, 1, 1, 16.0 / 9.0, 20.0, lookfrom, lookat, vup);
        let wide = Camera::new(400, 1, 1, 16.0 / 9.0, 90.0, lookfrom, lookat, vup);
        assert!((narrow.pixel_delta_u.length() - wide.pixel_delta_u.length()).abs() > 1e-9);
    }
}
```

The last test needs `pixel_delta_u` to be visible to the test module — it is, since tests are a child module and the field is private to `camera`, which is the same module tree.

- [ ] **Step 3: Add `--scene` to the CLI**

In `src/scene.rs`, add:

```rust
/// Names accepted by `--scene`.
pub const SCENE_NAMES: &[&str] = &["spheres"];
```

In `src/error.rs`, add the variant and its arms:

```rust
    UnknownScene { name: String },
```

```rust
            Self::UnknownScene { name } => write!(
                f,
                "unknown scene: {name} (valid: {})",
                crate::scene::SCENE_NAMES.join(", ")
            ),
```

```rust
            | Self::UnknownFlag { .. }
            | Self::UnknownScene { .. } => None,
```

Replace `src/cli.rs` in full:

```rust
//! Command-line argument parsing.

use crate::error::RenderError;
use crate::scene::SCENE_NAMES;

/// Where renders go when `--out` is omitted.
pub const DEFAULT_OUTPUT: &str = "images/rtiow.ppm";

/// The book's chapter-12/13 setup, used when no flags are given.
pub const DEFAULT_WIDTH: usize = 400;
pub const DEFAULT_SAMPLES: usize = 100;
pub const DEFAULT_SEED: u64 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub width: usize,
    pub samples_per_pixel: usize,
    pub seed: u64,
    pub scene: String,
    pub out: String,
    pub help: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            samples_per_pixel: DEFAULT_SAMPLES,
            seed: DEFAULT_SEED,
            scene: SCENE_NAMES[0].to_string(),
            out: DEFAULT_OUTPUT.to_string(),
            help: false,
        }
    }
}

impl Options {
    pub fn parse<I: Iterator<Item = String>>(args: I) -> Result<Self, RenderError> {
        let mut opts = Self::default();
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => opts.help = true,
                "--width" => {
                    opts.width = parse_usize(&mut args, "--width")?;
                }
                "--samples" => {
                    opts.samples_per_pixel = parse_usize(&mut args, "--samples")?;
                }
                "--seed" => {
                    opts.seed = parse_u64(&mut args, "--seed")?;
                }
                "--scene" => {
                    let v = next_value(&mut args, "--scene")?;
                    if !SCENE_NAMES.contains(&v.as_str()) {
                        return Err(RenderError::UnknownScene { name: v });
                    }
                    opts.scene = v;
                }
                "--out" => {
                    opts.out = next_value(&mut args, "--out")?;
                }
                other => return Err(RenderError::UnknownFlag { flag: other.to_string() }),
            }
        }

        if opts.width == 0 {
            return Err(RenderError::InvalidDimension { width: 0, height: 225 });
        }
        if opts.samples_per_pixel == 0 {
            return Err(RenderError::InvalidParameter {
                flag: "--samples".to_string(),
                value: opts.samples_per_pixel.to_string(),
            });
        }

        Ok(opts)
    }
}

fn next_value<I: Iterator<Item = String>>(
    args: &mut std::iter::Peekable<I>,
    flag: &str,
) -> Result<String, RenderError> {
    args.next()
        .ok_or_else(|| RenderError::MissingValue { flag: flag.to_string() })
}

fn parse_usize<I: Iterator<Item = String>>(
    args: &mut std::iter::Peekable<I>,
    flag: &str,
) -> Result<usize, RenderError> {
    let v = next_value(args, flag)?;
    v.parse()
        .map_err(|_| RenderError::InvalidParameter { flag: flag.to_string(), value: v })
}

fn parse_u64<I: Iterator<Item = String>>(
    args: &mut std::iter::Peekable<I>,
    flag: &str,
) -> Result<u64, RenderError> {
    let v = next_value(args, flag)?;
    v.parse()
        .map_err(|_| RenderError::InvalidParameter { flag: flag.to_string(), value: v })
}

pub const USAGE: &str = "\
grazing-incidence — a ray tracer

USAGE:
    grazing-incidence [OPTIONS]

OPTIONS:
    --width <N>     Image width in pixels     [default: 400]
    --samples <N>   Samples per pixel         [default: 100]
    --seed <N>      PRNG seed                 [default: 0]
    --scene <NAME>  Scene to render           [default: spheres]
    --out <PATH>    Output path               [default: images/rtiow.ppm]
    --help, -h      Print this message

Height is derived from a 16:9 aspect ratio, so --width 400 gives 400x225.
";

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, RenderError> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_match_the_book() {
        let opts = parse(&[]).unwrap();
        assert_eq!(opts.width, 400);
        assert_eq!(opts.samples_per_pixel, 100);
        assert_eq!(opts.seed, 0);
        assert_eq!(opts.scene, "spheres");
        assert_eq!(opts.out, "images/rtiow.ppm");
        assert!(!opts.help);
    }

    #[test]
    fn reads_every_flag() {
        let opts = parse(&[
            "--width", "800", "--samples", "16", "--seed", "7",
            "--scene", "spheres", "--out", "images/x.ppm",
        ])
        .unwrap();
        assert_eq!(opts.width, 800);
        assert_eq!(opts.samples_per_pixel, 16);
        assert_eq!(opts.seed, 7);
        assert_eq!(opts.scene, "spheres");
        assert_eq!(opts.out, "images/x.ppm");
    }

    #[test]
    fn help_short_and_long() {
        assert!(parse(&["--help"]).unwrap().help);
        assert!(parse(&["-h"]).unwrap().help);
    }

    #[test]
    fn rejects_an_unknown_flag() {
        let err = parse(&["--nope"]).unwrap_err();
        assert!(matches!(err, RenderError::UnknownFlag { ref flag } if flag == "--nope"));
    }

    #[test]
    fn rejects_an_unknown_scene() {
        let err = parse(&["--scene", "nope"]).unwrap_err();
        assert!(matches!(err, RenderError::UnknownScene { ref name } if name == "nope"));
    }

    #[test]
    fn rejects_a_flag_with_no_value() {
        assert!(matches!(parse(&["--width"]).unwrap_err(), RenderError::MissingValue { .. }));
    }

    #[test]
    fn rejects_non_numeric_values() {
        assert!(matches!(
            parse(&["--seed", "abc"]).unwrap_err(),
            RenderError::InvalidParameter { .. }
        ));
    }

    #[test]
    fn rejects_zero_width() {
        assert!(matches!(
            parse(&["--width", "0"]).unwrap_err(),
            RenderError::InvalidDimension { .. }
        ));
    }

    #[test]
    fn rejects_zero_samples() {
        assert!(matches!(
            parse(&["--samples", "0"]).unwrap_err(),
            RenderError::InvalidParameter { .. }
        ));
    }
}
```

- [ ] **Step 4: Rewrite `renderer.rs` to accept options**

Replace `src/renderer.rs` in full:

```rust
//! Rendering entry point.

use crate::camera::Camera;
use crate::error::RenderError;
use crate::hittable::{Hittable, HittableList};
use crate::image::Image;
use crate::rng::Rng;
use crate::scene::{self, Scene};
use crate::vec3::Vec3;

/// Whether to apply the sqrt gamma encode when writing pixels (§9).
pub const GAMMA: bool = true;

/// The seed every committed image is rendered with, so reruns are identical.
pub const DEFAULT_SEED: u64 = 0;

pub fn render_scene(opts: &crate::cli::Options) -> Result<Image, RenderError> {
    let objects = scene::build(Scene::Spheres);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    let camera = Camera::chapter_twelve();
    let mut rng = Rng::seed_from_u64(opts.seed);
    camera.render(&world, &mut rng)
}
```

`GAMMA` and `DEFAULT_SEED` are now duplicated in `cli.rs`. **Remove them from `renderer.rs`** and read the seed from `opts.seed`; keep `GAMMA` in `renderer.rs` only.

Update the `renderer.rs` test module to the new signature:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Options;

    #[test]
    fn same_seed_produces_an_identical_image() {
        let a = render_scene(&Options::default()).unwrap();
        let b = render_scene(&Options::default()).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }

    #[test]
    fn different_seeds_produce_different_images() {
        let mut x = Options::default();
        x.seed = 1;
        let mut y = Options::default();
        y.seed = 2;
        assert_ne!(render_scene(&x).unwrap().pixels, render_scene(&y).unwrap().pixels);
    }
}
```

The second test is slow at 400x225x100 samples. Use a small width:

```rust
    #[test]
    fn different_seeds_produce_different_images() {
        let mut x = Options::default();
        x.width = 40;
        x.seed = 1;
        let mut y = Options::default();
        y.width = 40;
        y.seed = 2;
        assert_ne!(render_scene(&x).unwrap().pixels, render_scene(&y).unwrap().pixels);
    }
```

- [ ] **Step 5: Update `main.rs`**

Replace `src/main.rs` in full:

```rust
use std::path::Path;

use grazing_incidence::cli::{self, Options};
use grazing_incidence::error::RenderError;
use grazing_incidence::renderer;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match Options::parse(args.into_iter()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}\n\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };

    if opts.help {
        print!("{}", cli::USAGE);
        return;
    }

    if let Err(e) = run(&opts) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(opts: &Options) -> Result<(), RenderError> {
    let image = renderer::render_scene(opts)?;

    let path = Path::new(&opts.out);
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }

    // Until chapter 14 there is only a PPM encoder.
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
        return Err(RenderError::UnsupportedFormat {
            extension: "png".to_string(),
        });
    }
    image.write_ppm(path, renderer::GAMMA)?;

    println!("wrote {}", path.display());
    Ok(())
}
```

Add to `src/error.rs`:

```rust
    UnsupportedFormat { extension: String },
```

```rust
            Self::UnsupportedFormat { extension } => write!(
                f,
                "unsupported output format: {extension} (PNG output lands in chapter 14)"
            ),
```

and add `| Self::UnsupportedFormat { .. } => None,` to `source`.

- [ ] **Step 6: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests PASS.

- [ ] **Step 7: Format, render, and commit**

```bash
cargo fmt
cargo run --release -- --width 400 --samples 100 --out images/rtiow.ppm
```
Expected: `wrote images/rtiow.ppm`, now framed by the book's §12 camera (90-degree lens from (-2,2,1)).

```bash
git add -A
git commit -m "feat(ch12): positionable camera and complete CLI (Ray Tracing in One Weekend §12)"
```

---

## Task 7: Defocus blur (§13 Defocus Blur)

**Files:**
- Modify: `src/camera.rs`, `src/renderer.rs`

**Interfaces:**
- Consumes: `rng::Rng::in_unit_disk`
- Produces:
  - `Camera` gains public `defocus_angle: f64` and `focus_dist: f64`
  - `Camera::new(...)` gains two trailing parameters
  - `Camera::chapter_thirteen() -> Self` — the book's §13 listing
  - `Camera::defocus_disk_sample(&self, rng: &mut Rng) -> Vec3`

- [ ] **Step 1: Add the aperture**

In `src/camera.rs`, add two fields to the struct:

```rust
    pub defocus_angle: f64,
    pub focus_dist: f64,
```

and two private fields:

```rust
    defocus_disk_u: Vec3,
    defocus_disk_v: Vec3,
```

- [ ] **Step 2: Extend the constructor**

Change the signature to take `defocus_angle` and `focus_dist` after `vup`:

```rust
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        image_width: usize,
        samples_per_pixel: usize,
        max_depth: usize,
        aspect_ratio: f64,
        vfov: f64,
        lookfrom: Vec3,
        lookat: Vec3,
        vup: Vec3,
        defocus_angle: f64,
        focus_dist: f64,
    ) -> Self {
```

and add the defocus disk computation after the viewport setup:

```rust
        // §13: the defocus disk is the lens aperture. Rays start anywhere on it,
        // which spreads out-of-focus rays exactly as a real lens would.
        let defocus_radius = focus_dist * (crate::degrees_to_radians(defocus_angle / 2.0)).tan();
        let defocus_disk_u = u * defocus_radius;
        let defocus_disk_v = v * defocus_radius;
```

and include them in the returned `Self`:

```rust
            defocus_angle,
            focus_dist,
            defocus_disk_u,
            defocus_disk_v,
```

- [ ] **Step 3: Add the §13 constructor**

```rust
    /// The §13 listing: a large 10-degree aperture focused at 3.4.
    pub fn chapter_thirteen() -> Self {
        Self::new(
            400,
            100,
            50,
            16.0 / 9.0,
            20.0,
            Vec3::new(-2.0, 2.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
            10.0,
            3.4,
        )
    }
```

- [ ] **Step 4: Sample the disk in `get_ray`**

```rust
    /// A camera ray through pixel (i, j), offset by the (du, dv) sample.
    fn get_ray(&self, i: usize, j: usize, du: f64, dv: f64, rng: &mut Rng) -> Ray {
        let pixel_sample = self.pixel00_loc
            + self.pixel_delta_u * (i as f64 + du)
            + self.pixel_delta_v * (j as f64 + dv);

        // A zero aperture collapses the disk to a point, which is a pinhole.
        let ray_origin = if self.defocus_angle <= 0.0 {
            self.lookfrom
        } else {
            self.defocus_disk_sample(rng)
        };

        Ray::new(ray_origin, pixel_sample - ray_origin)
    }

    /// A random point on the defocus disk (§13).
    fn defocus_disk_sample(&self, rng: &mut Rng) -> Vec3 {
        let p = rng.in_unit_disk();
        self.lookfrom + self.defocus_disk_u * p.x + self.defocus_disk_v * p.y
    }
```

Update the call site in `render`:

```rust
                    let du = rng.range(0.0, 1.0);
                    let dv = rng.range(0.0, 1.0);
                    let ray = self.get_ray(i, j, du, dv, rng);
```

- [ ] **Step 5: Update every `Camera::new` call site**

`chapter_twelve` gains two trailing arguments:

```rust
    pub fn chapter_twelve() -> Self {
        Self::new(
            400,
            100,
            50,
            16.0 / 9.0,
            90.0,
            Vec3::new(-2.0, 2.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
            0.0,
            10.0,
        )
    }
```

The test helper `test_camera` gains `0.0, 10.0`:

```rust
    fn test_camera(width: usize, samples: usize, depth: usize, aspect: f64) -> Camera {
        Camera::new(
            width,
            samples,
            depth,
            aspect,
            90.0,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
            0.0,
            10.0,
        )
    }
```

And the two inline calls inside
`a_narrow_field_of_view_yields_a_smaller_viewport` gain `0.0, 10.0` as well:

```rust
    #[test]
    fn a_narrow_field_of_view_yields_a_smaller_viewport() {
        let lookfrom = Vec3::new(-2.0, 2.0, 1.0);
        let lookat = Vec3::new(0.0, 0.0, -1.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let narrow = Camera::new(
            400, 1, 1, 16.0 / 9.0, 20.0, lookfrom, lookat, vup, 0.0, 10.0,
        );
        let wide = Camera::new(
            400, 1, 1, 16.0 / 9.0, 90.0, lookfrom, lookat, vup, 0.0, 10.0,
        );
        assert!((narrow.pixel_delta_u.length() - wide.pixel_delta_u.length()).abs() > 1e-9);
    }
```

- [ ] **Step 6: Add defocus tests**

```rust
    #[test]
    fn zero_aperture_collapses_the_disk_to_a_point() {
        let camera = Camera::chapter_twelve();
        let mut rng = Rng::seed_from_u64(1);
        for _ in 0..100 {
            assert_eq!(camera.defocus_disk_sample(&mut rng), camera.lookfrom);
        }
    }

    #[test]
    fn a_wide_aperture_spreads_ray_origins_over_the_disk() {
        let camera = Camera::chapter_thirteen();
        let mut rng = Rng::seed_from_u64(1);
        let center = camera.lookfrom;
        let mut max_distance = 0.0f64;
        for _ in 0..1_000 {
            let d = (camera.defocus_disk_sample(&mut rng) - center).length();
            max_distance = max_distance.max(d);
        }
        let radius = (camera.defocus_disk_u.length() + camera.defocus_disk_v.length()) / 2.0;
        assert!(max_distance > 0.0 && max_distance <= radius + 1e-9);
    }

    #[test]
    fn defocus_radius_follows_the_aperture_angle() {
        let lookfrom = Vec3::new(-2.0, 2.0, 1.0);
        let lookat = Vec3::new(0.0, 0.0, -1.0);
        let vup = Vec3::new(0.0, 1.0, 0.0);
        let small = Camera::new(400, 1, 1, 16.0 / 9.0, 20.0, lookfrom, lookat, vup, 2.0, 3.4);
        let large = Camera::new(400, 1, 1, 16.0 / 9.0, 20.0, lookfrom, lookat, vup, 20.0, 3.4);
        assert!(large.defocus_disk_u.length() > small.defocus_disk_u.length());
    }
```

- [ ] **Step 7: Switch the renderer to the §13 camera**

In `src/renderer.rs`:

```rust
    let camera = Camera::chapter_thirteen();
```

- [ ] **Step 8: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests PASS.

- [ ] **Step 9: Format, render, and commit**

```bash
cargo fmt
cargo run --release -- --out images/rtiow.ppm
```
Expected: the four spheres with the background out of focus and the near ground sharp.

```bash
git add -A
git commit -m "feat(ch13): thin-lens defocus blur (Ray Tracing in One Weekend §13)"
```

---

## Task 8: PNG output (ours)

**Files:**
- Create: `src/png.rs`
- Modify: `src/lib.rs`, `src/image.rs`, `src/main.rs`, `src/error.rs`

**Interfaces:**
- Consumes: `Image`
- Produces:
  - `png::crc32(&[u8]) -> u32`
  - `png::adler32(&[u8]) -> u32`
  - `png::deflate(&[u8]) -> Vec<u8>` — a complete zlib stream (header, DEFLATE, Adler-32)
  - `png::encode_rgb(width: u32, height: u32, rgb: &[u8]) -> Vec<u8>`
  - `Image::write_png(&self, path: &Path, gamma: bool) -> Result<(), RenderError>`
  - `Image::to_rgb_bytes(&self, gamma: bool) -> Vec<u8>`

This is the one piece of code the book does not teach. The render is unchanged; only the encoder changes.

**Why this matters:** PPM images are ~1.1 MB and GitHub will not preview them. PNG at this size is roughly 40–60 KB and renders inline, which is the difference between a repo people can look at and one they cannot.

**Why it is here at all:** the spec's decision 8 accepts it as deliberate scope beyond the book. If the DEFLATE encoder proves troublesome, fall back to *stored* (uncompressed) DEFLATE blocks: same valid PNG, larger file, about 15 lines instead of 150. Do not abandon PNG.

- [ ] **Step 1: Write `src/png.rs` — checksums first**

```rust
//! A minimal PNG encoder: CRC-32, Adler-32, and a DEFLATE compressor.
//!
//! Written from the format specifications. No crates.

const CRC_TABLE: [u32; 256] = build_crc_table();

const fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

/// CRC-32 as used by PNG chunks (IEEE 802.3 polynomial, reflected).
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &byte in data {
        c = CRC_TABLE[((c ^ byte as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Adler-32 as used by the zlib wrapper.
pub fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
```

- [ ] **Step 2: Add the bit writer and fixed-Huffman DEFLATE**

Append to `src/png.rs`:

```rust
/// Accumulates a DEFLATE bit stream. DEFLATE packs bits least-significant
/// first within each byte; Huffman codes are written most-significant bit
/// first.
struct BitWriter {
    out: Vec<u8>,
    bit_buffer: u32,
    bit_count: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self { out: Vec::new(), bit_buffer: 0, bit_count: 0 }
    }

    fn write_bits(&mut self, value: u32, count: u32) {
        self.bit_buffer |= value << self.bit_count;
        self.bit_count += count;
        while self.bit_count >= 8 {
            self.out.push((self.bit_buffer & 0xFF) as u8);
            self.bit_buffer >>= 8;
            self.bit_count -= 8;
        }
    }

    /// Huffman codes are stored MSB-first, so reverse before writing.
    fn write_code(&mut self, code: u32, length: u32) {
        let mut reversed = 0;
        for i in 0..length {
            reversed |= ((code >> (length - 1 - i)) & 1) << i;
        }
        self.write_bits(reversed, length);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bit_count > 0 {
            self.out.push((self.bit_buffer & 0xFF) as u8);
        }
        self.out
    }
}

/// The literal/length alphabet of DEFLATE's fixed Huffman code (RFC 1951 §3.2.6).
fn write_fixed_literal(w: &mut BitWriter, symbol: u32) {
    match symbol {
        0..=143 => w.write_code(0b0011_0000 + symbol, 8),
        144..=255 => w.write_code(0b1_1001_0000 + (symbol - 144), 9),
        256..=279 => w.write_code(symbol - 256, 7),
        _ => w.write_code(0b1100_0000 + (symbol - 280), 8),
    }
}

/// Length codes 257..285: the base length each code represents, and how many
/// extra bits follow it.
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Distance codes 0..29, and their extra-bit counts.
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Greedy LZ77 match finder over a hash table of 3-byte sequences.
///
/// This is not a sophisticated compressor. One candidate position per hash
/// bucket keeps it around 120 lines, and on rendered images it still gets
/// within a small factor of a full chain search, because runs of identical
/// pixels make the single candidate correct almost every time.
const HASH_BITS: usize = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const MAX_CHAIN: usize = 32;
const WINDOW: usize = 32768;

fn hash3(data: &[u8], pos: usize) -> usize {
    let a = data[pos] as usize;
    let b = data[pos + 1] as usize;
    let c = data[pos + 2] as usize;
    ((a << 10) ^ (b << 5) ^ c) & (HASH_SIZE - 1)
}

/// Compress `data` into a complete zlib stream: 2-byte header, DEFLATE
/// blocks with a fixed Huffman code, then the Adler-32 checksum.
pub fn deflate(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    out.extend_from_slice(&deflate_raw(data));
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// The raw DEFLATE stream, as it appears inside a PNG IDAT chunk.
pub fn deflate_raw(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    w.write_bits(1, 1); // BFINAL
    w.write_bits(1, 2); // BTYPE = 01, fixed Huffman

    let mut head = vec![usize::MAX; HASH_SIZE];
    let mut prev = vec![usize::MAX; data.len().max(1)];

    let mut pos = 0usize;
    while pos < data.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;

        if pos + MIN_MATCH <= data.len() {
            let h = hash3(data, pos);
            let mut candidate = head[h];
            let limit = pos.saturating_sub(WINDOW);
            let mut chain = 0;

            while candidate != usize::MAX && candidate >= limit && chain < MAX_CHAIN {
                chain += 1;
                let max_len = MAX_MATCH.min(data.len() - pos);
                let mut len = 0;
                while len < max_len && data[candidate + len] == data[pos + len] {
                    len += 1;
                }
                if len > best_len {
                    best_len = len;
                    best_dist = pos - candidate;
                    if len == MAX_MATCH {
                        break;
                    }
                }
                let next = prev[candidate];
                if next >= candidate {
                    break;
                }
                candidate = next;
            }

            prev[pos] = head[h];
            head[h] = pos;
        }

        if best_len >= MIN_MATCH {
            let length_index = LENGTH_BASE
                .iter()
                .rposition(|&b| b as usize <= best_len)
                .expect("every length >= 3 has a code");
            write_fixed_literal(&mut w, 257 + length_index as u32);
            let extra = LENGTH_EXTRA[length_index];
            if extra > 0 {
                let base = LENGTH_BASE[length_index];
                w.write_bits((best_len - base as usize) as u32, extra);
            }

            let dist_index = DIST_BASE
                .iter()
                .rposition(|&b| b as usize <= best_dist)
                .expect("every distance >= 1 has a code");
            w.write_code(dist_index as u32, 5);
            let dextra = DIST_EXTRA[dist_index];
            if dextra > 0 {
                w.write_bits((best_dist - DIST_BASE[dist_index] as usize) as u32, dextra);
            }

            // Index the bytes the match consumed so later matches can find them.
            for k in (pos + 1)..(pos + best_len) {
                if k + MIN_MATCH <= data.len() {
                    let h = hash3(data, k);
                    prev[k] = head[h];
                    head[h] = k;
                }
            }
            pos += best_len;
        } else {
            write_fixed_literal(&mut w, data[pos] as u32);
            pos += 1;
        }
    }

    write_fixed_literal(&mut w, 256); // end of block
    w.finish()
}
```

- [ ] **Step 3: Add the PNG chunking**

Append to `src/png.rs`:

```rust
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    let mut body = Vec::with_capacity(4 + payload.len());
    body.extend_from_slice(kind);
    body.extend_from_slice(payload);

    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// Encode 8-bit RGB pixels as a complete PNG file.
///
/// `rgb` must be exactly `width * height * 3` bytes, row-major.
pub fn encode_rgb(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(rgb.len(), (width as usize) * (height as usize) * 3);

    // Prefix every scanline with filter type 0 (None).
    let mut raw = Vec::with_capacity((width as usize * 3 + 1) * height as usize);
    let stride = width as usize * 3;
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * stride..(y + 1) * stride]);
    }

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(2); // colour type 2 = truecolour RGB
    ihdr.push(0); // compression method: DEFLATE
    ihdr.push(0); // filter method
    ihdr.push(0); // no interlace
    chunk(&mut out, b"IHDR", &ihdr);

    chunk(&mut out, b"IDAT", &deflate(&raw));
    chunk(&mut out, b"IEND", &[]);

    out
}
```

- [ ] **Step 4: Write `src/png.rs` tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_reference_check_value() {
        // The standard CRC-32 check value for "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn adler32_matches_the_reference_check_value() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn deflate_starts_with_a_zlib_header_and_checksum() {
        let out = deflate(b"hello hello hello");
        assert_eq!(&out[..2], &[0x78, 0x01]);
        assert_eq!(&out[out.len() - 4..], &adler32(b"hello hello hello").to_be_bytes());
    }

    #[test]
    fn deflate_really_compresses_runs() {
        let data = vec![7u8; 10_000];
        let out = deflate(&data);
        assert!(out.len() < 200, "compressed {} bytes from 10000", out.len());
    }

    #[test]
    fn deflate_handles_empty_input() {
        let out = deflate(&[]);
        assert_eq!(&out[..2], &[0x78, 0x01]);
        assert!(out.len() > 4);
    }

    #[test]
    fn deflate_handles_incompressible_input_without_corrupting_it() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i.wrapping_mul(2654435761) >> 16) as u8).collect();
        let out = deflate(&data);
        assert_eq!(&out[out.len() - 4..], &adler32(&data).to_be_bytes());
    }

    #[test]
    fn encode_rgb_has_a_valid_png_signature() {
        let png = encode_rgb(1, 1, &[255, 0, 0]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    #[test]
    fn encode_rgb_emits_ihdr_idat_iend_in_order() {
        let png = encode_rgb(4, 4, &vec![128; 4 * 4 * 3]);
        let text = String::from_utf8_lossy(&png).to_string();
        let ihdr = text.find("IHDR").unwrap();
        let idat = text.find("IDAT").unwrap();
        let iend = text.find("IEND").unwrap();
        assert!(ihdr < idat && idat < iend);
    }

    #[test]
    fn ihdr_carries_the_requested_dimensions() {
        let png = encode_rgb(300, 150, &vec![0; 300 * 150 * 3]);
        let ihdr_pos = png
            .windows(4)
            .position(|w| w == b"IHDR")
            .expect("IHDR chunk present");
        // Chunk layout: 4-byte length, then "IHDR", then width and height.
        let width = u32::from_be_bytes(png[ihdr_pos + 4..ihdr_pos + 8].try_into().unwrap());
        let height = u32::from_be_bytes(png[ihdr_pos + 8..ihdr_pos + 12].try_into().unwrap());
        assert_eq!(width, 300);
        assert_eq!(height, 150);
    }

    #[test]
    fn every_chunk_carries_a_matching_crc() {
        let png = encode_rgb(4, 4, &vec![10; 4 * 4 * 3]);
        let mut pos = 8;
        while pos + 8 <= png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let body = &png[pos + 4..pos + 4 + 4 + len];
            let expected = u32::from_be_bytes(
                png[pos + 4 + 4 + len..pos + 8 + 4 + len].try_into().unwrap(),
            );
            assert_eq!(crc32(body), expected, "bad CRC on chunk at offset {pos}");
            pos += 4 + 4 + len + 4;
        }
        assert_eq!(pos, png.len(), "chunks must tile the file exactly");
    }

    #[test]
    fn a_rendered_image_compresses_well() {
        // Flat runs, like a real render's sky.
        let mut rgb = Vec::new();
        for _ in 0..225 {
            rgb.extend_from_slice(&[90, 140, 220].repeat(400));
        }
        let png = encode_rgb(400, 225, &rgb);
        assert!(
            png.len() < rgb.len() / 10,
            "png {} bytes from rgb {}",
            png.len(),
            rgb.len()
        );
    }
}
```

- [ ] **Step 5: Register `png` and add `write_png` to `Image`**

Add to `src/lib.rs`:

```rust
pub mod png;
```

In `src/image.rs`, refactor the byte conversion out of `write_ppm` and add `write_png`:

```rust
    /// The gamma-encoded 8-bit RGB payload both encoders share.
    pub fn to_rgb_bytes(&self, gamma: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 3);
        for px in &self.pixels {
            for &c in px {
                let v = if gamma { c.sqrt() } else { c };
                out.push((256.0 * v.clamp(0.0, 0.999)) as u8);
            }
        }
        out
    }

    pub fn write_ppm(&self, path: &Path, gamma: bool) -> Result<(), RenderError> {
        let header = format!("P6\n{} {}\n255\n", self.width, self.height);
        let mut bytes = header.into_bytes();
        bytes.extend_from_slice(&self.to_rgb_bytes(gamma));
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Encode as PNG. Same pixels as `write_ppm`, roughly a twentieth of the
    /// size, and previewable on GitHub.
    pub fn write_png(&self, path: &Path, gamma: bool) -> Result<(), RenderError> {
        let rgb = self.to_rgb_bytes(gamma);
        let png = crate::png::encode_rgb(self.width as u32, self.height as u32, &rgb);
        std::fs::write(path, png)?;
        Ok(())
    }
```

- [ ] **Step 6: Route the extension in `main.rs`**

Replace the `run` body in `src/main.rs`:

```rust
fn run(opts: &Options) -> Result<(), RenderError> {
    let image = renderer::render_scene(opts)?;

    let path = Path::new(&opts.out);
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }

    let is_png = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));

    if is_png {
        image.write_png(path, renderer::GAMMA)?;
    } else {
        image.write_ppm(path, renderer::GAMMA)?;
    }

    println!("wrote {}", path.display());
    Ok(())
}
```

Remove `RenderError::UnsupportedFormat` and its arms — the encoder now exists, so the error can no longer occur.

- [ ] **Step 7: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests PASS, including the two checksum reference values. If `crc32` or `adler32` disagree with their published check values, the tables are wrong — fix that before anything else.

- [ ] **Step 8: Verify a real PNG opens**

```bash
cargo run --release -- --out images/ch14-defocus.png
```

Then open `images/ch14-defocus.png` in an image viewer. It must look identical to the PPM of Task 7. If the viewer rejects it, the DEFLATE stream is malformed — the most likely cause is a Huffman code written in the wrong bit order.

- [ ] **Step 9: Commit**

```bash
cargo fmt
git add -A
git commit -m "feat(ch14): hand-rolled PNG encoder with fixed-Huffman DEFLATE"
```

---

## Task 9: The cover scene and the final image

**Files:**
- Modify: `src/scene.rs`, `src/renderer.rs`, `src/cli.rs`

**Interfaces:**
- Consumes: everything above
- Produces:
  - `scene::Scene::Cover` and `scene::SCENE_NAMES = &["spheres", "cover"]`
  - `scene::build(Scene::Cover, rng: &mut Rng) -> Vec<Box<dyn Hittable>>`
  - `camera::Camera::cover() -> Self` — the book's §14 listing

**Note:** `build` now needs an `&mut Rng`, because the cover scene places spheres randomly. The spheres scene ignores it. This is the one signature change this plan makes to a settled API.

- [ ] **Step 1: Add the cover scene**

In `src/scene.rs`:

```rust
use crate::material::Material;
use crate::rng::Rng;
use crate::vec3::{Color, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    Spheres,
    /// The book's cover render: hundreds of random spheres (§14).
    Cover,
}

pub const SCENE_NAMES: &[&str] = &["spheres", "cover"];

/// Build the objects for a scene. Returns owned boxes so the caller can keep
/// them alive while `HittableList` borrows them.
///
/// `rng` seeds the cover scene's sphere placement. Because the seed is fixed,
/// the cover scene is reproducible like every other render.
pub fn build(scene: Scene, rng: &mut Rng) -> Vec<Box<dyn Hittable>> {
    match scene {
        Scene::Spheres => build_spheres(),
        Scene::Cover => build_cover(rng),
    }
}

fn build_spheres() -> Vec<Box<dyn Hittable>> {
    let ground = Sphere::new(
        Vec3::new(0.0, -1000.0, 0.0),
        1000.0,
        Material::lambertian(Vec3::new(0.8, 0.8, 0.0)),
    );
    let left = Sphere::new(
        Vec3::new(-1.0, 0.0, -1.0),
        0.5,
        Material::dielectric(1.5),
    );
    let middle = Sphere::new(
        Vec3::new(0.0, 0.0, -1.0),
        0.5,
        Material::lambertian(Vec3::new(0.1, 0.2, 0.5)),
    );
    let right = Sphere::new(
        Vec3::new(1.0, 0.0, -1.0),
        0.5,
        Material::metal(Vec3::new(0.8, 0.6, 0.2), 0.0),
    );
    vec![
        Box::new(ground),
        Box::new(left),
        Box::new(middle),
        Box::new(right),
    ]
}

/// The book's final render (Listing 88): a field of randomly placed spheres,
/// plus three deliberate foreground spheres.
fn build_cover(rng: &mut Rng) -> Vec<Box<dyn Hittable>> {
    let mut objects: Vec<Box<dyn Hittable>> = Vec::new();

    let ground = Sphere::new(
        Vec3::new(0.0, -1000.0, 0.0),
        1000.0,
        Material::lambertian(Vec3::new(0.5, 0.5, 0.5)),
    );
    objects.push(Box::new(ground));

    for a in -11..11 {
        for b in -11..11 {
            let choose_mat = rng.f64();
            let center = Vec3::new(
                a as f64 + 0.9 * rng.f64(),
                0.2,
                b as f64 + 0.9 * rng.f64(),
            );

            // Leave a gap so the foreground spheres are visible.
            if (center - Vec3::new(4.0, 0.2, 0.0)).length() <= 0.9 {
                continue;
            }

            let sphere = if choose_mat < 0.8 {
                Sphere::new(center, 0.2, Material::lambertian(Color::random(rng) * Color::random(rng)))
            } else if choose_mat < 0.95 {
                let albedo = Color::random_range(rng, 0.5, 1.0);
                let fuzz = rng.range(0.0, 0.5);
                Sphere::new(center, 0.2, Material::metal(albedo, fuzz))
            } else {
                Sphere::new(center, 0.2, Material::dielectric(1.5))
            };
            objects.push(Box::new(sphere));
        }
    }

    objects.push(Box::new(Sphere::new(
        Vec3::new(0.0, 1.0, 0.0),
        1.0,
        Material::dielectric(1.5),
    )));
    objects.push(Box::new(Sphere::new(
        Vec3::new(-4.0, 1.0, 0.0),
        1.0,
        Material::lambertian(Vec3::new(0.4, 0.2, 0.1)),
    )));
    objects.push(Box::new(Sphere::new(
        Vec3::new(4.0, 1.0, 0.0),
        1.0,
        Material::metal(Vec3::new(0.7, 0.6, 0.5), 0.0),
    )));

    objects
}
```

Update the existing `scene.rs` tests for the new `build` signature:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hittable::{Hittable, HittableList};
    use crate::ray::Ray;

    #[test]
    fn spheres_scene_contains_four_objects() {
        assert_eq!(build(Scene::Spheres, &mut Rng::seed_from_u64(0)).len(), 4);
    }

    #[test]
    fn spheres_scene_hits_the_ball() {
        let objects = build(Scene::Spheres, &mut Rng::seed_from_u64(0));
        let mut world = HittableList::new();
        for o in &objects {
            world.add(o.as_ref());
        }

        // A ray straight down from above the middle sphere must hit it.
        let ray = Ray::new(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let rec = world.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(rec.t < 5.0);
    }

    #[test]
    fn cover_scene_is_reproducible_for_a_given_seed() {
        let a = build(Scene::Cover, &mut Rng::seed_from_u64(0));
        let b = build(Scene::Cover, &mut Rng::seed_from_u64(0));
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn cover_scene_places_many_spheres() {
        let objects = build(Scene::Cover, &mut Rng::seed_from_u64(0));
        assert!(objects.len() > 300, "only {} spheres", objects.len());
    }

    #[test]
    fn different_seeds_place_the_cover_scene_differently() {
        // `build` returns trait objects, so compare a rendered ray rather than
        // the objects themselves: different placements give different hits.
        let hit_t = |seed: u64| {
            let objects = build(Scene::Cover, &mut Rng::seed_from_u64(seed));
            let mut world = HittableList::new();
            for o in &objects {
                world.add(o.as_ref());
            }
            let ray = Ray::new(
                Vec3::new(13.0, 2.0, 3.0),
                Vec3::new(-13.0, -2.0, -3.0).unit(),
            );
            world.hit(&ray, 0.001, f64::INFINITY).map(|r| r.t)
        };
        assert_ne!(hit_t(1), hit_t(2));
    }
}
```

- [ ] **Step 2: Add the cover camera**

In `src/camera.rs`:

```rust
    /// The §14 listing: 1200px wide, 500 samples, a narrow 20-degree lens with
    /// a small aperture. Expect this to take hours.
    pub fn cover() -> Self {
        Self::new(
            1200,
            500,
            50,
            16.0 / 9.0,
            20.0,
            Vec3::new(13.0, 2.0, 3.0),
            Vec3::ZERO,
            Vec3::new(0.0, 1.0, 0.0),
            0.6,
            10.0,
        )
    }
```

- [ ] **Step 3: Wire the scene name through the renderer**

In `src/cli.rs`, replace the `scene: String` field with a typed one:

```rust
    pub scene: Scene,
```

and in `parse`:

```rust
                "--scene" => {
                    let v = next_value(&mut args, "--scene")?;
                    opts.scene = Scene::parse(&v)?;
                }
```

In `src/scene.rs`:

```rust
impl Scene {
    pub fn parse(name: &str) -> Result<Self, crate::error::RenderError> {
        match name {
            "spheres" => Ok(Self::Spheres),
            "cover" => Ok(Self::Cover),
            other => Err(crate::error::UnknownScene { name: other.to_string() }),
        }
    }
}
```

Change `UnknownScene` in `src/error.rs` to a tuple variant holding only the name:

```rust
    UnknownScene(String),
```

```rust
            Self::UnknownScene(name) => write!(
                f,
                "unknown scene: {name} (valid: {})",
                crate::scene::SCENE_NAMES.join(", ")
            ),
```

```rust
            | Self::UnknownScene(_) => None,
```

In `src/renderer.rs`:

```rust
use crate::scene::{self, Scene};

pub fn render_scene(opts: &crate::cli::Options) -> Result<Image, RenderError> {
    // Scene construction and the render share one generator, so the cover
    // scene's sphere layout is reproducible from the seed.
    let mut rng = Rng::seed_from_u64(opts.seed);
    let objects = scene::build(opts.scene, &mut rng);
    let mut world = HittableList::new();
    for object in &objects {
        world.add(object.as_ref());
    }

    let camera = match opts.scene {
        Scene::Spheres => Camera::chapter_thirteen(),
        Scene::Cover => Camera::cover(),
    };
    camera.render(&world, &mut rng)
}
```

**Sharing one generator between scene construction and rendering is a deliberate choice.** It means the same seed reproduces both the layout and the image, which is what the determinism test asserts.

Update the `cli.rs` test that asserted `opts.scene == "spheres"` to compare against
`Scene::Spheres`:

```rust
    #[test]
    fn defaults_match_the_book() {
        let opts = parse(&[]).unwrap();
        assert_eq!(opts.width, 400);
        assert_eq!(opts.samples_per_pixel, 100);
        assert_eq!(opts.seed, 0);
        assert_eq!(opts.scene, Scene::Spheres);
        assert_eq!(opts.out, "images/rtiow.ppm");
        assert!(!opts.help);
    }
```

and add `use crate::scene::Scene;` to that test module.

`camera.rs`'s `test_world` helper also calls the old one-argument `build`. Update it:

```rust
    fn test_world() -> (Vec<Box<dyn Hittable>>, HittableList<'_>) {
        let objects = scene::build(Scene::Spheres, &mut Rng::seed_from_u64(1));
        let mut list = HittableList::new();
        for o in &objects {
            list.add(o.as_ref());
        }
        (objects, list)
    }
```

The `renderer.rs` test module already calls `render_scene(&Options::default())` and needs
no change beyond what Step 3 writes.

- [ ] **Step 4: Test**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test`
Expected: all tests PASS. The `different_seeds_produce_different_images` test may now be slow — if it exceeds a minute, drop `samples_per_pixel` to 4 in that test only.

- [ ] **Step 5: Render the fast image, then the cover**

First the four-sphere scene, which takes seconds:

```bash
cargo fmt
cargo run --release -- --scene spheres --out images/ch13-defocus.png
```

Then the cover render. **This takes hours on a CPU.** Start it and let it run:

```bash
cargo run --release -- --scene cover --out images/cover.png
```

Do not block on this. If it is not finished, commit the code now and render the cover later — the code is the deliverable, and the seed guarantees the image is reproducible whenever you have time for it.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(ch14): cover scene with hundreds of random spheres (Ray Tracing in One Weekend §14)"
```

---

## Final Verification

Run after every task is merged:

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo build --release
cargo run --release -- --scene spheres --out images/final-check.png
```

Then confirm:

1. `images/final-check.png` opens and matches the four-sphere defocus image.
2. `git status` is clean.
3. `Cargo.toml` `[dependencies]` is still empty.
4. `docs/superpowers/specs/2026-10-10-grazing-incidence-design.md` is unmodified.
5. The book markdown is still untracked: `git ls-files | Select-String -Pattern "\.md$"` must not list it.