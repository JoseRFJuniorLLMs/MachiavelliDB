//! Bayesian belief snapshots. Public observation keys group hidden hypotheses;
//! downstream planners receive the posterior rather than a selected hidden state.
pub mod particles;
pub use particles::{ParticleError, ParticleFilter};
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct BeliefError(&'static str);
#[derive(Clone, Debug)]
pub struct Belief {
    probabilities: Vec<f64>,
}
impl Belief {
    pub fn new(weights: &[f64]) -> Result<Self, BeliefError> {
        if weights.is_empty() || weights.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(BeliefError("invalid weights"));
        }
        let scale = weights.iter().copied().fold(0.0, f64::max);
        if scale == 0.0 {
            return Err(BeliefError("zero evidence"));
        }
        let total = weights.iter().map(|w| w / scale).sum::<f64>();
        Ok(Self {
            probabilities: weights.iter().map(|w| (w / scale) / total).collect(),
        })
    }
    pub fn probabilities(&self) -> &[f64] {
        &self.probabilities
    }
    /// log likelihoods are permitted to be -infinity for impossible observations.
    /// Failed evidence leaves the previous posterior unchanged.
    pub fn observe(&mut self, log_likelihoods: &[f64]) -> Result<(), BeliefError> {
        if log_likelihoods.len() != self.probabilities.len()
            || log_likelihoods
                .iter()
                .any(|v| v.is_nan() || *v == f64::INFINITY)
        {
            return Err(BeliefError("invalid likelihoods"));
        }
        let log_weights: Vec<f64> = self
            .probabilities
            .iter()
            .zip(log_likelihoods)
            .map(|(p, l)| p.ln() + l)
            .collect();
        let maximum = log_weights
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if !maximum.is_finite() {
            return Err(BeliefError("impossible evidence"));
        }
        let weights: Vec<_> = log_weights.iter().map(|v| (v - maximum).exp()).collect();
        *self = Self::new(&weights)?;
        Ok(())
    }
    pub fn expected(&self, utilities: &[f64]) -> Result<f64, BeliefError> {
        if utilities.len() != self.probabilities.len() || utilities.iter().any(|v| !v.is_finite()) {
            return Err(BeliefError("invalid utilities"));
        }
        let value = self
            .probabilities
            .iter()
            .zip(utilities)
            .map(|(p, u)| p * u)
            .sum::<f64>();
        if !value.is_finite() {
            return Err(BeliefError("utility overflow"));
        }
        Ok(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bayes_is_stable_and_impossible_evidence_is_atomic() {
        let mut b = Belief::new(&[1.0, 1.0]).unwrap();
        b.observe(&[0.8_f64.ln(), 0.2_f64.ln()]).unwrap();
        assert!((b.probabilities()[0] - 0.8).abs() < 1e-14);
        let old = b.probabilities().to_vec();
        assert!(b.observe(&[f64::NEG_INFINITY; 2]).is_err());
        assert_eq!(b.probabilities(), old);
        b.observe(&[-10000.0, -10001.0]).unwrap();
        assert!(b.probabilities()[0] > 0.9);
    }
    #[test]
    fn priors_and_dimensions_are_validated() {
        assert!(Belief::new(&[0.0, 0.0]).is_err());
        assert!(Belief::new(&[f64::NAN]).is_err());
        let b = Belief::new(&[f64::MAX, f64::MAX]).unwrap();
        assert_eq!(b.expected(&[1.0, -1.0]).unwrap(), 0.0);
        assert!(b.expected(&[1.0]).is_err());
    }
}
