//! PCG32, a small hand-rolled generator with a reproducible seed.

use crate::vec3::Vec3;

const MULTIPLIER: u64 = 6364136223846793005;

#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    pub fn seed_from_u64(seed: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (seed << 1) | 1,
        };
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
            let p = Vec3::new(self.range(-1.0, 1.0), self.range(-1.0, 1.0), 0.0);
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
            assert!(
                count > 9_000 && count < 11_000,
                "buckets skewed: {buckets:?}"
            );
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
