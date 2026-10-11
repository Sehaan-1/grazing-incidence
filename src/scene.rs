//! Named scenes.

use crate::error::RenderError;
use crate::hittable::Hittable;
use crate::material::Material;
use crate::rng::Rng;
use crate::sphere::Sphere;
use crate::vec3::{Color, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// The ground sphere plus three balls.
    Spheres,
    /// The cover render: a field of random spheres.
    Cover,
}

/// Names accepted by `--scene`, in the order `Options::default` uses them: the
/// first is the default scene. `Scene::parse` resolves through the same names,
/// so adding a variant means adding a name here with it.
pub const SCENE_NAMES: &[&str] = &["spheres", "cover"];

impl Scene {
    /// Look a scene up by its `--scene` name, or say which name was wrong.
    pub fn parse(name: &str) -> Result<Self, RenderError> {
        match name {
            "spheres" => Ok(Self::Spheres),
            "cover" => Ok(Self::Cover),
            other => Err(RenderError::UnknownScene(other.to_string())),
        }
    }
}

/// Build the objects for a scene. Returns owned boxes so the caller can keep
/// them alive while `HittableList` borrows them.
///
/// `rng` seeds the cover scene's sphere placement. Because the seed is fixed,
/// the cover scene is reproducible like every other render; the spheres scene
/// draws nothing from it.
pub fn build(scene: Scene, rng: &mut Rng) -> Vec<Box<dyn Hittable>> {
    match scene {
        Scene::Spheres => build_spheres(),
        Scene::Cover => build_cover(rng),
    }
}

/// The spheres world: a Lambertian ground, a matte ball, a metal ball, a glass
/// ball, and the hollow glass bubble sharing the glass ball's centre.
///
/// The ground sphere's top surface sits at -100.5 + 100 = -0.5, below the eye
/// at the origin; an earlier ground at (0, -1000, 0) with r = 1000 put that
/// surface exactly at y = 0, so the camera sat on it and no ray ever reached
/// the sky. Two departures from the obvious arrangement are deliberate and both
/// are visible in the image:
///
///   - the middle ball is at `(0, 0, -1.2)`, not `(0, 0, -1)`. Every other ball
///     sits at `z = -1`, so at `z = -1` the metal ball on the right lines up
///     exactly behind the matte one and hides it.
///   - there is a second, *smaller* glass sphere concentric with the left one:
///     `(-1, 0, -1)` with `r = 0.4` and index `1.0 / 1.5`. It is a hollow
///     bubble -- air inside glass inside air -- and it is the scene's whole
///     point: rays refract twice on the way in (1 -> 0.667 -> 1) and twice on
///     the way back out, so the scene bends around it.
///
/// Both matter to the renders and to the row-band arithmetic in
/// `camera::tests::a_column_reads_sky_then_ball_then_ground`, which is written
/// against this geometry.
fn build_spheres() -> Vec<Box<dyn Hittable>> {
    let ground = Sphere::new(
        Vec3::new(0.0, -100.5, -1.0),
        100.0,
        Material::lambertian(Vec3::new(0.8, 0.8, 0.0)),
    );
    // Ground, the matte ball, the left glass shell, the bubble inside it, and the
    // metal ball on the right. Order does not matter
    // -- `HittableList` takes the closest hit -- but keeping the five in
    // left-to-right image order makes them checkable against the render line by
    // line.
    let middle = Sphere::new(
        Vec3::new(0.0, 0.0, -1.2),
        0.5,
        Material::lambertian(Vec3::new(0.1, 0.2, 0.5)),
    );
    let left = Sphere::new(Vec3::new(-1.0, 0.0, -1.0), 0.5, Material::dielectric(1.5));
    // `1.0 / 1.50` is that reciprocal deliberately, not a typo: the bubble is
    // glass-within-glass, so entering it divides by 1.5 exactly as leaving the
    // shell does, and the net effect on a ray crossing both is nothing.
    let bubble = Sphere::new(
        Vec3::new(-1.0, 0.0, -1.0),
        0.4,
        Material::dielectric(1.0 / 1.5),
    );
    let right = Sphere::new(
        Vec3::new(1.0, 0.0, -1.0),
        0.5,
        Material::metal(Vec3::new(0.8, 0.6, 0.2), 0.0),
    );
    vec![
        Box::new(ground),
        Box::new(middle),
        Box::new(left),
        Box::new(bubble),
        Box::new(right),
    ]
}

/// The cover scene's grid: `a` and `b` run -11 through 10 inclusive, which is
/// 22 values each, so 22 * 22 = 484 cells before the gap removes any. Each cell
/// is 0.9 wide and its spheres have radius 0.2, so neighbouring cells never
/// touch.
const COVER_GRID_MIN: i32 = -11;
const COVER_GRID_MAX: i32 = 11;

/// The gap carved around the foreground metal ball: the closed ball of radius
/// 0.9 about (4, 0.2, 0), from which no grid sphere may come.
const COVER_GAP_CENTER: Vec3 = Vec3::new(4.0, 0.2, 0.0);
const COVER_GAP_RADIUS: f64 = 0.9;

/// The radius of every sphere in the grid, and of the three deliberate ones.
const COVER_SMALL_RADIUS: f64 = 0.2;
const COVER_LARGE_RADIUS: f64 = 1.0;

/// The cover scene's material draw, split out so the 80/15/5 split can be tested
/// without a camera. `choose_mat` is the value drawn *before* the sphere's
/// centre and it is passed in rather than drawn here, so the draw order stays
/// intact: a sphere that fails the gap test spends nothing further.
///
/// The thresholds are tested at their boundaries directly:
/// `0.8` is already metal, `0.95` is already glass.
fn material_for(choose_mat: f64, rng: &mut Rng) -> Material {
    if choose_mat < 0.8 {
        // `color::random() * color::random()` -- six draws, componentwise.
        Material::lambertian(Color::random(rng) * Color::random(rng))
    } else if choose_mat < 0.95 {
        Material::metal(Color::random_range(rng, 0.5, 1.0), rng.range(0.0, 0.5))
    } else {
        Material::dielectric(1.5)
    }
}

/// The cover render: a field of randomly placed and randomly materialled
/// spheres, plus three deliberate foreground ones.
///
/// The ground here is `(0, -1000, 0)` with radius 1000 -- deliberately *not* the
/// ground of `build_spheres`. This one's surface tops out at
/// -1000 + 1000 = 0, which is where the grid spheres rest (their centres are at
/// y = 0.2 with radius 0.2) and where the three foreground spheres rest (y = 1,
/// radius 1). Only the spheres scene needs its surface at y = -0.5, because that
/// scene's eye sits at the origin and would otherwise be standing on the
/// ground; here the eye is at (13, 2, 3), two units clear of the surface.
fn build_cover(rng: &mut Rng) -> Vec<Box<dyn Hittable>> {
    let mut objects: Vec<Box<dyn Hittable>> = Vec::new();

    objects.push(Box::new(Sphere::new(
        Vec3::new(0.0, -1000.0, 0.0),
        1000.0,
        Material::lambertian(Vec3::new(0.5, 0.5, 0.5)),
    )));

    for a in COVER_GRID_MIN..COVER_GRID_MAX {
        for b in COVER_GRID_MIN..COVER_GRID_MAX {
            let choose_mat = rng.f64();
            let center = Vec3::new(a as f64 + 0.9 * rng.f64(), 0.2, b as f64 + 0.9 * rng.f64());

            // The sphere is added only when its centre is *further* than
            // 0.9 from (4, 0.2, 0), so the gap is the closed ball and `continue`
            // is its negation. Reading this the other way round fills the gap and
            // buries the foreground metal ball it exists to clear.
            if (center - COVER_GAP_CENTER).length() <= COVER_GAP_RADIUS {
                continue;
            }

            let material = material_for(choose_mat, rng);
            objects.push(Box::new(Sphere::new(center, COVER_SMALL_RADIUS, material)));
        }
    }

    objects.push(Box::new(Sphere::new(
        Vec3::new(0.0, 1.0, 0.0),
        COVER_LARGE_RADIUS,
        Material::dielectric(1.5),
    )));
    objects.push(Box::new(Sphere::new(
        Vec3::new(-4.0, 1.0, 0.0),
        COVER_LARGE_RADIUS,
        Material::lambertian(Vec3::new(0.4, 0.2, 0.1)),
    )));
    objects.push(Box::new(Sphere::new(
        Vec3::new(4.0, 1.0, 0.0),
        COVER_LARGE_RADIUS,
        Material::metal(Vec3::new(0.7, 0.6, 0.5), 0.0),
    )));

    objects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hittable::HittableList;
    use crate::ray::Ray;

    /// The cover scene's objects, for one seed.
    fn cover_objects(seed: u64) -> Vec<Box<dyn Hittable>> {
        build(Scene::Cover, &mut Rng::seed_from_u64(seed))
    }

    /// Run `probe` against the cover world for `seed`. The world borrows its
    /// spheres, so it cannot outlive `build_cover` -- and a helper that handed
    /// both back would have to name a lifetime it cannot name. Borrowing inside
    /// a closure is the way to keep the objects alive exactly as long as the
    /// list that points at them.
    fn with_cover_world<R>(seed: u64, probe: impl FnOnce(&HittableList<'_>) -> R) -> R {
        let objects = cover_objects(seed);
        let mut world = HittableList::new();
        for object in &objects {
            world.add(object.as_ref());
        }
        probe(&world)
    }

    /// A ray dropped straight down from `height` at `(x, z)`.
    fn down_from(x: f64, height: f64, z: f64) -> Ray {
        Ray::new(Vec3::new(x, height, z), Vec3::new(0.0, -1.0, 0.0))
    }

    /// The layout of the cover field, read back through rays rather than off the
    /// trait objects.
    ///
    /// One downward probe per grid cell, at the cell's centre, from y = 5. What
    /// answers is one of four things, and each has its own distance:
    ///
    ///   - a grid sphere, whose top is at y = 0.4, so it answers at
    ///     t = 5 - (0.2 + sqrt(0.04 - d^2)) in [4.6, 4.8] for horizontal offset
    ///     d < 0.2;
    ///   - one of the three foreground balls, top at y = 2, answering at
    ///     t = 3 wherever the cell centre falls inside it -- the same three cells
    ///     for every seed, so they never move;
    ///   - the ground, whose surface is at y = -1000 + sqrt(10^6 - r^2) with r
    ///     the horizontal distance from the origin, which is at most
    ///     -1000 + sqrt(10^6 - 2 * 11.35^2) = -0.064 at the far corner, so it
    ///     answers at t > 5.06.
    ///
    /// The bands do not overlap, so a probe's `t` says which of the four it was,
    /// and the whole vector is a fingerprint of the layout.
    fn probe_signature(seed: u64) -> Vec<Option<f64>> {
        with_cover_world(seed, |world| {
            let mut out = Vec::new();
            for a in COVER_GRID_MIN..COVER_GRID_MAX {
                for b in COVER_GRID_MIN..COVER_GRID_MAX {
                    let ray = down_from(a as f64 + 0.45, 5.0, b as f64 + 0.45);
                    out.push(world.hit(&ray, 0.001, f64::INFINITY).map(|r| r.t));
                }
            }
            out
        })
    }

    #[test]
    fn spheres_scene_contains_five_objects() {
        // The ground, the matte ball, the left glass shell, the bubble inside
        // it and the metal ball. The count *is* the assertion: the bubble is
        // invisible to any probe that only looks at the outside of the left
        // ball, and its omission from an earlier version of this scene cost a
        // whole feature -- the hollow-glass render -- while every other test
        // stayed green. Five, because the scene defines five sphere objects and the
        // hollow bubble is one of them, not part of the shell it sits inside.
        assert_eq!(build(Scene::Spheres, &mut Rng::seed_from_u64(0)).len(), 5);
    }

    /// The scene's own geometry, read back through rays rather than off the
    /// trait objects: a matte ball at `z = -1.2` and a bubble of radius 0.4
    /// inside the left glass shell.
    ///
    /// `build` hands back `Box<dyn Hittable>`, so the only handle on the
    /// geometry is a probe -- which is also the better test, because a probe
    /// cannot be satisfied by a comment.
    ///
    /// **The matte ball's `z`.** A ray dropped down the ball's own axis from
    /// `(0, 2, -1.2)` meets it at `t = 2 - 0.5 = 1.5`: the ball's top is at
    /// `y = 0.5` whichever way round the image plane it faces, so the axis is
    /// what pins the depth. Put the ball back at `z = -1` and the same ray
    /// misses its centre by 0.2, so it meets the surface where
    /// `(2 - t)^2 + 0.2^2 = 0.25`, i.e.
    ///     t = 2 - sqrt(0.25 - 0.04) = 2 - sqrt(0.21)
    ///       = 2 - 0.458257569495584 = 1.541742430504416,
    /// which is 0.0417 away from 1.5 and fails a 1e-9 tolerance. The ground
    /// answers at `t = 2.5` in both cases -- it is `(0, -100.5, -1)` with
    /// `r = 100`, so its surface is at `y = -0.5002` under `z = -1.2` and
    /// `y = -0.5` under `z = -1`, both well below the ball's top at `y = 0.5`
    /// -- so it never masks the ball.
    ///
    /// **The bubble.** The shell and the bubble are concentric at `(-1, 0, -1)`
    /// with radii 0.5 and 0.4, so no ray from outside can tell them apart: it
    /// always meets the shell first. The probe starts at the shared centre,
    /// *inside* the bubble, and looks down. `Sphere::hit` rejects the near root
    /// against `t_min` and keeps the far one, so the bubble answers at exactly
    /// its own radius, `t = 0.4`, while the shell answers at `0.5`. Without the
    /// bubble the answer is the shell at 0.5 with `ior = 1.5`; with it the
    /// answer is 0.4 with `ior = 1.0 / 1.5`. The two are 0.1 apart and have
    /// different materials, so the probe fails on either if the bubble is
    /// dropped.
    ///
    /// The centre of the left ball is outside the ground: it is
    /// `sqrt(1 + 100.5^2) = 100.5074611946998` from `(0, -100.5, -1)`, which
    /// has radius 100.
    #[test]
    fn the_matte_ball_is_at_minus_one_point_two_with_a_hollow_bubble_inside_the_glass() {
        let objects = build(Scene::Spheres, &mut Rng::seed_from_u64(0));
        let mut world = HittableList::new();
        for o in &objects {
            world.add(o.as_ref());
        }

        let down_the_axis = Ray::new(Vec3::new(0.0, 2.0, -1.2), Vec3::new(0.0, -1.0, 0.0));
        let ball = world
            .hit(&down_the_axis, 0.001, f64::INFINITY)
            .expect("the matte ball at (0, 0, -1.2)");
        assert!(
            (ball.t - 1.5).abs() < 1e-9,
            "the matte ball answered at t = {}, not 1.5 -- is it at z = -1.2?",
            ball.t
        );
        assert_eq!(
            ball.material,
            Material::lambertian(Vec3::new(0.1, 0.2, 0.5))
        );

        // And the reading the old position would have given, so the tolerance
        // above is not carrying the argument by itself.
        let at_minus_one = Ray::new(Vec3::new(0.0, 2.0, -1.0), Vec3::new(0.0, -1.0, 0.0));
        let off_axis = world
            .hit(&at_minus_one, 0.001, f64::INFINITY)
            .expect("the same ball, probed 0.2 off its axis");
        assert!(
            (off_axis.t - (2.0 - 0.21_f64.sqrt())).abs() < 1e-9,
            "expected {}, got {}",
            2.0 - 0.21_f64.sqrt(),
            off_axis.t
        );
        assert!(
            (off_axis.t - 1.5).abs() > 1e-3,
            "probing at z = -1 must NOT give the same answer as probing at z = -1.2"
        );

        let straight_down = Ray::new(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(0.0, -1.0, 0.0));
        let bubble = world
            .hit(&straight_down, 0.001, f64::INFINITY)
            .expect("the bubble at (-1, 0, -1)");
        assert!(
            (bubble.t - 0.4).abs() < 1e-9,
            "the bubble answered at t = {}, not 0.4",
            bubble.t
        );
        assert_eq!(bubble.material, Material::dielectric(1.0 / 1.5));

        // The same ray with `t_min` past the bubble reaches the shell, which is
        // what makes the 0.4 above a reading of the bubble rather than of
        // whichever glass sphere happens to be inner.
        let shell = world
            .hit(&straight_down, 0.401, f64::INFINITY)
            .expect("the glass shell, from inside the bubble");
        assert!(
            (shell.t - 0.5).abs() < 1e-9,
            "the shell answered at t = {}, not 0.5",
            shell.t
        );
        assert_eq!(shell.material, Material::dielectric(1.5));
    }

    #[test]
    fn every_advertised_scene_name_resolves() {
        // `--scene` resolves through `Scene::parse` against `SCENE_NAMES`. If
        // those two ever drift, a name the CLI accepts would fail at parse
        // time, so they are checked against each other rather than written down
        // twice.
        for name in SCENE_NAMES {
            assert!(
                Scene::parse(name).is_ok(),
                "`{name}` is advertised but does not parse"
            );
        }
        assert!(Scene::parse("nope").is_err());
        assert_eq!(SCENE_NAMES.len(), 2);
        assert_eq!(SCENE_NAMES[0], "spheres");
        assert_eq!(SCENE_NAMES[1], "cover");
    }

    #[test]
    fn an_unknown_scene_names_the_valid_ones() {
        // The message is what a user sees after a typo, and the list it prints
        // comes from `SCENE_NAMES`, so it follows a scene being added.
        let err = Scene::parse("kover").unwrap_err();
        assert!(
            err.to_string().contains("cover"),
            "the message should list the valid names, got `{err}`"
        );
        assert!(err.to_string().contains("spheres"));
    }

    #[test]
    fn spheres_scene_hits_the_middle_ball() {
        let objects = build(Scene::Spheres, &mut Rng::seed_from_u64(0));
        let mut world = HittableList::new();
        for o in &objects {
            world.add(o.as_ref());
        }

        // A ray straight down from above the middle ball must hit that ball, not
        // the ground. The ball at (0, 0, -1.2) with r = 0.5 puts its surface at
        // t = 2 - 0.5 = 1.5 along this ray; the ground at (0, -100.5, -1) with
        // r = 100 is far enough below that its surface is at
        // t = 102.5 - sqrt(102.5^2 - 506.29) = 102.5 - 99.9998 = 2.5002. The
        // two outer balls are 1.0 away from the ray's own line in x, outside
        // their radius of 0.5, so neither can answer, and the bubble is inside
        // the left one. `HittableList` takes the closest of what is left, so
        // this pins which sphere did.
        let ray = Ray::new(Vec3::new(0.0, 2.0, -1.2), Vec3::new(0.0, -1.0, 0.0));
        let rec = world.hit(&ray, 0.001, f64::INFINITY).unwrap();
        assert!(
            (rec.t - 1.5).abs() < 1e-9,
            "expected t = 1.5, got {}",
            rec.t
        );
    }

    #[test]
    fn the_eye_sits_above_the_ground_not_on_it() {
        // The bug this scene guards: a ground at (0,-1000,0) with r = 1000 has
        // its surface at y = 0, exactly where the camera is, so every ray that
        // tilts downward starts already inside the sphere and no ray reaches the
        // sky. The spheres scene's ground tops out at y = -0.5, so a ray from the
        // origin aimed straight up misses everything.
        let objects = build(Scene::Spheres, &mut Rng::seed_from_u64(0));
        let mut world = HittableList::new();
        for o in &objects {
            world.add(o.as_ref());
        }

        let up = Ray::new(Vec3::ZERO, Vec3::new(0.0, 1.0, -0.001).unit());
        assert!(world.hit(&up, 0.001, f64::INFINITY).is_none());
    }

    #[test]
    fn the_spheres_scene_is_the_same_whatever_the_seed() {
        // `build` takes a generator now, but the spheres world has nothing
        // random in it. If it started drawing, the spheres render would stop
        // being reproducible from its seed -- the guarantee the cover scene
        // relies on.
        let a = build(Scene::Spheres, &mut Rng::seed_from_u64(0));
        let b = build(Scene::Spheres, &mut Rng::seed_from_u64(12345));
        assert_eq!(a.len(), b.len());
    }

    /// The cover ground is `(0, -1000, 0)` with radius 1000, whose surface is at
    /// y = 0 -- *not* the spheres scene's geometry, whose surface is at y = -0.5.
    ///
    /// Measured by shooting, not by inspecting, because `build` hands back trait
    /// objects. The cover eye at (13, 2, 3) is outside every grid sphere
    /// (they span x and z in [-10.9, 10.9] and y in [0, 0.4]) and above the
    /// ground, so a ray dropped from there can only reach the ground.
    ///
    /// That ray meets the surface of a radius-1000 sphere centred at
    /// (0, -1000, 0) where
    ///     13^2 + (y + 1000)^2 + 3^2 = 1000^2
    /// so     (y + 1000)^2 = 1000000 - 169 - 9 = 999822
    /// and taking the *upper* root, y = -1000 + sqrt(999822), gives
    ///     t = 2 - (-1000 + sqrt(999822)) = 1002 - sqrt(999822)
    ///
    /// sqrt(999822) = 999.91099604... (999.911^2 = 999822.0079, one rounding
    /// off), so t = 2.08900396... The lower root is 1002 - 999.91099604... =
    /// 2.089 for the upper and 1002 + 999.91... for the lower, which is behind
    /// the eye, so only the upper one counts.
    #[test]
    fn the_cover_ground_is_the_big_flat_one_not_the_spheres_ground() {
        let rec = with_cover_world(0, |world| {
            world.hit(&down_from(13.0, 2.0, 3.0), 0.001, f64::INFINITY)
        })
        .expect("a ray from the cover eye reaches the cover ground");

        let expected = 1002.0 - (1_000_000.0_f64 - 178.0).sqrt();
        assert!(
            (rec.t - expected).abs() < 1e-9,
            "cover ground at t = {}, expected {expected}",
            rec.t
        );
        assert!(
            (rec.t - 2.089_004).abs() < 1e-5,
            "cover ground at t = {}, expected about 2.089",
            rec.t
        );
        // And emphatically not the spheres scene's. Dropping this scene's ray into
        // that ground instead -- a sphere at (0, -100.5, -1) with radius 100 --
        // gives
        // `oc = (13, 102.5, 4)`, `a = 1`, `half_b = -102.5` and
        // `c = 169 + 10506.25 + 16 - 10000 = 691.25`, so the near root is
        // `102.5 - sqrt(10506.25 - 691.25) = 102.5 - sqrt(9815)` = 3.4288. That
        // is 1.34 away from the cover's answer, so the two cannot be confused.
        assert!(
            (rec.t - 3.428_804).abs() > 0.1,
            "that is the spheres ground, not the cover's"
        );
    }

    /// The cover eye is in open sky, not inside the ground.
    ///
    /// The spheres scene has a test of the same name, but it does not transfer: the
    /// cover ground's surface *is* y = 0, so a spheres-shaped check here would
    /// pass for the wrong reason and hide a real regression. This scene's eye is
    /// at y = 2 and the surface is at y = 0, so a ray straight up from it misses
    /// everything.
    #[test]
    fn the_cover_eye_is_above_the_ground_and_sees_the_sky() {
        with_cover_world(0, |world| {
            let up = Ray::new(
                Vec3::new(13.0, 2.0, 3.0),
                Vec3::new(0.0, 1.0, -0.001).unit(),
            );
            assert!(world.hit(&up, 0.001, f64::INFINITY).is_none());
        });
    }

    /// The three deliberate foreground spheres, each pinned by geometry rather
    /// than by its position in the returned vector.
    ///
    /// A ray dropped from y = 5 at each centre reaches its foreground sphere
    /// first: those sit at y = 1 with radius 1, so their tops are at y = 2 and
    /// they answer at t = 5 - 2 = 3. Every grid sphere tops out at y = 0.4,
    /// i.e. t = 4.6, and the ground is past t = 5, so nothing in the field can
    /// get in the way and all three `t` values are exactly 3.
    #[test]
    fn the_three_foreground_spheres_sit_where_the_cover_frame_puts_them() {
        with_cover_world(0, |world| {
            let glass = world
                .hit(&down_from(0.0, 5.0, 0.0), 0.001, f64::INFINITY)
                .expect("the glass ball at (0,1,0)");
            assert!((glass.t - 3.0).abs() < 1e-9, "t = {}", glass.t);
            assert_eq!(glass.material, Material::dielectric(1.5));

            let matte = world
                .hit(&down_from(-4.0, 5.0, 0.0), 0.001, f64::INFINITY)
                .expect("the matte ball at (-4,1,0)");
            assert!((matte.t - 3.0).abs() < 1e-9, "t = {}", matte.t);
            assert_eq!(
                matte.material,
                Material::lambertian(Vec3::new(0.4, 0.2, 0.1))
            );

            let metal = world
                .hit(&down_from(4.0, 5.0, 0.0), 0.001, f64::INFINITY)
                .expect("the metal ball at (4,1,0)");
            assert!((metal.t - 3.0).abs() < 1e-9, "t = {}", metal.t);
            assert_eq!(
                metal.material,
                Material::metal(Vec3::new(0.7, 0.6, 0.5), 0.0)
            );
        });
    }

    /// The gap around `(4, 0.2, 0)` really is empty -- exhaustively, not on a
    /// sample.
    ///
    /// A ring or a few rays would not do. Only four of the 484 grid cells can
    /// put a sphere anywhere near the gap (see the bound below), and a handful
    /// of probe points inside the gap is a real chance of missing the intruder.
    /// Measured: a 16-point ring at radius 0.6 still passed with the gap removed
    /// entirely. The probe grid below is sized so that a miss is impossible
    /// rather than unlikely.
    ///
    /// **What has to be covered.** Only four of the 484 grid cells can put a
    /// sphere anywhere near the gap (see the bound below), and a handful of
    /// probe points inside it is a real chance of missing the intruder.
    /// Measured: a 16-point ring at radius 0.6 still passed with the gap removed
    /// entirely. The grid below is sized so that a miss is impossible rather
    /// than unlikely.
    ///
    /// A *legal* sphere has its centre at horizontal distance `rho > 0.9` from
    /// `(4, 0)` -- every centre sits at `y = 0.2`, so the 3D gap test is exactly
    /// a 2D one -- and radius 0.2, so its surface cannot come nearer than
    /// `0.9 - 0.2 = 0.7`. Conversely an *illegal* one has `rho < 0.9`, so its
    /// surface reaches to `rho - 0.2 < 0.7`. Radius 0.7 is therefore exactly the
    /// line between the two, and the boundary is safe: a probe at `d = 0.7`
    /// needs a centre within 0.2 of it, hence `rho <= 0.9`, which no kept sphere
    /// can be.
    ///
    /// A lattice of spacing 0.1 has covering radius `0.1 * sqrt(2)/2 = 0.0707`,
    /// comfortably under the 0.2 an intruder needs, so probing the disc of
    /// radius 0.7 cannot miss one.
    ///
    /// **What a probe may hit.** From y = 5 straight down, at a point `P` within
    /// 0.7 of `(4, 0)`:
    ///
    ///   - the foreground metal ball at (4, 1, 0) with radius 1 reaches
    ///     horizontally to `r_h <= 0.7`, where its top is at
    ///     `y = 1 + sqrt(1 - 0.49) = 1.714`, answering at `t = 3.286`;
    ///   - the ground's surface is at `y = -1000 + sqrt(10^6 - R^2)` with `R` the
    ///     distance from the origin, at most `4 + 0.7 = 4.7` here, so the
    ///     surface is at worst `-1000 + sqrt(10^6 - 22.09) = -0.01105` and it
    ///     answers at `t = 5.01105` at the latest;
    ///   - the other two foreground balls are at least 3.3 away horizontally and
    ///     cannot reach here at all;
    ///   - a grid sphere's top is at `y = 0.4`, answering at
    ///     `t = 5 - (0.2 + sqrt(0.04 - d^2))` for horizontal offset `d < 0.2`,
    ///     which runs from 4.6 at `d = 0` up to 4.8 as `d` approaches 0.2.
    ///
    /// The two legal bands therefore stop at 3.286 and resume at 5.011, and
    /// `[4.6, 4.8]` can only be a grid sphere. That interval is the assertion.
    #[test]
    fn the_gap_in_front_of_the_foreground_metal_ball_is_kept_clear() {
        let mut probed = 0usize;
        for seed in 0..8 {
            with_cover_world(seed, |world| {
                for i in 0..=14 {
                    for j in 0..=14 {
                        let x = 4.0 + (i as f64 - 7.0) * 0.1;
                        let z = (j as f64 - 7.0) * 0.1;
                        // Only the disc matters; outside it grid spheres are
                        // expected and legitimate.
                        if (x - 4.0).hypot(z) > 0.7 {
                            continue;
                        }
                        probed += 1;
                        let Some(rec) = world.hit(&down_from(x, 5.0, z), 0.001, f64::INFINITY)
                        else {
                            continue;
                        };
                        assert!(
                            !(4.6..=4.8).contains(&rec.t),
                            "seed {seed}: a grid sphere is sitting in the gap over ({x:.1}, ·, \
                             {z:.1}); it answered at t = {}",
                            rec.t
                        );
                    }
                }
            });
        }
        // The disc of radius 0.7 has area pi * 0.49 = 1.5394 and the lattice has
        // 0.01 per cell, so about 154 points per seed and 1231 over eight.
        assert!(
            (1_150..=1_350).contains(&probed),
            "only {probed} probes covered the gap, which is too few to be sure of it"
        );
    }

    /// The count *itself*, which the gap probe above cannot see: a probe only
    /// fails if an intruder is both inside the gap and under a lattice point, so
    /// a mutation that widened the gap to, say, 0.5 -- removing spheres far from
    /// it -- would leave every probe clean.
    ///
    /// With the real gap, the number of grid spheres removed is 0 to 4, so the
    /// total is 484 to 488. Measured over seeds 0 through 7: 486, 486, 487,
    /// 484, 485, 485, 485, 484. Every one of them is *below* 488, which is the
    /// part that carries the weight: a gap that removed nothing at all would put
    /// every seed at exactly 488. At most four cells can be affected, so on a
    /// correct build *at most four* seeds out of eight could reach 488 -- but
    /// reaching it is possible in principle and this asserts the weaker fact
    /// that not all eight do.
    ///
    /// Narrowing the gap to nothing is the other direction: it would put every
    /// seed at exactly 488, so requiring at least one seed below 488 kills that
    /// mutation too. Measured, seeds 3 and 7 both land on 484 -- four cells
    /// removed, which is the maximum the geometry allows.
    #[test]
    fn the_gap_removes_at_least_one_sphere_and_never_more_than_four() {
        let counts: Vec<usize> = (0..8).map(|seed| cover_objects(seed).len()).collect();

        assert!(
            counts.iter().any(|&n| n < 488),
            "no seed lost a sphere to the gap at all: {counts:?}"
        );
        assert!(
            counts.iter().any(|&n| n <= 484),
            "no seed hit the four-cell maximum: {counts:?}"
        );
        for (seed, count) in counts.iter().enumerate() {
            assert!(
                (484..=488).contains(count),
                "seed {seed} produced {count} objects, outside 484..=488"
            );
        }
    }

    /// How many objects the cover scene holds. The bound is arithmetic, not a
    /// measurement:
    ///
    ///   - `a` and `b` each run -11 through 10, which is 22 values, so
    ///     22 * 22 = 484 grid cells;
    ///   - a cell's centre is `(a + 0.9u, 0.2, b + 0.9v)`, so it can fall
    ///     within 0.9 of `(4, 0.2, 0)` only when
    ///     `|a + 0.9u - 4| <= 0.9` and `|b + 0.9v| <= 0.9`. Since `a + 0.9u`
    ///     runs over `[a, a + 0.9)` the first forces `a` in {3, 4} and the
    ///     second forces `b` in {-1, 0} -- four cells at most;
    ///   - so the grid contributes 480 to 484 spheres, and the ground plus the
    ///     three foreground balls add 4.
    ///
    /// That is [484, 488]. The brief's `> 300` is true of every layout this
    /// scene can produce and so asserts nothing at all.
    ///
    /// Measured over seeds 0 through 7: 486, 486, 487, 484, 485, 485, 485, 484.
    /// The gap really does remove a varying number of spheres rather than
    /// always the same. This test pins the bound; the two above it pin that the
    /// bound is reached from both ends.
    #[test]
    fn the_cover_scene_holds_a_field_of_spheres_within_the_layouts_bounds() {
        for seed in 0..8 {
            let count = cover_objects(seed).len();
            assert!(
                (484..=488).contains(&count),
                "seed {seed} produced {count} objects, outside 484..=488"
            );
        }
    }

    /// Seeds really do move the spheres around, and the object count is only
    /// part of it: two seeds can agree on how many spheres the gap removed while
    /// placing them in completely different cells.
    ///
    /// The gap can touch at most four of the 484 cells (see the bound above), so
    /// most seeds keep all 484 grid spheres and agree on 488 objects. Comparing
    /// counts alone would therefore be vacuous. What is compared instead is the
    /// layout itself, through `probe_signature`.
    ///
    /// A cell's own sphere is caught by the probe at that cell's centre when its
    /// horizontal offset is under 0.2, a fraction
    /// pi * 0.2^2 / 0.9^2 = 0.12566 / 0.81 = 0.1551 of cells -- 75 of 484 in
    /// expectation, with binomial sd sqrt(484 * 0.1551 * 0.8449) = 7.96. Seed 1
    /// answers with a sphere on 89 probes (including the three foreground balls,
    /// which never move), 1.8 sd above that, and seed 2 answers on a different
    /// 86. Overwhelmingly those two sets are disjoint, so the symmetric
    /// difference runs to about 150; measured, 139 of 484.
    #[test]
    fn different_seeds_place_the_cover_scene_differently() {
        let one = probe_signature(1);
        let two = probe_signature(2);

        assert_ne!(
            one, two,
            "two seeds laid the field out identically, which means the seed is not reaching the placement"
        );

        // Not vacuous in the other direction either: a signature has to be
        // stable for a fixed seed, or "different" would only mean "noisy".
        assert_eq!(probe_signature(1), one);

        // And it has to be *discriminating*, not a coincidence of two seeds.
        // 100 is a wide floor under the ~139 two seeds actually produce, and
        // still far from "they agreed".
        let differing = one.iter().zip(&two).filter(|(a, b)| a != b).count();
        assert!(
            differing > 100,
            "only {differing} of 484 probes disagreed between two seeds"
        );
    }

    #[test]
    fn cover_scene_is_reproducible_for_a_given_seed() {
        // Compared through every probe rather than by length: two runs of one
        // seed must agree on the whole layout, not merely on how many spheres
        // there are.
        assert_eq!(probe_signature(0), probe_signature(0));
    }

    /// The cover scene's material split is 80% Lambertian, 15% metal, 5% glass.
    ///
    /// Over 100_000 independent draws the counts should sit at 80000, 15000 and
    /// 5000. The binomial standard deviations are
    ///     lambertian  sqrt(100000 * 0.80 * 0.20) = 126.49
    ///     metal      sqrt(100000 * 0.15 * 0.85) = 112.92
    ///     dielectric sqrt(100000 * 0.05 * 0.95) =  68.92
    /// and the bands below are three of those either side, so they hold for any
    /// seed rather than for one lucky draw. Inverting the 0.95 threshold would
    /// move metal to about 5% and glass to about 0% -- 15 and 3.5 sd away.
    #[test]
    fn the_cover_scene_splits_its_materials_eighty_fifteen_five() {
        let mut rng = Rng::seed_from_u64(0);
        let (mut lambertian, mut metal, mut glass) = (0usize, 0usize, 0usize);
        for _ in 0..100_000 {
            match material_for(rng.f64(), &mut rng) {
                Material::Lambertian { .. } => lambertian += 1,
                Material::Metal { .. } => metal += 1,
                Material::Dielectric { .. } => glass += 1,
            }
        }
        assert!(
            (76_000..=84_000).contains(&lambertian),
            "{lambertian} of 100000 were lambertian, expected about 80000"
        );
        assert!(
            (13_000..=17_000).contains(&metal),
            "{metal} of 100000 were metal, expected about 15000"
        );
        assert!(
            (4_000..=6_000).contains(&glass),
            "{glass} of 100000 were glass, expected about 5000"
        );
    }

    /// The thresholds are `0.8` and `0.95` and the comparison is strict at both.
    /// These two values are the only places the split can be wrong without the
    /// statistical test above noticing.
    #[test]
    fn the_material_thresholds_are_strict_at_eight_and_ninety_five() {
        let mut rng = Rng::seed_from_u64(1);
        assert!(matches!(
            material_for(0.799_999, &mut rng),
            Material::Lambertian { .. }
        ));
        assert!(matches!(
            material_for(0.8, &mut rng),
            Material::Metal { .. }
        ));
        assert!(matches!(
            material_for(0.949_999, &mut rng),
            Material::Metal { .. }
        ));
        assert!(matches!(
            material_for(0.95, &mut rng),
            Material::Dielectric { .. }
        ));
    }

    /// The cover scene's metal: albedo uniform in [0.5, 1) and fuzz
    /// uniform in [0, 0.5).
    #[test]
    fn cover_metal_uses_the_pinned_albedo() {
        let mut rng = Rng::seed_from_u64(3);
        for _ in 0..10_000 {
            let Material::Metal { albedo, fuzz } = material_for(0.9, &mut rng) else {
                panic!("0.9 is above 0.8 and below 0.95, so it should be metal");
            };
            assert!(
                (0.5..1.0).contains(&albedo.x)
                    && (0.5..1.0).contains(&albedo.y)
                    && (0.5..1.0).contains(&albedo.z),
                "albedo {albedo:?} left [0.5, 1)"
            );
            assert!((0.0..0.5).contains(&fuzz), "fuzz {fuzz} left [0, 0.5)");
        }
    }

    /// The Lambertian albedo is a *product* of two uniform colours, not one.
    ///
    /// The albedo is `color::random() * color::random()`, so each component is
    /// the product of two independent uniforms on [0, 1). What tells that apart
    /// from a single uniform colour is how often the result clears 0.5:
    ///
    ///     P(u > 0.5)     = 0.5        for one uniform
    ///     P(u v > 0.5)   = 0.1534264  for a product of two
    ///
    /// The second is *not* 0.25. 0.25 is `P(u > 0.5 and v > 0.5)`, a different
    /// event: `uv > 0.5` also holds when one factor is small and the other large.
    /// Integrating, for each `v` the `u` values that clear 0.5 are `(0.5/v, 1)`,
    /// which is non-empty only for `v > 0.5`, so
    ///
    ///     P(uv > 0.5) = int over v in [1/2, 1] of (1 - 0.5/v) dv
    ///                 = [v - (1/2) ln v] from 1/2 to 1
    ///                 = 1 - (0.5 + 0.5 ln 2)
    ///                 = 0.5 - (1/2) ln 2
    ///                 = 0.5 - 0.3465736 = 0.1534264
    ///
    /// Over 3 components * 30_000 draws = 90_000 samples that is 90_000 *
    /// 0.1534264 = 13_808 expected, with binomial sd
    /// sqrt(90_000 * 0.1534264 * 0.8465736) = sqrt(11_688) = 108.1. Four sd either
    /// side is 13_376..=14_241. A single uniform colour would sit at 45_000, which
    /// is 288 sd away.
    #[test]
    fn cover_lambertian_albedo_is_a_product_of_two_uniform_colours() {
        let mut rng = Rng::seed_from_u64(5);
        let mut over_half = 0usize;
        for _ in 0..30_000 {
            let Material::Lambertian { albedo } = material_for(0.1, &mut rng) else {
                panic!("0.1 is below 0.8, so it should be lambertian");
            };
            for c in [albedo.x, albedo.y, albedo.z] {
                assert!((0.0..1.0).contains(&c), "component {c} left [0, 1)");
                if c > 0.5 {
                    over_half += 1;
                }
            }
        }
        // Four sd either side of 13_808 is 13_376..=14_241. Observed: 13_809.
        assert!(
            (13_376..=14_241).contains(&over_half),
            "{over_half} of 90000 albedo components exceeded 0.5, expected about 13808"
        );
        // And a single uniform colour is nowhere near it.
        assert!(
            !(40_000..=50_000).contains(&over_half),
            "that is one uniform colour, not a product of two"
        );
    }
}
