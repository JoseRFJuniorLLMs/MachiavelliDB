//! Sistemas de referência com Hessiana analítica, usados nos testes de qualificação
//! (M1) e como blocos dos potenciais táticos.

use crate::system::Potential;

/// `V(q) = ½ Σ k_i (q_i − c_i)²`.
#[derive(Clone, Debug, PartialEq)]
pub struct Harmonic {
    pub stiffness: Vec<f64>,
    pub center: Vec<f64>,
}

impl Harmonic {
    pub fn isotropic(n: usize, k: f64) -> Self {
        Self {
            stiffness: vec![k; n],
            center: vec![0.0; n],
        }
    }
}

impl Potential for Harmonic {
    fn dim(&self) -> usize {
        self.stiffness.len()
    }
    fn value(&self, q: &[f64]) -> f64 {
        0.5 * q
            .iter()
            .zip(&self.center)
            .zip(&self.stiffness)
            .map(|((x, c), k)| k * (x - c) * (x - c))
            .sum::<f64>()
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        for i in 0..q.len() {
            out[i] = self.stiffness[i] * (q[i] - self.center[i]);
        }
    }
    fn hessian(&self, _q: &[f64], out: &mut [f64]) {
        let n = self.dim();
        out.fill(0.0);
        for i in 0..n {
            out[i * n + i] = self.stiffness[i];
        }
    }
}

/// Cadeia de Fermi–Pasta–Ulam–Tsingou com extremos fixos:
/// `V = Σ_{i=0}^{n} [½Δ_i² + α/3 Δ_i³ + β/4 Δ_i⁴]`, `Δ_i = q_{i+1} − q_i`, `q_0 = q_{n+1} = 0`.
/// Sistema não linear acoplado, o teste clássico de conservação de longo prazo.
#[derive(Clone, Debug, PartialEq)]
pub struct FpuChain {
    pub n: usize,
    pub alpha: f64,
    pub beta: f64,
}

impl FpuChain {
    fn delta(q: &[f64], i: usize) -> f64 {
        let n = q.len();
        let right = if i < n { q[i] } else { 0.0 };
        let left = if i > 0 { q[i - 1] } else { 0.0 };
        right - left
    }

    fn force(&self, d: f64) -> f64 {
        d + self.alpha * d * d + self.beta * d * d * d
    }

    fn stiffness(&self, d: f64) -> f64 {
        1.0 + 2.0 * self.alpha * d + 3.0 * self.beta * d * d
    }
}

impl Potential for FpuChain {
    fn dim(&self) -> usize {
        self.n
    }
    fn value(&self, q: &[f64]) -> f64 {
        (0..=self.n)
            .map(|i| {
                let d = Self::delta(q, i);
                0.5 * d * d + self.alpha / 3.0 * d * d * d + self.beta / 4.0 * d * d * d * d
            })
            .sum()
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        // Δ_k (índice k da mola à esquerda de q_k, em base 0: mola k liga q_{k-1} a q_k).
        for k in 0..self.n {
            let left = Self::delta(q, k);
            let right = Self::delta(q, k + 1);
            out[k] = self.force(left) - self.force(right);
        }
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        let n = self.n;
        out.fill(0.0);
        for k in 0..n {
            let left = self.stiffness(Self::delta(q, k));
            let right = self.stiffness(Self::delta(q, k + 1));
            out[k * n + k] = left + right;
            if k + 1 < n {
                out[k * n + k + 1] = -right;
                out[(k + 1) * n + k] = -right;
            }
        }
    }
}

/// Potencial de Hénon–Heiles `V = ½(x² + y²) + x²y − y³/3` (n = 2).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HenonHeiles;

impl Potential for HenonHeiles {
    fn dim(&self) -> usize {
        2
    }
    fn value(&self, q: &[f64]) -> f64 {
        let (x, y) = (q[0], q[1]);
        0.5 * (x * x + y * y) + x * x * y - y * y * y / 3.0
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        let (x, y) = (q[0], q[1]);
        out[0] = x + 2.0 * x * y;
        out[1] = y + x * x - y * y;
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        let (x, y) = (q[0], q[1]);
        out[0] = 1.0 + 2.0 * y;
        out[1] = 2.0 * x;
        out[2] = 2.0 * x;
        out[3] = 1.0 - 2.0 * y;
    }
}

/// Pêndulo `V = −cos θ` em cada coordenada (n pêndulos independentes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pendula {
    pub n: usize,
}

impl Potential for Pendula {
    fn dim(&self) -> usize {
        self.n
    }
    fn value(&self, q: &[f64]) -> f64 {
        q.iter().map(|t| -t.cos()).sum()
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        for (o, t) in out.iter_mut().zip(q) {
            *o = t.sin();
        }
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        let n = self.n;
        out.fill(0.0);
        for i in 0..n {
            out[i * n + i] = q[i].cos();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_gradient_and_hessian<P: Potential>(pot: &P, q: &[f64]) {
        let n = pot.dim();
        let mut g = vec![0.0; n];
        pot.gradient(q, &mut g);
        for j in 0..n {
            let h = 1e-6;
            let mut qp = q.to_vec();
            let mut qm = q.to_vec();
            qp[j] += h;
            qm[j] -= h;
            let num = (pot.value(&qp) - pot.value(&qm)) / (2.0 * h);
            assert!((num - g[j]).abs() < 1e-6, "∂V/∂q_{j}: {num} vs {}", g[j]);
        }
        let mut analytic = vec![0.0; n * n];
        pot.hessian(q, &mut analytic);
        // Referência: a Hessiana por diferenças finitas do trait.
        struct NoHess<'a, P>(&'a P);
        impl<P: Potential> Potential for NoHess<'_, P> {
            fn dim(&self) -> usize {
                self.0.dim()
            }
            fn value(&self, q: &[f64]) -> f64 {
                self.0.value(q)
            }
            fn gradient(&self, q: &[f64], out: &mut [f64]) {
                self.0.gradient(q, out)
            }
        }
        let mut numeric = vec![0.0; n * n];
        NoHess(pot).hessian(q, &mut numeric);
        for (a, b) in analytic.iter().zip(&numeric) {
            assert!((a - b).abs() < 1e-5, "Hessiana: {a} vs {b}");
        }
    }

    #[test]
    fn analytic_derivatives_match_finite_differences() {
        check_gradient_and_hessian(
            &FpuChain {
                n: 6,
                alpha: 0.25,
                beta: 0.5,
            },
            &[0.1, -0.2, 0.3, 0.05, -0.1, 0.2],
        );
        check_gradient_and_hessian(&HenonHeiles, &[0.2, -0.3]);
        check_gradient_and_hessian(&Pendula { n: 3 }, &[0.5, 2.0, -1.0]);
        check_gradient_and_hessian(
            &Harmonic {
                stiffness: vec![1.0, 4.0],
                center: vec![0.5, -0.5],
            },
            &[1.0, 2.0],
        );
    }
}
