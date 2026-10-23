//! Potenciais, energia cinética e Hamiltonianos.

use crate::integrators::IntegratorError;
use std::sync::Arc;

/// Energia potencial `V(q)` com gradiente analítico.
pub trait Potential: Send + Sync {
    /// Dimensão de configuração `n`.
    fn dim(&self) -> usize;

    fn value(&self, q: &[f64]) -> f64;

    /// `out ← ∇V(q)`.
    fn gradient(&self, q: &[f64], out: &mut [f64]);

    /// out += grad V(q). O default aloca um vetor temporário; implementações
    /// analíticas podem sobrepor para acumular sem alocações no laço numérico.
    fn gradient_add(&self, q: &[f64], out: &mut [f64]) {
        assert_eq!(out.len(), self.dim());
        let mut derivative = vec![0.0; self.dim()];
        self.gradient(q, &mut derivative);
        for (result, value) in out.iter_mut().zip(derivative) {
            *result += value;
        }
    }

    /// `out ← ∇²V(q)` em ordem de linhas (`n × n`). Por omissão: diferenças centrais
    /// do gradiente, simetrizadas. Implementações com Hessiana analítica devem sobrepor.
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        let n = self.dim();
        assert_eq!(out.len(), n * n);
        let mut qq = q.to_vec();
        let mut gp = vec![0.0; n];
        let mut gm = vec![0.0; n];
        for j in 0..n {
            let h = 1e-6 * q[j].abs().max(1.0);
            qq[j] = q[j] + h;
            self.gradient(&qq, &mut gp);
            qq[j] = q[j] - h;
            self.gradient(&qq, &mut gm);
            qq[j] = q[j];
            for i in 0..n {
                out[i * n + j] = (gp[i] - gm[i]) / (2.0 * h);
            }
        }
        for i in 0..n {
            for j in 0..i {
                let m = 0.5 * (out[i * n + j] + out[j * n + i]);
                out[i * n + j] = m;
                out[j * n + i] = m;
            }
        }
    }

    /// out += Hess V(q). O default aloca uma matriz temporária; implementações
    /// analíticas podem sobrepor para acumular sem alocações no laço numérico.
    fn hessian_add(&self, q: &[f64], out: &mut [f64]) {
        let n = self.dim();
        let len = n.checked_mul(n).expect("dimensão da Hessiana excede usize");
        assert_eq!(out.len(), len);
        let mut derivative = vec![0.0; len];
        self.hessian(q, &mut derivative);
        for (result, value) in out.iter_mut().zip(derivative) {
            *result += value;
        }
    }
}

impl<P: Potential + ?Sized> Potential for &P {
    fn dim(&self) -> usize {
        (**self).dim()
    }
    fn value(&self, q: &[f64]) -> f64 {
        (**self).value(q)
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        (**self).gradient(q, out)
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        (**self).hessian(q, out)
    }
    fn gradient_add(&self, q: &[f64], out: &mut [f64]) {
        (**self).gradient_add(q, out)
    }
    fn hessian_add(&self, q: &[f64], out: &mut [f64]) {
        (**self).hessian_add(q, out)
    }
}

impl<P: Potential + ?Sized> Potential for Box<P> {
    fn dim(&self) -> usize {
        (**self).dim()
    }
    fn value(&self, q: &[f64]) -> f64 {
        (**self).value(q)
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        (**self).gradient(q, out)
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        (**self).hessian(q, out)
    }
    fn gradient_add(&self, q: &[f64], out: &mut [f64]) {
        (**self).gradient_add(q, out)
    }
    fn hessian_add(&self, q: &[f64], out: &mut [f64]) {
        (**self).hessian_add(q, out)
    }
}

impl<P: Potential + ?Sized> Potential for Arc<P> {
    fn dim(&self) -> usize {
        (**self).dim()
    }
    fn value(&self, q: &[f64]) -> f64 {
        (**self).value(q)
    }
    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        (**self).gradient(q, out)
    }
    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        (**self).hessian(q, out)
    }
    fn gradient_add(&self, q: &[f64], out: &mut [f64]) {
        (**self).gradient_add(q, out)
    }
    fn hessian_add(&self, q: &[f64], out: &mut [f64]) {
        (**self).hessian_add(q, out)
    }
}

/// Energia cinética `T(p) = ½ pᵀ M⁻¹ p` com `M = diag(m_1, …, m_n)`.
///
/// Massa infinita (inversa 0) fixa a posição; forças ainda podem alterar o momento.
/// Isso não implementa o isolamento de um nó traído.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagonalKinetic {
    inv_mass: Vec<f64>,
}

impl DiagonalKinetic {
    /// A partir de massas positivas com inversas finitas (+∞ é permitido).
    pub fn from_masses(masses: &[f64]) -> Result<Self, IntegratorError> {
        let inv_mass = masses
            .iter()
            .map(|&m| {
                if m == f64::INFINITY {
                    Ok(0.0)
                } else if m.is_finite() && m > 0.0 {
                    let inverse = 1.0 / m;
                    if inverse.is_finite() {
                        Ok(inverse)
                    } else {
                        Err(IntegratorError::InvalidMass(m))
                    }
                } else {
                    Err(IntegratorError::InvalidMass(m))
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { inv_mass })
    }

    pub fn uniform(n: usize, mass: f64) -> Self {
        Self::from_masses(&vec![mass; n]).expect("massa uniforme positiva")
    }

    pub fn inv_mass(&self) -> &[f64] {
        &self.inv_mass
    }

    pub fn dim(&self) -> usize {
        self.inv_mass.len()
    }

    pub fn energy(&self, p: &[f64]) -> f64 {
        assert_eq!(
            p.len(),
            self.dim(),
            "momento e massa com dimensões diferentes"
        );
        p.iter()
            .zip(&self.inv_mass)
            .map(|(pi, w)| {
                let scaled = pi * w.sqrt();
                (0.5 * scaled) * scaled
            })
            .sum()
    }

    /// `out ← ∂T/∂p = M⁻¹ p`.
    pub fn velocity(&self, p: &[f64], out: &mut [f64]) {
        assert_eq!(
            p.len(),
            self.dim(),
            "momento e massa com dimensões diferentes"
        );
        assert_eq!(
            out.len(),
            self.dim(),
            "saída e massa com dimensões diferentes"
        );
        for ((o, pi), w) in out.iter_mut().zip(p).zip(&self.inv_mass) {
            *o = pi * w;
        }
    }
}

/// Hamiltoniano geral `H(q, p)` (pode não ser separável).
pub trait Hamiltonian: Send + Sync {
    fn dim(&self) -> usize;
    fn energy(&self, q: &[f64], p: &[f64]) -> f64;
    /// `out ← ∂H/∂q`.
    fn grad_q(&self, q: &[f64], p: &[f64], out: &mut [f64]);
    /// `out ← ∂H/∂p`.
    fn grad_p(&self, q: &[f64], p: &[f64], out: &mut [f64]);
}

/// Hamiltoniano separável `H = T(p) + V(q)`: o caso em que Störmer-Verlet e Yoshida
/// são explícitos e simpléticos.
#[derive(Clone, Debug)]
pub struct Separable<P> {
    pub kinetic: DiagonalKinetic,
    pub potential: P,
}

impl<P: Potential> Separable<P> {
    pub fn new(kinetic: DiagonalKinetic, potential: P) -> Self {
        assert_eq!(
            kinetic.dim(),
            potential.dim(),
            "massa e potencial com dimensões diferentes"
        );
        Self { kinetic, potential }
    }

    pub fn energy(&self, q: &[f64], p: &[f64]) -> f64 {
        self.kinetic.energy(p) + self.potential.value(q)
    }

    pub fn dim(&self) -> usize {
        self.kinetic.dim()
    }
}

impl<P: Potential> Hamiltonian for Separable<P> {
    fn dim(&self) -> usize {
        self.kinetic.dim()
    }
    fn energy(&self, q: &[f64], p: &[f64]) -> f64 {
        Separable::energy(self, q, p)
    }
    fn grad_q(&self, q: &[f64], _p: &[f64], out: &mut [f64]) {
        self.potential.gradient(q, out)
    }
    fn grad_p(&self, _q: &[f64], p: &[f64], out: &mut [f64]) {
        self.kinetic.velocity(p, out)
    }
}

impl<P: Potential> crate::poisson::PhaseFunction for Separable<P> {
    fn dim(&self) -> usize {
        self.kinetic.dim()
    }
    fn value(&self, q: &[f64], p: &[f64]) -> f64 {
        self.energy(q, p)
    }
    fn gradient(&self, q: &[f64], p: &[f64], gq: &mut [f64], gp: &mut [f64]) {
        self.potential.gradient(q, gq);
        self.kinetic.velocity(p, gp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::Harmonic;

    #[test]
    fn kinetic_scaling_avoids_artificial_overflow() {
        let kinetic = DiagonalKinetic::from_masses(&[1e300]).unwrap();
        let energy = kinetic.energy(&[1e200]);
        assert!(energy.is_finite());
        assert!((energy / 5e99 - 1.0).abs() < 1e-14);

        let kinetic = DiagonalKinetic::uniform(3, 1.0);
        let energy = kinetic.energy(&[1e154; 3]);
        assert!(energy.is_finite());
        assert!((energy / 1.5e308 - 1.0).abs() < 1e-14);
    }

    #[test]
    fn infinite_mass_has_zero_energy_and_velocity_for_finite_momentum() {
        let kinetic = DiagonalKinetic::from_masses(&[f64::INFINITY]).unwrap();
        assert_eq!(kinetic.energy(&[f64::MAX]), 0.0);
        let mut velocity = [1.0];
        kinetic.velocity(&[f64::MAX], &mut velocity);
        assert_eq!(velocity, [0.0]);
    }

    #[test]
    fn rejects_positive_mass_whose_inverse_overflows() {
        let mass = f64::from_bits(1);
        assert_eq!(
            DiagonalKinetic::from_masses(&[mass]),
            Err(IntegratorError::InvalidMass(mass))
        );
    }

    #[test]
    #[should_panic(expected = "momento e massa")]
    fn kinetic_energy_rejects_wrong_dimension() {
        DiagonalKinetic::uniform(2, 1.0).energy(&[1.0]);
    }

    #[test]
    fn default_additive_derivatives_preserve_existing_values() {
        let potential = Harmonic::isotropic(2, 3.0);
        let mut gradient = [1.0, 2.0];
        potential.gradient_add(&[2.0, -1.0], &mut gradient);
        assert_eq!(gradient, [7.0, -1.0]);
        let mut hessian = [1.0; 4];
        potential.hessian_add(&[2.0, -1.0], &mut hessian);
        assert_eq!(hessian, [4.0, 1.0, 1.0, 4.0]);
    }

    #[test]
    fn wrappers_forward_additive_overrides() {
        struct AdditiveOnly;

        impl Potential for AdditiveOnly {
            fn dim(&self) -> usize {
                1
            }
            fn value(&self, _q: &[f64]) -> f64 {
                0.0
            }
            fn gradient(&self, _q: &[f64], _out: &mut [f64]) {
                panic!("o wrapper deve encaminhar gradient_add");
            }
            fn hessian(&self, _q: &[f64], _out: &mut [f64]) {
                panic!("o wrapper deve encaminhar hessian_add");
            }
            fn gradient_add(&self, _q: &[f64], out: &mut [f64]) {
                out[0] += 3.0;
            }
            fn hessian_add(&self, _q: &[f64], out: &mut [f64]) {
                out[0] += 5.0;
            }
        }

        fn check(potential: impl Potential) {
            let mut gradient = [1.0];
            let mut hessian = [2.0];
            potential.gradient_add(&[0.0], &mut gradient);
            potential.hessian_add(&[0.0], &mut hessian);
            assert_eq!(gradient, [4.0]);
            assert_eq!(hessian, [7.0]);
        }

        check(&AdditiveOnly);
        check(Box::new(AdditiveOnly));
        check(Arc::new(AdditiveOnly));
    }
}
