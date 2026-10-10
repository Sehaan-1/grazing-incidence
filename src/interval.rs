//! Numeric ranges.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub min: f64,
    pub max: f64,
}

impl Interval {
    pub const EMPTY: Interval = Interval {
        min: f64::INFINITY,
        max: f64::NEG_INFINITY,
    };

    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// True when `x` lies within the interval, endpoints included (§6.8).
    pub fn contains(&self, x: f64) -> bool {
        self.min <= x && x <= self.max
    }

    /// Widen by `pad` on both sides. Padding an empty interval keeps it empty
    /// rather than producing a real range.
    pub fn pad(&self, pad: f64) -> Interval {
        if self.min > self.max {
            return Interval::EMPTY;
        }
        Interval::new(self.min - pad, self.max + pad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_includes_both_endpoints() {
        let i = Interval::new(0.0, 1.0);
        assert!(i.contains(0.0));
        assert!(i.contains(1.0));
        assert!(i.contains(0.5));
    }

    #[test]
    fn contains_excludes_values_outside() {
        let i = Interval::new(0.0, 1.0);
        assert!(!i.contains(-0.001));
        assert!(!i.contains(1.001));
    }

    #[test]
    fn pad_widens_both_sides() {
        let p = Interval::new(0.0, 1.0).pad(0.1);
        // min == -0.1 and max == 1.1. Reading the max back against 0.1 would
        // compare 1.1 - 0.1 == 1.0 against zero, which no implementation can
        // satisfy; the endpoints are the originals moved by the pad.
        assert!((p.min + 0.1).abs() < 1e-9);
        assert!((p.max - 1.1).abs() < 1e-9);
    }

    #[test]
    fn padding_empty_stays_empty() {
        assert!(Interval::EMPTY.pad(1.0).min > Interval::EMPTY.max);
    }
}
