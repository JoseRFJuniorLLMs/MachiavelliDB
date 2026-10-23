//! Mapa tangente `Φ(t) = ∂z(t)/∂z(0)` do fluxo discreto.
//!
//! Pelo teorema de Liouville o fluxo preserva o volume: `det Φ = 1`. Para um integrador
//! simplético vale a condição mais forte `ΦᵀJΦ = J`. Ambas são medidas aqui sobre o
//! produto dos Jacobianos exatos de cada subpasso.

use crate::form::canonical_j;
use crate::linalg::{Matrix, determinant};

#[derive(Clone, Debug)]
pub struct TangentMap {
    n: usize,
    phi: Matrix,
    scratch: Vec<f64>,
}

impl TangentMap {
    pub fn identity(n: usize) -> Self {
        Self {
            n,
            phi: Matrix::identity(2 * n),
            scratch: vec![0.0; 2 * n * n],
        }
    }

    pub fn dim(&self) -> usize {
        self.n
    }

    pub fn matrix(&self) -> &Matrix {
        &self.phi
    }

    /// Kick `p ← p − τ ∇V(q)`: `Φ_p ← Φ_p − τ · ∇²V · Φ_q`.
    pub fn kick(&mut self, hessian: &[f64], tau: f64) {
        let n = self.n;
        let cols = 2 * n;
        // scratch = H · Φ_q   (n × 2n)
        self.scratch.fill(0.0);
        for i in 0..n {
            for k in 0..n {
                let h = hessian[i * n + k];
                if h == 0.0 {
                    continue;
                }
                for j in 0..cols {
                    self.scratch[i * cols + j] += h * self.phi[(k, j)];
                }
            }
        }
        for i in 0..n {
            for j in 0..cols {
                self.phi[(n + i, j)] -= tau * self.scratch[i * cols + j];
            }
        }
    }

    /// Drift `q ← q + h M⁻¹ p`: `Φ_q ← Φ_q + h · M⁻¹ · Φ_p`.
    pub fn drift(&mut self, inv_mass: &[f64], h: f64) {
        let n = self.n;
        for i in 0..n {
            let w = h * inv_mass[i];
            if w == 0.0 {
                continue;
            }
            for j in 0..2 * n {
                let v = self.phi[(n + i, j)];
                self.phi[(i, j)] += w * v;
            }
        }
    }

    /// `max |ΦᵀJΦ − J| / max(1, max|Φ|²)`: defeito simplético relativo à escala de `Φ`.
    pub fn symplectic_defect(&self) -> f64 {
        let j = canonical_j(self.n);
        let d = self
            .phi
            .transpose()
            .mul(&j)
            .mul(&self.phi)
            .sub(&j)
            .max_abs();
        let scale = self.phi.max_abs().powi(2).max(1.0);
        d / scale
    }

    /// `det Φ`: volume de Liouville de um paralelepípedo unitário transportado.
    pub fn liouville_volume(&self) -> f64 {
        determinant(&self.phi)
    }
}
