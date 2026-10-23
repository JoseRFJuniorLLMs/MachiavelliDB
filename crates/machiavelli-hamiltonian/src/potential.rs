//! Smooth, explicitly sampled risk fields and adversarial Gaussian barriers.
//!
//! Telemetry is a fixed snapshot, not a forecast: replacing it between rollouts
//! changes the Hamiltonian, so energy conservation only applies while it is frozen.
//! Checked methods return errors; infallible trait methods signal errors with NaN.

use crate::HamiltonianError;
use machiavelli_symplectic::system::Potential;

fn finite(value: f64, field: &'static str) -> Result<f64, HamiltonianError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(HamiltonianError::NonFinite { field })
    }
}

fn nonnegative(value: f64, field: &'static str) -> Result<(), HamiltonianError> {
    finite(value, field)?;
    if value < 0.0 {
        return Err(HamiltonianError::InvalidParameter { field, value });
    }
    Ok(())
}

fn dimension(n: usize) -> Result<(), HamiltonianError> {
    if n == 0 || n.checked_mul(n).is_none() {
        return Err(HamiltonianError::InvalidParameter {
            field: "dimension",
            value: n as f64,
        });
    }
    Ok(())
}

fn length(expected: usize, actual: usize) -> Result<(), HamiltonianError> {
    if expected != actual {
        Err(HamiltonianError::DimensionMismatch { expected, actual })
    } else {
        Ok(())
    }
}

fn position(n: usize, q: &[f64]) -> Result<(), HamiltonianError> {
    length(n, q.len())?;
    for &x in q {
        finite(x, "position")?;
    }
    Ok(())
}

/// Normalized anomaly observations in `[0, 1]`, sampled at `observed_at`.
///
/// The caller supplies the timestamp and the normalization policy. Latency is a
/// normalized anomaly score, not a duration; entropy divergence is likewise a
/// score, not raw entropy. Every vector has one observation per configuration
/// coordinate. Construction copies ownership of this immutable snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct RiskSnapshot {
    observed_at: u64,
    latency: Vec<f64>,
    error_rate: Vec<f64>,
    entropy_divergence: Vec<f64>,
}

impl RiskSnapshot {
    pub fn new(
        observed_at: u64,
        latency: Vec<f64>,
        error_rate: Vec<f64>,
        entropy_divergence: Vec<f64>,
    ) -> Result<Self, HamiltonianError> {
        dimension(latency.len())?;
        length(latency.len(), error_rate.len())?;
        length(latency.len(), entropy_divergence.len())?;
        for (field, values) in [
            ("latency", &latency),
            ("error_rate", &error_rate),
            ("entropy_divergence", &entropy_divergence),
        ] {
            for &value in values {
                finite(value, field)?;
                if !(0.0..=1.0).contains(&value) {
                    return Err(HamiltonianError::InvalidParameter { field, value });
                }
            }
        }
        Ok(Self {
            observed_at,
            latency,
            error_rate,
            entropy_divergence,
        })
    }

    pub fn observed_at(&self) -> u64 {
        self.observed_at
    }
    pub fn dim(&self) -> usize {
        self.latency.len()
    }
    pub fn latency(&self) -> &[f64] {
        &self.latency
    }
    pub fn error_rate(&self) -> &[f64] {
        &self.error_rate
    }
    pub fn entropy_divergence(&self) -> &[f64] {
        &self.entropy_divergence
    }
}

/// Nonnegative telemetry weights, normalized to sum to one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiskWeights {
    normalized: [f64; 3],
}

impl RiskWeights {
    pub fn new(
        latency: f64,
        error_rate: f64,
        entropy_divergence: f64,
    ) -> Result<Self, HamiltonianError> {
        nonnegative(latency, "latency_weight")?;
        nonnegative(error_rate, "error_rate_weight")?;
        nonnegative(entropy_divergence, "entropy_weight")?;
        let total = finite(latency + error_rate + entropy_divergence, "weight_sum")?;
        if total == 0.0 {
            return Err(HamiltonianError::InvalidParameter {
                field: "weight_sum",
                value: total,
            });
        }
        Ok(Self {
            normalized: [
                latency / total,
                error_rate / total,
                entropy_divergence / total,
            ],
        })
    }

    /// Weights in latency, error-rate, entropy-divergence order.
    pub fn normalized(&self) -> [f64; 3] {
        self.normalized
    }
}

impl Default for RiskWeights {
    fn default() -> Self {
        Self {
            normalized: [1.0 / 3.0; 3],
        }
    }
}

/// `V_F(q; t_snapshot) = scale * sum_i r_i * softplus(q_i)`.
///
/// `r_i` is the weighted mean of the three normalized anomaly scores. The
/// configuration coordinate `q_i` is a dimensionless exposure variable: larger
/// values expose more risk. `softplus(x) = ln(1 + exp(x))` is evaluated stably,
/// stays nonnegative, and has derivatives `sigmoid(x)` and `s * (1 - s)`.
/// A zero anomaly score produces zero risk; this model does not predict faults.
///
/// Build a new potential to use newer telemetry. Mutating a snapshot during a
/// rollout would make its energy explicitly time dependent.
#[derive(Clone, Debug, PartialEq)]
pub struct FortunaPotential {
    snapshot: RiskSnapshot,
    weights: RiskWeights,
    scale: f64,
    coefficients: Vec<f64>,
}

impl FortunaPotential {
    pub fn new(
        snapshot: RiskSnapshot,
        weights: RiskWeights,
        scale: f64,
    ) -> Result<Self, HamiltonianError> {
        nonnegative(scale, "fortuna_scale")?;
        let w = weights.normalized;
        let mut coefficients = Vec::with_capacity(snapshot.dim());
        for i in 0..snapshot.dim() {
            let risk = (w[0] * snapshot.latency[i]
                + w[1] * snapshot.error_rate[i]
                + w[2] * snapshot.entropy_divergence[i])
                .clamp(0.0, 1.0);
            coefficients.push(finite(scale * risk, "fortuna_coefficient")?);
        }
        Ok(Self {
            snapshot,
            weights,
            scale,
            coefficients,
        })
    }

    pub fn snapshot(&self) -> &RiskSnapshot {
        &self.snapshot
    }
    pub fn weights(&self) -> RiskWeights {
        self.weights
    }
    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn validate_position(&self, q: &[f64]) -> Result<(), HamiltonianError> {
        position(self.dim(), q)
    }

    pub fn try_value(&self, q: &[f64]) -> Result<f64, HamiltonianError> {
        self.validate_position(q)?;
        let mut total = 0.0;
        for (&x, &coefficient) in q.iter().zip(&self.coefficients) {
            let softplus = if x > 0.0 {
                x + (-x).exp().ln_1p()
            } else {
                x.exp().ln_1p()
            };
            total = finite(total + coefficient * softplus, "fortuna_value")?;
        }
        Ok(total)
    }

    pub fn try_gradient(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        length(self.dim(), out.len())?;
        for ((o, &x), &coefficient) in out.iter_mut().zip(q).zip(&self.coefficients) {
            *o = finite(coefficient * sigmoid(x), "fortuna_gradient")?;
        }
        Ok(())
    }

    pub fn try_hessian(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        let n = self.dim();
        length(n * n, out.len())?;
        out.fill(0.0);
        for (i, (&x, &coefficient)) in q.iter().zip(&self.coefficients).enumerate() {
            let s = sigmoid(x);
            out[i * n + i] = finite(coefficient * s * (1.0 - s), "fortuna_hessian")?;
        }
        Ok(())
    }
}

fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

impl FortunaPotential {
    pub fn try_gradient_add(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        length(self.dim(), out.len())?;
        for ((o, &x), &coefficient) in out.iter_mut().zip(q).zip(&self.coefficients) {
            *o = finite(*o + coefficient * sigmoid(x), "fortuna_gradient_sum")?;
        }
        Ok(())
    }

    pub fn try_hessian_add(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        let n = self.dim();
        length(n * n, out.len())?;
        for &x in out.iter() {
            finite(x, "hessian_accumulator")?;
        }
        for (i, (&x, &coefficient)) in q.iter().zip(&self.coefficients).enumerate() {
            let s = sigmoid(x);
            out[i * n + i] = finite(
                out[i * n + i] + coefficient * s * (1.0 - s),
                "fortuna_hessian_sum",
            )?;
        }
        Ok(())
    }
}

impl Potential for FortunaPotential {
    fn dim(&self) -> usize {
        self.coefficients.len()
    }
    fn value(&self, q: &[f64]) -> f64 {
        self.try_value(q).unwrap_or(f64::NAN)
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        if self.try_gradient(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        if self.try_hessian(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn gradient_add(&self, q: &[f64], out: &mut [f64]) {
        if self.try_gradient_add(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn hessian_add(&self, q: &[f64], out: &mut [f64]) {
        if self.try_hessian_add(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
}
/// Nonnegative Gaussian adversarial barrier:
/// `Phi(q) = amplitude * exp(-||q - center||^2 / (2 * width^2))`.
///
/// With `z = (q - center) / width`, its gradient is `-Phi * z / width`
/// and its Hessian is `Phi * (z_i*z_j - delta_ij) / width^2`.
/// The center is a supplied scenario parameter, not an inferred attacker state.
#[derive(Clone, Debug, PartialEq)]
pub struct AdversarialPotential {
    center: Vec<f64>,
    amplitude: f64,
    width: f64,
    inv_width: f64,
    inv_width_squared: f64,
}

impl AdversarialPotential {
    pub fn new(center: Vec<f64>, amplitude: f64, width: f64) -> Result<Self, HamiltonianError> {
        dimension(center.len())?;
        for &x in &center {
            finite(x, "adversarial_center")?;
        }
        nonnegative(amplitude, "adversarial_amplitude")?;
        finite(width, "adversarial_width")?;
        if width <= 0.0 {
            return Err(HamiltonianError::InvalidParameter {
                field: "adversarial_width",
                value: width,
            });
        }
        let inv_width = finite(width.recip(), "inverse_width")?;
        let inv_width_squared = finite(inv_width * inv_width, "inverse_width_squared")?;
        if inv_width_squared == 0.0 {
            return Err(HamiltonianError::InvalidParameter {
                field: "adversarial_width",
                value: width,
            });
        }
        finite(amplitude * inv_width, "adversarial_gradient_scale")?;
        finite(amplitude * inv_width_squared, "adversarial_hessian_scale")?;
        Ok(Self {
            center,
            amplitude,
            width,
            inv_width,
            inv_width_squared,
        })
    }

    pub fn center(&self) -> &[f64] {
        &self.center
    }
    pub fn amplitude(&self) -> f64 {
        self.amplitude
    }
    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn validate_position(&self, q: &[f64]) -> Result<(), HamiltonianError> {
        position(self.dim(), q)
    }

    // Infinite normalized distance can arise from finite, extremely distant
    // coordinates. Its Gaussian limit and all its derivatives are zero.
    fn profile(&self, q: &[f64]) -> (f64, Option<f64>) {
        let radius_squared: f64 = q
            .iter()
            .zip(&self.center)
            .map(|(x, c)| {
                let z = (x - c) * self.inv_width;
                z * z
            })
            .sum();
        if self.amplitude == 0.0 || !radius_squared.is_finite() {
            return (0.0, None);
        }
        let attenuation = (-0.5 * radius_squared).exp();
        let value = self.amplitude * attenuation;
        if attenuation == 0.0 || value == 0.0 {
            // A derivative can remain representable after the energy itself
            // underflows: keep its logarithm only on this uncommon path.
            let log_value = self.amplitude.ln() - 0.5 * radius_squared;
            let recovered = log_value.exp();
            (
                recovered,
                if recovered == 0.0 {
                    Some(log_value)
                } else {
                    None
                },
            )
        } else {
            (value, None)
        }
    }

    pub fn try_value(&self, q: &[f64]) -> Result<f64, HamiltonianError> {
        self.validate_position(q)?;
        finite(self.profile(q).0, "adversarial_value")
    }

    pub fn try_gradient(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        length(self.dim(), out.len())?;
        let (value, log_value) = self.profile(q);
        if value == 0.0 && log_value.is_none() {
            out.fill(0.0);
            return Ok(());
        }
        let factor = value * self.inv_width;
        let log_factor = log_value.map(|v| v + self.inv_width.ln());
        for ((o, x), c) in out.iter_mut().zip(q).zip(&self.center) {
            let z = (x - c) * self.inv_width;
            let derivative = if let Some(log_factor) = log_factor {
                signed_exp(log_factor, -z)
            } else {
                -factor * z
            };
            *o = finite(derivative, "adversarial_gradient")?;
        }
        Ok(())
    }

    pub fn try_hessian(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        let n = self.dim();
        length(n * n, out.len())?;
        let (value, log_value) = self.profile(q);
        if value == 0.0 && log_value.is_none() {
            out.fill(0.0);
            return Ok(());
        }
        let factor = value * self.inv_width_squared;
        let log_factor = log_value.map(|v| v + 2.0 * self.inv_width.ln());
        for i in 0..n {
            let zi = (q[i] - self.center[i]) * self.inv_width;
            for j in 0..n {
                let zj = (q[j] - self.center[j]) * self.inv_width;
                let identity = if i == j { 1.0 } else { 0.0 };
                let coefficient = zi * zj - identity;
                let derivative = if let Some(log_factor) = log_factor {
                    signed_exp(log_factor, coefficient)
                } else {
                    factor * coefficient
                };
                out[i * n + j] = finite(derivative, "adversarial_hessian")?;
            }
        }
        Ok(())
    }
}

impl AdversarialPotential {
    pub fn try_gradient_add(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        length(self.dim(), out.len())?;
        for &x in out.iter() {
            finite(x, "gradient_accumulator")?;
        }
        let (value, log_value) = self.profile(q);
        if value == 0.0 && log_value.is_none() {
            return Ok(());
        }
        let factor = value * self.inv_width;
        let log_factor = log_value.map(|v| v + self.inv_width.ln());
        for ((o, x), c) in out.iter_mut().zip(q).zip(&self.center) {
            let z = (x - c) * self.inv_width;
            let derivative = if let Some(log_factor) = log_factor {
                signed_exp(log_factor, -z)
            } else {
                -factor * z
            };
            *o = finite(*o + derivative, "adversarial_gradient_sum")?;
        }
        Ok(())
    }

    pub fn try_hessian_add(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        let n = self.dim();
        length(n * n, out.len())?;
        for &x in out.iter() {
            finite(x, "hessian_accumulator")?;
        }
        let (value, log_value) = self.profile(q);
        if value == 0.0 && log_value.is_none() {
            return Ok(());
        }
        let factor = value * self.inv_width_squared;
        let log_factor = log_value.map(|v| v + 2.0 * self.inv_width.ln());
        for i in 0..n {
            let zi = (q[i] - self.center[i]) * self.inv_width;
            for j in 0..n {
                let zj = (q[j] - self.center[j]) * self.inv_width;
                let identity = if i == j { 1.0 } else { 0.0 };
                let coefficient = zi * zj - identity;
                let derivative = if let Some(log_factor) = log_factor {
                    signed_exp(log_factor, coefficient)
                } else {
                    factor * coefficient
                };
                out[i * n + j] = finite(out[i * n + j] + derivative, "adversarial_hessian_sum")?;
            }
        }
        Ok(())
    }
}

// Evaluate exp(log_factor) * coefficient without losing the entire product when
// exp(log_factor) alone underflows. A zero coefficient never reaches ln(0).
fn signed_exp(log_factor: f64, coefficient: f64) -> f64 {
    if coefficient == 0.0 {
        0.0
    } else {
        coefficient.signum() * (log_factor + coefficient.abs().ln()).exp()
    }
}

impl Potential for AdversarialPotential {
    fn dim(&self) -> usize {
        self.center.len()
    }
    fn value(&self, q: &[f64]) -> f64 {
        self.try_value(q).unwrap_or(f64::NAN)
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        if self.try_gradient(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        if self.try_hessian(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn gradient_add(&self, q: &[f64], out: &mut [f64]) {
        if self.try_gradient_add(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn hessian_add(&self, q: &[f64], out: &mut [f64]) {
        if self.try_hessian_add(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
}
/// Sum of one or more potential fields with the same nonzero dimension.
///
/// Third-party `Potential` implementations remain responsible for their own
/// contracts. Checked evaluation rejects nonfinite terms and accumulated sums;
/// trait methods signal invalid input with NaN because `Potential` has no error return.
pub struct SumPotential {
    terms: Vec<Box<dyn Potential>>,
    n: usize,
}

impl SumPotential {
    pub fn new(terms: Vec<Box<dyn Potential>>) -> Result<Self, HamiltonianError> {
        let n = terms.first().map_or(0, |term| term.dim());
        dimension(n)?;
        for term in &terms {
            length(n, term.dim())?;
        }
        Ok(Self { terms, n })
    }

    pub fn terms(&self) -> &[Box<dyn Potential>] {
        &self.terms
    }
    pub fn validate_position(&self, q: &[f64]) -> Result<(), HamiltonianError> {
        position(self.n, q)
    }

    pub fn try_value(&self, q: &[f64]) -> Result<f64, HamiltonianError> {
        self.validate_position(q)?;
        let mut total = 0.0;
        for term in &self.terms {
            total = finite(total + term.value(q), "sum_value")?;
        }
        Ok(total)
    }

    pub fn try_gradient(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        length(self.n, out.len())?;
        out.fill(0.0);
        self.try_gradient_add(q, out)
    }

    pub fn try_gradient_add(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        length(self.n, out.len())?;
        for term in &self.terms {
            term.gradient_add(q, out);
            for &value in out.iter() {
                finite(value, "sum_gradient")?;
            }
        }
        Ok(())
    }

    pub fn try_hessian(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        length(self.n * self.n, out.len())?;
        out.fill(0.0);
        self.try_hessian_add(q, out)
    }

    pub fn try_hessian_add(&self, q: &[f64], out: &mut [f64]) -> Result<(), HamiltonianError> {
        self.validate_position(q)?;
        length(self.n * self.n, out.len())?;
        for term in &self.terms {
            term.hessian_add(q, out);
            for &value in out.iter() {
                finite(value, "sum_hessian")?;
            }
        }
        Ok(())
    }
}

impl Potential for SumPotential {
    fn dim(&self) -> usize {
        self.n
    }
    fn value(&self, q: &[f64]) -> f64 {
        self.try_value(q).unwrap_or(f64::NAN)
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        if self.try_gradient(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        if self.try_hessian(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn gradient_add(&self, q: &[f64], out: &mut [f64]) {
        if self.try_gradient_add(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
    fn hessian_add(&self, q: &[f64], out: &mut [f64]) {
        if self.try_hessian_add(q, out).is_err() {
            out.fill(f64::NAN);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(value: f64) -> RiskSnapshot {
        RiskSnapshot::new(123, vec![value; 2], vec![value; 2], vec![value; 2]).unwrap()
    }

    fn derivatives(potential: &dyn Potential, q: &[f64]) {
        let n = q.len();
        let step = 1e-5;
        let mut gradient = vec![0.0; n];
        let mut hessian = vec![0.0; n * n];
        potential.gradient(q, &mut gradient);
        potential.hessian(q, &mut hessian);
        for j in 0..n {
            let mut plus = q.to_vec();
            let mut minus = q.to_vec();
            plus[j] += step;
            minus[j] -= step;
            let estimated = (potential.value(&plus) - potential.value(&minus)) / (2.0 * step);
            assert!(
                (gradient[j] - estimated).abs() < 1e-8,
                "gradient {j}: {} vs {estimated}",
                gradient[j]
            );
            let mut gp = vec![0.0; n];
            let mut gm = vec![0.0; n];
            potential.gradient(&plus, &mut gp);
            potential.gradient(&minus, &mut gm);
            for i in 0..n {
                let estimated = (gp[i] - gm[i]) / (2.0 * step);
                assert!(
                    (hessian[i * n + j] - estimated).abs() < 1e-8,
                    "Hessian ({i}, {j}): {} vs {estimated}",
                    hessian[i * n + j]
                );
            }
        }
    }

    #[test]
    fn analytic_derivatives_match_finite_differences() {
        let risk =
            FortunaPotential::new(snapshot(0.6), RiskWeights::new(1.0, 2.0, 3.0).unwrap(), 2.5)
                .unwrap();
        let adversarial = AdversarialPotential::new(vec![0.3, -0.2], 2.2, 0.8).unwrap();
        derivatives(&risk, &[-0.5, 1.1]);
        derivatives(&adversarial, &[-0.5, 1.1]);
        let sum = SumPotential::new(vec![Box::new(risk), Box::new(adversarial)]).unwrap();
        derivatives(&sum, &[-0.5, 1.1]);
    }

    #[test]
    fn risk_increases_with_anomaly_and_exposure() {
        let low = FortunaPotential::new(snapshot(0.2), RiskWeights::default(), 3.0).unwrap();
        let high = FortunaPotential::new(snapshot(0.8), RiskWeights::default(), 3.0).unwrap();
        assert!(low.value(&[0.0, 0.0]) > 0.0);
        assert!(high.value(&[0.0, 0.0]) > low.value(&[0.0, 0.0]));
        assert!(high.value(&[1.0, 1.0]) > high.value(&[0.0, 0.0]));
        assert_eq!(high.snapshot().observed_at(), 123);
        assert_eq!(high.snapshot().latency(), &[0.8, 0.8]);
        let zero = FortunaPotential::new(snapshot(0.0), RiskWeights::default(), 3.0).unwrap();
        assert_eq!(zero.value(&[10.0, 10.0]), 0.0);
        let disabled = FortunaPotential::new(snapshot(1.0), RiskWeights::default(), 0.0).unwrap();
        assert_eq!(disabled.value(&[f64::MAX, f64::MAX]), 0.0);
    }

    #[test]
    fn telemetry_weights_select_each_signal() {
        let data = RiskSnapshot::new(7, vec![0.2], vec![0.5], vec![0.9]).unwrap();
        for (weights, risk) in [
            ([1.0, 0.0, 0.0], 0.2),
            ([0.0, 1.0, 0.0], 0.5),
            ([0.0, 0.0, 1.0], 0.9),
        ] {
            let p = FortunaPotential::new(
                data.clone(),
                RiskWeights::new(weights[0], weights[1], weights[2]).unwrap(),
                1.0,
            )
            .unwrap();
            assert!((p.value(&[0.0]) - risk * 2.0_f64.ln()).abs() < 1e-15);
        }
    }

    #[test]
    fn gaussian_has_a_peak_and_finite_extreme_limits() {
        let p = AdversarialPotential::new(vec![0.0, 0.0], 2.0, 0.5).unwrap();
        assert_eq!(p.value(&[0.0, 0.0]), 2.0);
        assert!(p.value(&[0.5, 0.0]) < 2.0);
        let mut gradient = [1.0; 2];
        let mut hessian = [1.0; 4];
        p.try_gradient(&[f64::MAX, -f64::MAX], &mut gradient)
            .unwrap();
        p.try_hessian(&[f64::MAX, -f64::MAX], &mut hessian).unwrap();
        assert_eq!(gradient, [0.0; 2]);
        assert_eq!(hessian, [0.0; 4]);
        p.hessian(&[0.0, 0.0], &mut hessian);
        assert_eq!(hessian, [-8.0, 0.0, 0.0, -8.0]);
        let zero = AdversarialPotential::new(vec![0.0], 0.0, 1.0).unwrap();
        assert_eq!(zero.value(&[f64::MAX]), 0.0);
        zero.hessian(&[f64::MAX], &mut hessian[..1]);
        assert_eq!(hessian[0], 0.0);
        let huge = AdversarialPotential::new(vec![0.0], 1e300, 1.0).unwrap();
        assert!(huge.value(&[40.0]) > 0.0);
    }

    #[test]
    fn gaussian_recovers_derivatives_when_energy_underflows() {
        let width = 1e-154;
        let p = AdversarialPotential::new(vec![0.0], 1.0, width).unwrap();
        let q = [4e-153];
        assert_eq!(p.try_value(&q).unwrap(), 0.0);
        let mut gradient = [0.0];
        let mut hessian = [0.0];
        p.try_gradient(&q, &mut gradient).unwrap();
        p.try_hessian(&q, &mut hessian).unwrap();
        let z = q[0] / width;
        let log_profile = -0.5 * z * z;
        let expected_gradient = -(log_profile + z.ln() - width.ln()).exp();
        let expected_hessian = (log_profile + (z * z - 1.0).ln() - 2.0 * width.ln()).exp();
        assert!(expected_gradient < 0.0 && expected_hessian > 0.0);
        assert!((gradient[0] / expected_gradient - 1.0).abs() < 1e-11);
        assert!((hessian[0] / expected_hessian - 1.0).abs() < 1e-11);
        let mut accumulated_gradient = [expected_gradient];
        let mut accumulated_hessian = [expected_hessian];
        p.gradient_add(&q, &mut accumulated_gradient);
        p.hessian_add(&q, &mut accumulated_hessian);
        assert!((accumulated_gradient[0] / (2.0 * expected_gradient) - 1.0).abs() < 1e-11);
        assert!((accumulated_hessian[0] / (2.0 * expected_hessian) - 1.0).abs() < 1e-11);
        // Negative coordinates reverse the gradient, while a transverse zero
        // coordinate exercises the exact-zero coefficient in the log fallback.
        let p2 = AdversarialPotential::new(vec![0.0, 0.0], 1.0, width).unwrap();
        let mut g2 = [0.0; 2];
        let mut h2 = [0.0; 4];
        p2.try_gradient(&[-q[0], 0.0], &mut g2).unwrap();
        p2.try_hessian(&[-q[0], 0.0], &mut h2).unwrap();
        assert!((g2[0] / -expected_gradient - 1.0).abs() < 1e-11);
        assert_eq!(g2[1], 0.0);
        assert_eq!(h2[1], 0.0);
        assert_eq!(h2[2], 0.0);
        assert!(h2[3] < 0.0);
    }

    #[test]
    fn softplus_stays_finite_without_invalid_logarithms() {
        let p = FortunaPotential::new(snapshot(1.0), RiskWeights::default(), 1.0).unwrap();
        assert_eq!(p.try_value(&[-f64::MAX, f64::MAX]).unwrap(), f64::MAX);
        let mut gradient = [0.0; 2];
        let mut hessian = [0.0; 4];
        p.try_gradient(&[-f64::MAX, f64::MAX], &mut gradient)
            .unwrap();
        p.try_hessian(&[-f64::MAX, f64::MAX], &mut hessian).unwrap();
        assert_eq!(gradient, [0.0, 1.0]);
        assert_eq!(hessian, [0.0; 4]);
    }

    #[test]
    fn constructors_reject_invalid_and_unrepresentable_parameters() {
        assert!(RiskSnapshot::new(0, vec![], vec![], vec![]).is_err());
        assert!(RiskSnapshot::new(0, vec![0.1], vec![], vec![0.1]).is_err());
        for bad in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert!(RiskSnapshot::new(0, vec![bad], vec![0.0], vec![0.0]).is_err());
        }
        assert!(RiskWeights::new(0.0, 0.0, 0.0).is_err());
        assert!(RiskWeights::new(-1.0, 1.0, 1.0).is_err());
        assert!(RiskWeights::new(f64::NAN, 1.0, 1.0).is_err());
        assert!(RiskWeights::new(f64::MAX, f64::MAX, 1.0).is_err());
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(FortunaPotential::new(snapshot(0.5), RiskWeights::default(), bad).is_err());
        }
        assert!(AdversarialPotential::new(vec![], 1.0, 1.0).is_err());
        assert!(AdversarialPotential::new(vec![f64::INFINITY], 1.0, 1.0).is_err());
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-200, 1e200] {
            assert!(AdversarialPotential::new(vec![0.0], 1.0, bad).is_err());
        }
        assert!(AdversarialPotential::new(vec![0.0], -1.0, 1.0).is_err());
        assert!(AdversarialPotential::new(vec![0.0], f64::INFINITY, 1.0).is_err());
        assert!(AdversarialPotential::new(vec![0.0], f64::MAX, 0.5).is_err());
        assert!(SumPotential::new(vec![]).is_err());
        let a = AdversarialPotential::new(vec![0.0], 1.0, 1.0).unwrap();
        let b = AdversarialPotential::new(vec![0.0, 0.0], 1.0, 1.0).unwrap();
        assert!(SumPotential::new(vec![Box::new(a), Box::new(b)]).is_err());
    }

    #[test]
    fn accumulation_preserves_existing_entries() {
        let risk = FortunaPotential::new(snapshot(0.6), RiskWeights::default(), 2.5).unwrap();
        let adversarial = AdversarialPotential::new(vec![0.3, -0.2], 2.2, 0.8).unwrap();
        let sum =
            SumPotential::new(vec![Box::new(risk.clone()), Box::new(adversarial.clone())]).unwrap();
        for p in [&risk as &dyn Potential, &adversarial, &sum] {
            let q = [-0.5, 1.1];
            let mut gradient = [0.0; 2];
            let mut hessian = [0.0; 4];
            p.gradient(&q, &mut gradient);
            p.hessian(&q, &mut hessian);
            let mut accumulated_gradient = [3.0; 2];
            let mut accumulated_hessian = [4.0; 4];
            p.gradient_add(&q, &mut accumulated_gradient);
            p.hessian_add(&q, &mut accumulated_hessian);
            for i in 0..2 {
                assert!((accumulated_gradient[i] - (3.0 + gradient[i])).abs() < 1e-14);
            }
            for i in 0..4 {
                assert!((accumulated_hessian[i] - (4.0 + hessian[i])).abs() < 1e-14);
            }
        }
    }

    #[test]
    fn trait_signals_invalid_evaluation_without_panicking() {
        let risk = FortunaPotential::new(snapshot(1.0), RiskWeights::default(), f64::MAX).unwrap();
        assert!(risk.value(&[1.0, 1.0]).is_nan());
        let mut out = [f64::MAX; 2];
        risk.gradient_add(&[1.0, 1.0], &mut out);
        assert!(out.iter().all(|x| x.is_nan()));
        risk.gradient(&[f64::NAN, 0.0], &mut out);
        assert!(out.iter().all(|x| x.is_nan()));
        let gaussian = AdversarialPotential::new(vec![-f64::MAX], f64::MAX, 1.0).unwrap();
        assert_eq!(gaussian.value(&[f64::MAX]), 0.0);
        gaussian.gradient(&[f64::MAX], &mut out[..1]);
        assert_eq!(out[0], 0.0);
        gaussian.hessian(&[f64::MAX], &mut out[..1]);
        assert_eq!(out[0], 0.0);
        assert!(gaussian.value(&[f64::NAN]).is_nan());
        let sum = SumPotential::new(vec![Box::new(risk)]).unwrap();
        assert!(sum.try_value(&[1.0, 1.0]).is_err());
        assert!(sum.value(&[1.0, 1.0]).is_nan());
    }

    #[test]
    fn checked_evaluation_rejects_dimensions_nonfinite_and_overflow() {
        let risk = FortunaPotential::new(snapshot(1.0), RiskWeights::default(), f64::MAX).unwrap();
        assert!(risk.try_value(&[1.0, 1.0]).is_err());
        assert!(risk.try_value(&[1.0]).is_err());
        assert!(risk.try_gradient(&[0.0, 0.0], &mut [0.0]).is_err());
        assert!(risk.try_hessian(&[0.0, 0.0], &mut [0.0; 3]).is_err());
        assert!(risk.try_value(&[f64::NAN, 0.0]).is_err());
        let gaussian = AdversarialPotential::new(vec![0.0], 1.0, 1.0).unwrap();
        assert!(gaussian.try_value(&[f64::INFINITY]).is_err());
        assert!(gaussian.try_gradient(&[0.0], &mut []).is_err());
        assert!(gaussian.try_hessian(&[0.0], &mut []).is_err());
        let sum = SumPotential::new(vec![Box::new(gaussian)]).unwrap();
        assert!(sum.try_value(&[]).is_err());
        assert!(sum.try_gradient(&[0.0], &mut []).is_err());
        assert!(sum.try_hessian(&[0.0], &mut []).is_err());
    }
}
