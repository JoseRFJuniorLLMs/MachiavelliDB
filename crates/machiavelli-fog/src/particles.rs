//! Particle hypotheses with deterministic symplectic propagation and Bayesian observations.
//! Resampling and measurement updates are statistical operations, not symplectic maps.
use crate::{Belief, BeliefError};
use machiavelli_core::{Deadline, PhaseVector};
use machiavelli_symplectic::{Integrator, IntegratorError, Potential, Scheme, Separable};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParticleError {
    #[error(
        "particles require 1..4096 states, dimension 1..128, at most 128 Casimirs, matching shapes/constitution and one weight per state"
    )]
    InvalidParticles,
    #[error(
        "invalid particle propagation: finite dt, at most 1000000 steps and a matching Hamiltonian dimension required"
    )]
    InvalidPropagation,
    #[error(
        "position observation needs matching finite coordinates and a finite positive standard deviation"
    )]
    InvalidObservation,
    #[error("resampling threshold must be a finite fraction in [0,1]")]
    InvalidThreshold,
    #[error(transparent)]
    Core(#[from] machiavelli_core::CoreError),
    #[error(transparent)]
    Belief(#[from] BeliefError),
    #[error(transparent)]
    Integrator(#[from] IntegratorError),
}
#[derive(Clone)]
pub struct ParticleFilter {
    particles: Vec<PhaseVector>,
    belief: Belief,
    rng: ChaCha8Rng,
}
impl ParticleFilter {
    pub fn new(
        particles: Vec<PhaseVector>,
        weights: &[f64],
        seed: u64,
    ) -> Result<Self, ParticleError> {
        let first = particles.first().ok_or(ParticleError::InvalidParticles)?;
        if particles.len() > 4096
            || particles.len() != weights.len()
            || !(1..=128).contains(&first.dim())
            || first.c().len() > 128
        {
            return Err(ParticleError::InvalidParticles);
        }
        for phase in &particles {
            phase.validate()?;
            if phase.dim() != first.dim() || phase.c() != first.c() {
                return Err(ParticleError::InvalidParticles);
            }
        }
        Ok(Self {
            particles,
            belief: Belief::new(weights)?,
            rng: ChaCha8Rng::seed_from_u64(seed),
        })
    }
    pub fn particles(&self) -> &[PhaseVector] {
        &self.particles
    }
    pub fn weights(&self) -> &[f64] {
        self.belief.probabilities()
    }
    pub fn effective_sample_size(&self) -> f64 {
        (1.0 / self.weights().iter().map(|p| p * p).sum::<f64>())
            .clamp(1.0, self.particles.len() as f64)
    }
    /// All particles advance or none do. False means the deadline discarded the candidate ensemble.
    /// Each particle reuses its compensated integrator between bounded chunks of 64 steps.
    pub fn predict<P: Potential>(
        &mut self,
        system: &Separable<P>,
        scheme: Scheme,
        dt: f64,
        steps: usize,
        deadline: &Deadline,
    ) -> Result<bool, ParticleError> {
        if !dt.is_finite() || steps > 1_000_000 || system.dim() != self.particles[0].dim() {
            return Err(ParticleError::InvalidPropagation);
        }
        if deadline.expired() {
            return Ok(false);
        }
        let mut next = self.particles.clone();
        for phase in &mut next {
            let mut integrator = Integrator::new(system, scheme).compensated();
            let mut completed = 0;
            while completed < steps {
                if deadline.expired() {
                    return Ok(false);
                }
                let count = (steps - completed).min(64);
                let (q, p) = phase.qp_mut();
                integrator.run(q, p, dt, count)?;
                completed += count;
            }
            phase.validate()?;
        }
        if deadline.expired() {
            return Ok(false);
        }
        self.particles = next;
        Ok(true)
    }
    /// Custom observation likelihoods in log space. Impossible evidence leaves the prior intact.
    pub fn observe_log_likelihoods(
        &mut self,
        log_likelihoods: &[f64],
    ) -> Result<(), ParticleError> {
        self.belief.observe(log_likelihoods)?;
        Ok(())
    }
    /// Isotropic Gaussian position sensor. The shared normalizing constant cancels in Bayes' rule.
    pub fn observe_position(
        &mut self,
        observation: &[f64],
        standard_deviation: f64,
    ) -> Result<(), ParticleError> {
        if observation.len() != self.particles[0].dim()
            || observation.iter().any(|v| !v.is_finite())
            || !standard_deviation.is_finite()
            || standard_deviation <= 0.0
        {
            return Err(ParticleError::InvalidObservation);
        }
        let logs = self
            .particles
            .iter()
            .map(|phase| {
                phase
                    .q()
                    .iter()
                    .zip(observation)
                    .map(|(q, o)| {
                        let residual = (q - o) / standard_deviation;
                        -0.5 * residual * residual
                    })
                    .sum()
            })
            .collect::<Vec<f64>>();
        self.observe_log_likelihoods(&logs)
    }
    /// Seeded systematic resampling when ESS falls below fraction*N (up to rounding tolerance).
    /// No process noise is invented; repeated particles can require later model-specific rejuvenation.
    pub fn resample_if_needed(&mut self, minimum_fraction: f64) -> Result<bool, ParticleError> {
        if !minimum_fraction.is_finite() || !(0.0..=1.0).contains(&minimum_fraction) {
            return Err(ParticleError::InvalidThreshold);
        }
        let n = self.particles.len();
        if self.effective_sample_size() + 1e-12 * n as f64 >= minimum_fraction * n as f64 {
            return Ok(false);
        }
        let total: f64 = self.weights().iter().sum();
        let spacing = total / n as f64;
        let start = self.rng.random::<f64>() * spacing;
        let last = self
            .weights()
            .iter()
            .rposition(|w| *w > 0.0)
            .expect("normalized nonzero belief");
        let mut index = 0;
        let mut cumulative = self.weights()[0];
        let mut next = Vec::with_capacity(n);
        for i in 0..n {
            let target = start + i as f64 * spacing;
            while index + 1 < n && (self.weights()[index] == 0.0 || target >= cumulative) {
                index += 1;
                cumulative += self.weights()[index];
            }
            let selected = if self.weights()[index] > 0.0 {
                index
            } else {
                last
            };
            next.push(self.particles[selected].clone());
        }
        self.particles = next;
        self.belief = Belief::new(&vec![1.0; n])?;
        Ok(true)
    }
    pub fn mean_phase(&self) -> Result<PhaseVector, ParticleError> {
        let coordinate = |index: usize, momentum: bool| {
            let values = self
                .particles
                .iter()
                .map(|p| if momentum { p.p()[index] } else { p.q()[index] })
                .collect::<Vec<_>>();
            let scale = values.iter().fold(0.0_f64, |s, v| s.max(v.abs()));
            if scale == 0.0 {
                return 0.0;
            }
            let minimum = values
                .iter()
                .map(|v| v / scale)
                .fold(f64::INFINITY, f64::min);
            let maximum = values
                .iter()
                .map(|v| v / scale)
                .fold(f64::NEG_INFINITY, f64::max);
            let total: f64 = self.weights().iter().sum();
            let scaled: f64 = values
                .iter()
                .zip(self.weights())
                .map(|(v, w)| (v / scale) * w)
                .sum();
            (scaled / total).clamp(minimum, maximum) * scale
        };
        let n = self.particles[0].dim();
        Ok(PhaseVector::with_constitution(
            (0..n).map(|i| coordinate(i, false)).collect(),
            (0..n).map(|i| coordinate(i, true)).collect(),
            self.particles[0].c().to_vec(),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_symplectic::{DiagonalKinetic, systems::Harmonic};
    fn phase(q: f64, p: f64) -> PhaseVector {
        PhaseVector::with_constitution(vec![q], vec![p], vec![7.0]).unwrap()
    }
    fn system(stiffness: f64) -> Separable<Harmonic> {
        Separable::new(
            DiagonalKinetic::uniform(1, 1.0),
            Harmonic::isotropic(1, stiffness),
        )
    }
    #[test]
    fn symplectic_prediction_is_reversible_and_keeps_weights_and_constitution() {
        let particles = vec![phase(0.3, 0.7), phase(-0.8, 0.2)];
        let mut filter = ParticleFilter::new(particles.clone(), &[1.0, 3.0], 42).unwrap();
        let weights = filter.weights().to_vec();
        assert!(
            filter
                .predict(&system(1.0), Scheme::Yoshida4, 0.01, 100, &Deadline::none())
                .unwrap()
        );
        assert_ne!(filter.particles(), particles);
        assert_eq!(filter.weights(), weights);
        assert!(
            filter
                .predict(
                    &system(1.0),
                    Scheme::Yoshida4,
                    -0.01,
                    100,
                    &Deadline::none()
                )
                .unwrap()
        );
        for (old, new) in particles.iter().zip(filter.particles()) {
            assert!((old.q()[0] - new.q()[0]).abs() < 1e-12);
            assert!((old.p()[0] - new.p()[0]).abs() < 1e-12);
            assert_eq!(new.c(), &[7.0]);
        }
    }
    #[test]
    fn deadline_or_numerical_failure_never_publishes_partial_predictions() {
        let particles = vec![phase(1.0, 0.0), phase(f64::MAX, 0.0)];
        let mut filter = ParticleFilter::new(particles.clone(), &[1.0, 1.0], 42).unwrap();
        assert!(
            !filter
                .predict(
                    &system(1.0),
                    Scheme::StormerVerlet,
                    0.01,
                    100,
                    &Deadline::from_millis(0)
                )
                .unwrap()
        );
        assert_eq!(filter.particles(), particles);
        assert!(
            filter
                .predict(
                    &system(2.0),
                    Scheme::StormerVerlet,
                    0.01,
                    100,
                    &Deadline::none()
                )
                .is_err()
        );
        assert_eq!(filter.particles(), particles);
        assert_eq!(filter.weights(), &[0.5, 0.5]);
    }
    #[test]
    fn sensor_observations_concentrate_posterior_and_resampling_preserves_casimir() {
        let particles = vec![phase(0.0, 0.0), phase(10.0, 0.0)];
        let mut filter = ParticleFilter::new(particles, &[1.0, 1.0], 42).unwrap();
        assert_eq!(filter.effective_sample_size(), 2.0);
        assert!(!filter.resample_if_needed(1.0).unwrap());
        filter.observe_position(&[0.0], 1.0).unwrap();
        assert!(filter.weights()[0] > 0.999);
        assert!(filter.effective_sample_size() < 1.001);
        assert!(filter.resample_if_needed(0.9).unwrap());
        assert_eq!(filter.weights(), &[0.5, 0.5]);
        assert!(
            filter
                .particles()
                .iter()
                .all(|p| p.q() == [0.0] && p.c() == [7.0])
        );
    }
    #[test]
    fn impossible_or_malformed_observations_leave_prior_unchanged() {
        let mut filter =
            ParticleFilter::new(vec![phase(1.0, 0.0), phase(2.0, 0.0)], &[1.0, 3.0], 42).unwrap();
        let weights = filter.weights().to_vec();
        assert!(
            filter
                .observe_log_likelihoods(&[f64::NEG_INFINITY; 2])
                .is_err()
        );
        assert_eq!(filter.weights(), weights);
        assert!(filter.observe_position(&[f64::NAN], 1.0).is_err());
        assert!(filter.observe_position(&[1.0], 0.0).is_err());
        assert!(filter.observe_position(&[], 1.0).is_err());
        assert_eq!(filter.weights(), weights);
    }
    #[test]
    fn seeded_resampling_is_reproducible_and_zero_weight_particles_never_survive() {
        let particles = (0..8).map(|i| phase(i as f64, 0.0)).collect();
        let mut first =
            ParticleFilter::new(particles, &[0.0, 1.0, 4.0, 0.0, 0.0, 1.0, 0.0, 0.0], 7).unwrap();
        let mut second = first.clone();
        assert!(first.resample_if_needed(f64::NAN).is_err());
        assert!(first.resample_if_needed(1.0).unwrap());
        assert!(second.resample_if_needed(1.0).unwrap());
        assert_eq!(first.particles(), second.particles());
        assert_eq!(first.weights(), second.weights());
        assert!(
            first
                .particles()
                .iter()
                .all(|p| [1.0, 2.0, 5.0].contains(&p.q()[0]))
        );
    }
    #[test]
    fn weighted_mean_is_finite_for_opposing_extreme_coordinates() {
        let filter = ParticleFilter::new(
            vec![phase(f64::MAX, 1.0), phase(-f64::MAX, 3.0)],
            &[1.0, 1.0],
            0,
        )
        .unwrap();
        let mean = filter.mean_phase().unwrap();
        assert_eq!(mean.q(), &[0.0]);
        assert_eq!(mean.p(), &[2.0]);
        assert_eq!(mean.c(), &[7.0]);
    }
    #[test]
    fn invalid_shapes_constitution_weights_and_prediction_budgets_are_rejected() {
        assert!(ParticleFilter::new(vec![], &[], 0).is_err());
        assert!(ParticleFilter::new(vec![phase(0.0, 0.0)], &[], 0).is_err());
        assert!(
            ParticleFilter::new(vec![phase(0.0, 0.0), PhaseVector::zeros(2)], &[1.0, 1.0], 0)
                .is_err()
        );
        assert!(
            ParticleFilter::new(vec![phase(0.0, 0.0), PhaseVector::zeros(1)], &[1.0, 1.0], 0)
                .is_err()
        );
        assert!(ParticleFilter::new(vec![phase(0.0, 0.0)], &[0.0], 0).is_err());
        let mut filter = ParticleFilter::new(vec![phase(0.0, 0.0)], &[1.0], 0).unwrap();
        assert!(
            filter
                .predict(
                    &system(1.0),
                    Scheme::StormerVerlet,
                    f64::NAN,
                    1,
                    &Deadline::none()
                )
                .is_err()
        );
    }
}
