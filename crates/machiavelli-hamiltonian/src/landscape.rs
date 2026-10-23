//! Local second-order analysis of the potential landscape.
//!
//! A positive Hessian at a stationary point identifies a local minimum. It does
//! not establish a dissipative attractor, a global basin, an irreversible fall,
//! or the position of a separatrix. Those require information about trajectories
//! and the complete dynamics, beyond a local Hessian.

use crate::HamiltonianError;
use machiavelli_symplectic::Potential;

/// Classification supported by the gradient and the local Hessian signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandscapeKind {
    /// Stationary, with a positive-definite Hessian within the tolerances.
    Minimum,
    /// Stationary, with a negative-definite Hessian within the tolerances.
    Maximum,
    /// Stationary, with both positive and negative curvature and no zero modes.
    Saddle,
    /// Stationary, with at least one curvature indistinguishable from zero.
    /// Higher derivatives are needed to classify the flat directions.
    Degenerate,
    /// The gradient norm is larger than the stationarity tolerance.
    NonStationary,
}

/// Numerical thresholds for [`analyze`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandscapeTolerances {
    /// Absolute upper bound on the Euclidean gradient norm for stationarity.
    pub gradient: f64,
    /// Absolute eigenvalue magnitude regarded as a zero-curvature direction.
    pub eigenvalue: f64,
    /// Maximum Hessian asymmetry, relative to its largest absolute entry.
    pub symmetry: f64,
    /// Upper bound on the off-diagonal Frobenius norm after normalizing the
    /// Hessian by its largest absolute entry. Must be strictly between 0 and 1.
    pub jacobi: f64,
    /// Maximum number of complete cyclic Jacobi sweeps; must be nonzero.
    pub max_sweeps: usize,
}

impl Default for LandscapeTolerances {
    fn default() -> Self {
        Self {
            gradient: 1e-8,
            eigenvalue: 1e-8,
            symmetry: 1e-10,
            jacobi: 1e-14,
            max_sweeps: 100,
        }
    }
}

impl LandscapeTolerances {
    fn validate(self) -> Result<(), HamiltonianError> {
        for (field, value) in [
            ("gradient_tolerance", self.gradient),
            ("eigenvalue_tolerance", self.eigenvalue),
            ("symmetry_tolerance", self.symmetry),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(HamiltonianError::InvalidParameter { field, value });
            }
        }
        if !self.jacobi.is_finite() || self.jacobi <= 0.0 || self.jacobi >= 1.0 {
            return Err(HamiltonianError::InvalidParameter {
                field: "jacobi_tolerance",
                value: self.jacobi,
            });
        }
        if self.max_sweeps == 0 {
            return Err(HamiltonianError::InvalidParameter {
                field: "max_sweeps",
                value: 0.0,
            });
        }
        Ok(())
    }
}

/// A local diagnostic; the direction counts describe Hessian eigenvalue signs,
/// not globally stable or unstable manifolds. Eigenvalues are sorted ascending.
#[derive(Clone, Debug, PartialEq)]
pub struct LandscapeReport {
    pub kind: LandscapeKind,
    pub potential_energy: f64,
    pub gradient_norm: f64,
    pub eigenvalues: Vec<f64>,
    pub negative_directions: usize,
    pub zero_directions: usize,
    pub positive_directions: usize,
    pub jacobi_sweeps: usize,
}

/// Analyze `V(q)`, its gradient, and its symmetric Hessian at one point.
///
/// Rejects non-finite inputs/outputs, inconsistent dimensions, invalid thresholds,
/// and Hessians whose relative asymmetry exceeds `tolerances.symmetry`. Accepted
/// roundoff-level asymmetry is averaged before solving. The Jacobi solver scales
/// the Hessian to avoid overflow and verifies convergence before returning; an
/// unrepresentable energy, gradient norm, or eigenvalue is an error.
///
/// The curvature threshold is absolute in the potential's units. The Jacobi
/// residual must also leave every eigenvalue classification unambiguous at the
/// absolute curvature threshold; otherwise sweeps continue or convergence fails.
/// This bounds uncertainty from the remaining off-diagonal entries, not floating-
/// point error accumulated by the rotations. Very large dynamic ranges and
/// near-threshold eigenvalues remain limited by derivative and machine precision.
pub fn analyze<P: Potential + ?Sized>(
    potential: &P,
    q: &[f64],
    tolerances: LandscapeTolerances,
) -> Result<LandscapeReport, HamiltonianError> {
    tolerances.validate()?;
    let n = potential.dim();
    if q.len() != n {
        return Err(HamiltonianError::DimensionMismatch {
            expected: n,
            actual: q.len(),
        });
    }
    if n == 0 {
        return Err(HamiltonianError::InvalidParameter {
            field: "dimension",
            value: 0.0,
        });
    }
    let elements = n.checked_mul(n).ok_or(HamiltonianError::InvalidParameter {
        field: "dimension",
        value: n as f64,
    })?;
    require_finite(q, "q")?;
    let potential_energy = potential.value(q);
    require_finite(&[potential_energy], "potential_energy")?;
    let mut gradient = zero_vector(n)?;
    potential.gradient(q, &mut gradient);
    require_finite(&gradient, "gradient")?;
    // hypot keeps representable norms finite even when their squares overflow.
    let gradient_norm = gradient.iter().fold(0.0_f64, |norm, &x| norm.hypot(x));
    require_finite(&[gradient_norm], "gradient_norm")?;
    let mut hessian = zero_vector(elements)?;
    potential.hessian(q, &mut hessian);
    require_finite(&hessian, "hessian")?;
    let (eigenvalues, jacobi_sweeps) = symmetric_eigenvalues(hessian, n, tolerances)?;
    let mut negative_directions = 0;
    let mut zero_directions = 0;
    let mut positive_directions = 0;
    for &eigenvalue in &eigenvalues {
        if eigenvalue.abs() <= tolerances.eigenvalue {
            zero_directions += 1;
        } else if eigenvalue < 0.0 {
            negative_directions += 1;
        } else {
            positive_directions += 1;
        }
    }
    let kind = if gradient_norm > tolerances.gradient {
        LandscapeKind::NonStationary
    } else if zero_directions > 0 {
        LandscapeKind::Degenerate
    } else if positive_directions == n {
        LandscapeKind::Minimum
    } else if negative_directions == n {
        LandscapeKind::Maximum
    } else {
        LandscapeKind::Saddle
    };
    Ok(LandscapeReport {
        kind,
        potential_energy,
        gradient_norm,
        eigenvalues,
        negative_directions,
        zero_directions,
        positive_directions,
        jacobi_sweeps,
    })
}

fn zero_vector(len: usize) -> Result<Vec<f64>, HamiltonianError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| HamiltonianError::InvalidParameter {
            field: "allocation_size",
            value: len as f64,
        })?;
    values.resize(len, 0.0);
    Ok(values)
}

fn require_finite(values: &[f64], field: &'static str) -> Result<(), HamiltonianError> {
    if values.iter().any(|x| !x.is_finite()) {
        Err(HamiltonianError::NonFinite { field })
    } else {
        Ok(())
    }
}

fn off_diagonal_norm(matrix: &[f64], n: usize) -> f64 {
    let mut norm = 0.0_f64;
    for row in 0..n {
        for col in row + 1..n {
            let value = matrix[row * n + col];
            // Each off-diagonal entry occurs twice in the full Frobenius norm.
            norm = norm.hypot(value).hypot(value);
        }
    }
    norm
}

// For a symmetric matrix, the off-diagonal Frobenius norm bounds the spectral
// distance from its diagonal matrix (Weyl's bound). Resolve every interval
// [d-residual, d+residual] completely into one curvature class before stopping.
// Difference comparisons avoid overflowing interval endpoints or unscaling the
// residual. The normalized absolute threshold may underflow to 0 or overflow to
// infinity; these limiting cases do not require overflowing physical bounds.
fn curvature_classification_resolved(
    matrix: &[f64],
    n: usize,
    residual: f64,
    normalized_threshold: f64,
) -> bool {
    if residual == 0.0 {
        // No off-diagonal uncertainty remains, including for exact thresholds.
        return true;
    }
    (0..n).all(|i| {
        let magnitude = matrix[i * n + i].abs();
        (magnitude > normalized_threshold && residual < magnitude - normalized_threshold)
            || (magnitude <= normalized_threshold && residual <= normalized_threshold - magnitude)
    })
}

fn symmetric_eigenvalues(
    mut matrix: Vec<f64>,
    n: usize,
    tolerances: LandscapeTolerances,
) -> Result<(Vec<f64>, usize), HamiltonianError> {
    let scale = matrix.iter().fold(0.0_f64, |max, x| max.max(x.abs()));
    if scale == 0.0 {
        return Ok((zero_vector(n)?, 0));
    }
    for value in &mut matrix {
        *value /= scale;
    }
    for row in 0..n {
        for col in row + 1..n {
            let a = matrix[row * n + col];
            let b = matrix[col * n + row];
            let asymmetry = (a - b).abs();
            if asymmetry > tolerances.symmetry {
                return Err(HamiltonianError::InvalidParameter {
                    field: "hessian_relative_asymmetry",
                    value: asymmetry,
                });
            }
            let average = 0.5 * a + 0.5 * b;
            matrix[row * n + col] = average;
            matrix[col * n + row] = average;
        }
    }
    let normalized_threshold = tolerances.eigenvalue / scale;
    for sweep in 0..=tolerances.max_sweeps {
        let residual = off_diagonal_norm(&matrix, n);
        require_finite(&[residual], "jacobi_residual")?;
        if residual <= tolerances.jacobi
            && curvature_classification_resolved(&matrix, n, residual, normalized_threshold)
        {
            let mut eigenvalues: Vec<f64> = (0..n)
                .map(|index| matrix[index * n + index] * scale)
                .collect();
            require_finite(&eigenvalues, "eigenvalues")?;
            eigenvalues.sort_by(f64::total_cmp);
            return Ok((eigenvalues, sweep));
        }
        if sweep == tolerances.max_sweeps {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = matrix[p * n + q];
                if apq == 0.0 {
                    continue;
                }
                let app = matrix[p * n + p];
                let aqq = matrix[q * n + q];
                let delta = 0.5 * aqq - 0.5 * app;
                // This avoids squaring a large cotangent or dividing by a tiny
                // off-diagonal entry. The denominator has no cancellation.
                let magnitude = delta.hypot(apq);
                let denominator = if delta >= 0.0 {
                    delta + magnitude
                } else {
                    delta - magnitude
                };
                let t = apq / denominator;
                let c = (1.0 + t * t).sqrt().recip();
                let s = t * c;
                for k in 0..n {
                    if k == p || k == q {
                        continue;
                    }
                    let akp = matrix[k * n + p];
                    let akq = matrix[k * n + q];
                    let new_kp = c * akp - s * akq;
                    let new_kq = s * akp + c * akq;
                    require_finite(&[new_kp, new_kq], "jacobi_rotation")?;
                    matrix[k * n + p] = new_kp;
                    matrix[p * n + k] = new_kp;
                    matrix[k * n + q] = new_kq;
                    matrix[q * n + k] = new_kq;
                }
                let new_app = app - t * apq;
                let new_aqq = aqq + t * apq;
                require_finite(&[new_app, new_aqq], "jacobi_rotation")?;
                matrix[p * n + p] = new_app;
                matrix[q * n + q] = new_aqq;
                matrix[p * n + q] = 0.0;
                matrix[q * n + p] = 0.0;
            }
        }
    }
    Err(HamiltonianError::NotConverged {
        iterations: tolerances.max_sweeps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Quadratic {
        hessian: Vec<f64>,
        linear: Vec<f64>,
    }

    impl Quadratic {
        fn diagonal(values: &[f64]) -> Self {
            let n = values.len();
            let mut hessian = vec![0.0; n * n];
            for (i, &value) in values.iter().enumerate() {
                hessian[i * n + i] = value;
            }
            Self {
                hessian,
                linear: vec![0.0; n],
            }
        }
    }

    impl Potential for Quadratic {
        fn dim(&self) -> usize {
            self.linear.len()
        }

        fn value(&self, q: &[f64]) -> f64 {
            let n = self.dim();
            let mut energy = 0.0;
            for i in 0..n {
                energy += self.linear[i] * q[i];
                for j in 0..n {
                    energy += 0.5 * q[i] * self.hessian[i * n + j] * q[j];
                }
            }
            energy
        }

        fn gradient(&self, q: &[f64], out: &mut [f64]) {
            let n = self.dim();
            out.copy_from_slice(&self.linear);
            for (i, value) in out.iter_mut().enumerate() {
                for (j, &coordinate) in q.iter().enumerate() {
                    *value += self.hessian[i * n + j] * coordinate;
                }
            }
        }

        fn hessian(&self, _q: &[f64], out: &mut [f64]) {
            out.copy_from_slice(&self.hessian);
        }
    }

    #[test]
    fn harmonic_minimum_has_only_positive_curvature() {
        let potential = Quadratic::diagonal(&[1.0, 4.0]);
        let report = analyze(&potential, &[0.0, 0.0], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::Minimum);
        assert_eq!(report.eigenvalues, [1.0, 4.0]);
        assert_eq!(report.positive_directions, 2);
        assert_eq!(report.negative_directions, 0);
        assert_eq!(report.zero_directions, 0);
        assert_eq!(report.gradient_norm, 0.0);
        assert_eq!(report.potential_energy, 0.0);
        assert_eq!(report.jacobi_sweeps, 0);
    }

    #[test]
    fn saddle_and_maximum_have_correct_signatures() {
        let saddle = Quadratic::diagonal(&[-2.0, 5.0]);
        let report = analyze(&saddle, &[0.0, 0.0], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::Saddle);
        assert_eq!(report.negative_directions, 1);
        assert_eq!(report.positive_directions, 1);
        let maximum = Quadratic::diagonal(&[-2.0, -7.0]);
        let report = analyze(&maximum, &[0.0, 0.0], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::Maximum);
        assert_eq!(report.eigenvalues, [-7.0, -2.0]);
        assert_eq!(report.negative_directions, 2);
    }

    #[test]
    fn flat_directions_are_degenerate_and_a_gradient_is_nonstationary() {
        let potential = Quadratic::diagonal(&[0.0, 2.0]);
        let report = analyze(&potential, &[0.0, 0.0], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::Degenerate);
        assert_eq!(report.zero_directions, 1);
        let report = analyze(&potential, &[0.0, 3.0], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::NonStationary);
        assert_eq!(report.gradient_norm, 6.0);
        assert_eq!(report.potential_energy, 9.0);
        let flat = Quadratic::diagonal(&[0.0, 0.0]);
        let report = analyze(&flat, &[0.0, 0.0], Default::default()).unwrap();
        assert_eq!(report.zero_directions, 2);
        assert_eq!(report.kind, LandscapeKind::Degenerate);
    }

    #[test]
    fn rotated_coupled_hessian_uses_eigenvalues_instead_of_diagonal_signs() {
        let potential = Quadratic {
            hessian: vec![1.5, 2.5, 2.5, 1.5],
            linear: vec![0.0; 2],
        };
        let report = analyze(&potential, &[0.0, 0.0], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::Saddle);
        assert!((report.eigenvalues[0] + 1.0).abs() < 1e-13);
        assert!((report.eigenvalues[1] - 4.0).abs() < 1e-13);
        assert!(report.jacobi_sweeps > 0);
    }

    #[test]
    fn dense_three_dimensional_hessian_converges() {
        // I + vv^T for v=(1,2,3) has spectrum (1,1,15).
        let potential = Quadratic {
            hessian: vec![2.0, 2.0, 3.0, 2.0, 5.0, 6.0, 3.0, 6.0, 10.0],
            linear: vec![0.0; 3],
        };
        let report = analyze(&potential, &[0.0; 3], Default::default()).unwrap();
        for (actual, expected) in report.eigenvalues.iter().zip([1.0, 1.0, 15.0]) {
            assert!((actual - expected).abs() < 1e-12);
        }
        assert_eq!(report.kind, LandscapeKind::Minimum);
    }

    #[test]
    fn incomplete_jacobi_solve_returns_an_error() {
        let potential = Quadratic {
            hessian: vec![2.0, 1.0, 0.5, 1.0, 3.0, 0.7, 0.5, 0.7, 4.0],
            linear: vec![0.0; 3],
        };
        let tolerances = LandscapeTolerances {
            max_sweeps: 1,
            ..Default::default()
        };
        assert!(matches!(
            analyze(&potential, &[0.0; 3], tolerances),
            Err(HamiltonianError::NotConverged { iterations: 1 })
        ));
    }

    #[test]
    fn rejects_nonfinite_inputs_derivatives_and_invalid_configuration() {
        let potential = Quadratic::diagonal(&[1.0]);
        assert!(matches!(
            analyze(&potential, &[f64::NAN], Default::default()),
            Err(HamiltonianError::NonFinite { field: "q" })
        ));
        assert!(matches!(
            analyze(&potential, &[], Default::default()),
            Err(HamiltonianError::DimensionMismatch {
                expected: 1,
                actual: 0
            })
        ));
        for invalid in [f64::NAN, f64::INFINITY, -1.0] {
            let tolerances = LandscapeTolerances {
                eigenvalue: invalid,
                ..Default::default()
            };
            assert!(matches!(
                analyze(&potential, &[0.0], tolerances),
                Err(HamiltonianError::InvalidParameter { .. })
            ));
        }
        let tolerances = LandscapeTolerances {
            max_sweeps: 0,
            ..Default::default()
        };
        assert!(matches!(
            analyze(&potential, &[0.0], tolerances),
            Err(HamiltonianError::InvalidParameter {
                field: "max_sweeps",
                ..
            })
        ));
        let nonfinite = Quadratic {
            hessian: vec![1.0],
            linear: vec![f64::NAN],
        };
        assert!(matches!(
            analyze(&nonfinite, &[0.0], Default::default()),
            Err(HamiltonianError::NonFinite { .. })
        ));
    }

    #[test]
    fn rejects_asymmetric_hessian_instead_of_inventing_a_signature() {
        let potential = Quadratic {
            hessian: vec![1.0, 1.0, 0.0, 1.0],
            linear: vec![0.0; 2],
        };
        assert!(matches!(
            analyze(&potential, &[0.0; 2], Default::default()),
            Err(HamiltonianError::InvalidParameter {
                field: "hessian_relative_asymmetry",
                ..
            })
        ));
    }

    #[test]
    fn scaled_solver_and_stable_gradient_norm_avoid_intermediate_overflow() {
        let potential = Quadratic::diagonal(&[1e300, 2e300]);
        let report = analyze(&potential, &[0.0; 2], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::Minimum);
        assert_eq!(report.eigenvalues, [1e300, 2e300]);
        let potential = Quadratic {
            hessian: vec![1.0, 0.0, 0.0, 1.0],
            linear: vec![1e200; 2],
        };
        let report = analyze(&potential, &[0.0; 2], Default::default()).unwrap();
        assert!((report.gradient_norm / 1e200 - 2.0_f64.sqrt()).abs() < 1e-14);
        assert_eq!(report.kind, LandscapeKind::NonStationary);
    }

    #[test]
    fn small_relative_residual_does_not_hide_large_negative_curvature() {
        let potential = Quadratic {
            hessian: vec![1.0, 1e285, 1e285, 1e300],
            linear: vec![0.0; 2],
        };
        let report = analyze(&potential, &[0.0; 2], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::Saddle);
        assert_eq!(report.negative_directions, 1);
        assert_eq!(report.positive_directions, 1);
        assert!((report.eigenvalues[0] / -1e270 - 1.0).abs() < 1e-13);
        assert!(report.jacobi_sweeps > 0);
    }

    #[test]
    fn dense_hessian_resolves_curvature_across_extreme_scales() {
        let potential = Quadratic {
            hessian: vec![1.0, 1e285, 1e285, 1e285, 1e300, 2e299, 1e285, 2e299, 5e299],
            linear: vec![0.0; 3],
        };
        let report = analyze(&potential, &[0.0; 3], Default::default()).unwrap();
        assert_eq!(report.kind, LandscapeKind::Saddle);
        assert_eq!(report.negative_directions, 1);
        assert_eq!(report.positive_directions, 2);
        // The small eigenvalue is the Schur complement to leading order.
        assert!((report.eigenvalues[0] / (-1e270 * 1.1 / 0.46) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn absolute_threshold_normalization_handles_extreme_ratios() {
        let tiny_threshold = LandscapeTolerances {
            eigenvalue: 1e-300,
            ..Default::default()
        };
        let potential = Quadratic::diagonal(&[1.0, 1e300]);
        let report = analyze(&potential, &[0.0; 2], tiny_threshold).unwrap();
        assert_eq!(report.kind, LandscapeKind::Minimum);
        let large_threshold = LandscapeTolerances {
            eigenvalue: 1e300,
            ..Default::default()
        };
        let potential = Quadratic::diagonal(&[1e-300, 2e-300]);
        let report = analyze(&potential, &[0.0; 2], large_threshold).unwrap();
        assert_eq!(report.kind, LandscapeKind::Degenerate);
        assert_eq!(report.zero_directions, 2);
    }

    #[test]
    fn rejects_eigenvalues_that_cannot_be_represented() {
        let potential = Quadratic {
            hessian: vec![f64::MAX; 4],
            linear: vec![0.0; 2],
        };
        assert!(matches!(
            analyze(&potential, &[0.0; 2], Default::default()),
            Err(HamiltonianError::NonFinite {
                field: "eigenvalues"
            })
        ));
    }
}
