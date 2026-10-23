//! Seeded capped Pareto shocks for reproducible counterfactual stress tests.
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
pub struct Fortuna {
    rng: ChaCha8Rng,
    alpha: f64,
    scale: f64,
    cap: f64,
}
impl Fortuna {
    pub fn new(seed: u64, alpha: f64, scale: f64, cap: f64) -> Result<Self, &'static str> {
        if !alpha.is_finite()
            || alpha <= 0.0
            || !scale.is_finite()
            || scale <= 0.0
            || !cap.is_finite()
            || cap < scale
        {
            return Err("invalid Pareto shock parameters");
        }
        Ok(Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            alpha,
            scale,
            cap,
        })
    }
    pub fn sample(&mut self) -> f64 {
        let u = self.rng.random::<f64>();
        (self.scale / (1.0 - u).powf(1.0 / self.alpha)).min(self.cap)
    }
    pub fn signed(&mut self) -> f64 {
        let sign = if self.rng.random::<bool>() { 1.0 } else { -1.0 };
        sign * self.sample()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shocks_are_reproducible_bounded_and_heavy_tailed() {
        let mut a = Fortuna::new(7, 1.5, 1.0, 1000.0).unwrap();
        let mut b = Fortuna::new(7, 1.5, 1.0, 1000.0).unwrap();
        let mut tail = 0;
        for _ in 0..10000 {
            let x = a.sample();
            assert_eq!(x, b.sample());
            assert!((1.0..=1000.0).contains(&x));
            if x > 10.0 {
                tail += 1;
            }
        }
        assert!(tail > 100);
        assert!(Fortuna::new(0, 0.0, 1.0, 10.0).is_err());
    }
}
