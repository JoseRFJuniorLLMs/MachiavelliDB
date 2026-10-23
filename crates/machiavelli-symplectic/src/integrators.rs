//! Integradores simpléticos explícitos (Störmer-Verlet, Yoshida-4) para Hamiltonianos
//! separáveis e o leapfrog generalizado (implícito) para os não separáveis.

use crate::system::{Hamiltonian, Potential, Separable};
use crate::tangent::TangentMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum IntegratorError {
    #[error("massa inválida: {0} (exige m > 0 ou +∞)")]
    InvalidMass(f64),
    #[error("dimensões incoerentes: esperado {expected}, recebido {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("iteração de ponto fixo não convergiu em {iterations} passos (resíduo {residual:e})")]
    NotConverged { iterations: usize, residual: f64 },
    #[error("passo de integração inválido: {0}")]
    InvalidStep(f64),
    #[error("estado de integração contém coordenadas não finitas")]
    NonFiniteState,
}

/// Esquema de composição.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Scheme {
    /// Störmer-Verlet kick-drift-kick, 2ª ordem.
    StormerVerlet,
    /// Composição *triple jump* de Yoshida (1990), 4ª ordem.
    Yoshida4,
}

impl Scheme {
    pub fn order(self) -> u32 {
        match self {
            Scheme::StormerVerlet => 2,
            Scheme::Yoshida4 => 4,
        }
    }

    /// Frações do passo usadas em cada subpasso de Verlet.
    pub fn weights(self) -> &'static [f64] {
        match self {
            Scheme::StormerVerlet => &[1.0],
            Scheme::Yoshida4 => &YOSHIDA4,
        }
    }
}

/// `w1 = 1/(2 − 2^{1/3})`, `w0 = −2^{1/3}/(2 − 2^{1/3})`.
const YOSHIDA4: [f64; 3] = {
    // 2^{1/3} com precisão de f64 (const fn não tem cbrt).
    const CBRT2: f64 = 1.259_921_049_894_873_2;
    const W1: f64 = 1.0 / (2.0 - CBRT2);
    const W0: f64 = -CBRT2 / (2.0 - CBRT2);
    [W1, W0, W1]
};

/// Integrador explícito para `H = ½pᵀM⁻¹p + V(q)`, sem alocações no laço.
///
/// `run` reaproveita a força do fim de cada subpasso no início do seguinte (FSAL),
/// pelo que custa um gradiente por subpasso.
/// Resultados não finitos são recusados e invalidam o cache. Uma falha pode deixar
/// q, p e o mapa tangente parcialmente atualizados; não há rollback.
pub struct Integrator<'s, P: Potential> {
    sys: &'s Separable<P>,
    scheme: Scheme,
    force: Vec<f64>,
    force_valid: bool,
    compensated: bool,
    comp_q: Vec<f64>,
    comp_p: Vec<f64>,
    hessian: Vec<f64>,
}

impl<P: Potential> std::fmt::Debug for Integrator<'_, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Integrator")
            .field("scheme", &self.scheme)
            .field("dim", &self.sys.dim())
            .field("compensated", &self.compensated)
            .finish()
    }
}

impl<'s, P: Potential> Integrator<'s, P> {
    pub fn new(sys: &'s Separable<P>, scheme: Scheme) -> Self {
        let n = sys.dim();
        Self {
            sys,
            scheme,
            force: vec![0.0; n],
            force_valid: false,
            compensated: false,
            comp_q: vec![0.0; n],
            comp_p: vec![0.0; n],
            hessian: Vec::new(),
        }
    }

    /// Soma compensada (Kahan) nas atualizações de `q` e `p`: reduz o passeio aleatório
    /// do arredondamento em integrações de milhões de passos.
    pub fn compensated(mut self) -> Self {
        self.compensated = true;
        self
    }

    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// Descarta a força em cache (obrigatório se `q` foi alterado por fora entre chamadas).
    pub fn invalidate(&mut self) {
        self.force_valid = false;
        self.comp_q.fill(0.0);
        self.comp_p.fill(0.0);
    }

    fn check(&mut self, q: &[f64], p: &[f64], dt: f64) -> Result<(), IntegratorError> {
        let n = self.sys.dim();
        if q.len() != n || p.len() != n {
            self.invalidate();
            return Err(IntegratorError::DimensionMismatch {
                expected: n,
                actual: q.len().max(p.len()),
            });
        }
        if !dt.is_finite() {
            self.invalidate();
            return Err(IntegratorError::InvalidStep(dt));
        }
        self.check_finite(q, p)
    }

    fn check_finite(&mut self, q: &[f64], p: &[f64]) -> Result<(), IntegratorError> {
        if q.iter().chain(p).any(|value| !value.is_finite()) {
            self.invalidate();
            return Err(IntegratorError::NonFiniteState);
        }
        Ok(())
    }

    #[inline]
    fn add(compensated: bool, x: &mut f64, c: &mut f64, delta: f64) {
        if compensated {
            let y = delta - *c;
            let t = *x + y;
            *c = (t - *x) - y;
            *x = t;
        } else {
            *x += delta;
        }
    }

    /// Um subpasso de Verlet com passo `h`.
    fn verlet(&mut self, q: &mut [f64], p: &mut [f64], h: f64) {
        if !self.force_valid {
            self.sys.potential.gradient(q, &mut self.force);
        }
        let half = 0.5 * h;
        let inv_m = self.sys.kinetic.inv_mass();
        for i in 0..q.len() {
            Self::add(
                self.compensated,
                &mut p[i],
                &mut self.comp_p[i],
                -half * self.force[i],
            );
        }
        for i in 0..q.len() {
            Self::add(
                self.compensated,
                &mut q[i],
                &mut self.comp_q[i],
                h * (p[i] * inv_m[i]),
            );
        }
        self.sys.potential.gradient(q, &mut self.force);
        self.force_valid = true;
        for i in 0..q.len() {
            Self::add(
                self.compensated,
                &mut p[i],
                &mut self.comp_p[i],
                -half * self.force[i],
            );
        }
    }

    /// Um passo completo, autónomo (recalcula a força inicial).
    pub fn step(&mut self, q: &mut [f64], p: &mut [f64], dt: f64) -> Result<(), IntegratorError> {
        self.check(q, p, dt)?;
        self.force_valid = false;
        for &w in self.scheme.weights() {
            self.verlet(q, p, w * dt);
        }
        self.check_finite(q, p)
    }

    /// `steps` passos seguidos com FSAL.
    pub fn run(
        &mut self,
        q: &mut [f64],
        p: &mut [f64],
        dt: f64,
        steps: usize,
    ) -> Result<(), IntegratorError> {
        self.check(q, p, dt)?;
        self.force_valid = false;
        for _ in 0..steps {
            for &w in self.scheme.weights() {
                self.verlet(q, p, w * dt);
            }
            self.check_finite(q, p)?;
        }
        Ok(())
    }

    /// Como `run`, chamando `observe(passo, q, p)` a cada `every` passos.
    pub fn run_observed<F: FnMut(usize, &[f64], &[f64])>(
        &mut self,
        q: &mut [f64],
        p: &mut [f64],
        dt: f64,
        steps: usize,
        every: usize,
        mut observe: F,
    ) -> Result<(), IntegratorError> {
        self.check(q, p, dt)?;
        self.force_valid = false;
        let every = every.max(1);
        for s in 1..=steps {
            for &w in self.scheme.weights() {
                self.verlet(q, p, w * dt);
            }
            self.check_finite(q, p)?;
            if s.is_multiple_of(every) {
                observe(s, q, p);
            }
        }
        Ok(())
    }

    /// Integra e propaga o mapa tangente `Φ = ∂z(t)/∂z(0)` pelo produto exato dos
    /// Jacobianos de cada kick e drift (cada um é uma matriz de cisalhamento simplética).
    pub fn run_with_tangent(
        &mut self,
        q: &mut [f64],
        p: &mut [f64],
        dt: f64,
        steps: usize,
        tangent: &mut TangentMap,
    ) -> Result<(), IntegratorError> {
        self.check(q, p, dt)?;
        let n = q.len();
        if tangent.dim() != n {
            return Err(IntegratorError::DimensionMismatch {
                expected: n,
                actual: tangent.dim(),
            });
        }
        self.hessian.resize(n * n, 0.0);
        self.force_valid = false;
        for _ in 0..steps {
            for &w in self.scheme.weights() {
                let h = w * dt;
                self.sys.potential.hessian(q, &mut self.hessian);
                tangent.kick(&self.hessian, 0.5 * h);
                tangent.drift(self.sys.kinetic.inv_mass(), h);
                self.verlet(q, p, h);
                self.sys.potential.hessian(q, &mut self.hessian);
                tangent.kick(&self.hessian, 0.5 * h);
            }
            self.check_finite(q, p)?;
        }
        Ok(())
    }
}

/// Leapfrog generalizado (Störmer-Verlet implícito) para `H(q, p)` não separável:
///
/// ```text
/// p½   = p  − h/2 · ∂H/∂q(q,  p½)                      (ponto fixo)
/// q'   = q  + h/2 · [∂H/∂p(q, p½) + ∂H/∂p(q', p½)]     (ponto fixo)
/// p'   = p½ − h/2 · ∂H/∂q(q', p½)
/// ```
///
/// É simplético e de 2ª ordem; serve quando a massa estratégica depende de `q`.
#[derive(Clone, Copy, Debug)]
pub struct GeneralizedLeapfrog {
    pub tolerance: f64,
    pub max_iterations: usize,
}

impl Default for GeneralizedLeapfrog {
    fn default() -> Self {
        Self {
            tolerance: 1e-15,
            max_iterations: 100,
        }
    }
}

impl GeneralizedLeapfrog {
    /// Um passo; devolve o total de iterações de ponto fixo usadas.
    pub fn step<H: Hamiltonian + ?Sized>(
        &self,
        ham: &H,
        q: &mut [f64],
        p: &mut [f64],
        dt: f64,
    ) -> Result<usize, IntegratorError> {
        let n = ham.dim();
        if q.len() != n || p.len() != n {
            return Err(IntegratorError::DimensionMismatch {
                expected: n,
                actual: q.len().max(p.len()),
            });
        }
        let half = 0.5 * dt;
        let mut iterations = 0;
        let mut g = vec![0.0; n];
        let mut vel0 = vec![0.0; n];
        let mut vel1 = vec![0.0; n];

        // 1) p½
        let mut p_half = p.to_vec();
        let mut next = vec![0.0; n];
        let mut converged = false;
        for _ in 0..self.max_iterations {
            iterations += 1;
            ham.grad_q(q, &p_half, &mut g);
            let mut residual: f64 = 0.0;
            for i in 0..n {
                next[i] = p[i] - half * g[i];
                residual = residual.max((next[i] - p_half[i]).abs() / (1.0 + next[i].abs()));
            }
            std::mem::swap(&mut p_half, &mut next);
            if residual <= self.tolerance {
                converged = true;
                break;
            }
        }
        if !converged {
            return Err(IntegratorError::NotConverged {
                iterations,
                residual: f64::NAN,
            });
        }

        // 2) q'
        ham.grad_p(q, &p_half, &mut vel0);
        let mut q_new: Vec<f64> = (0..n).map(|i| q[i] + dt * vel0[i]).collect();
        converged = false;
        for _ in 0..self.max_iterations {
            iterations += 1;
            ham.grad_p(&q_new, &p_half, &mut vel1);
            let mut residual: f64 = 0.0;
            for i in 0..n {
                next[i] = q[i] + half * (vel0[i] + vel1[i]);
                residual = residual.max((next[i] - q_new[i]).abs() / (1.0 + next[i].abs()));
            }
            std::mem::swap(&mut q_new, &mut next);
            if residual <= self.tolerance {
                converged = true;
                break;
            }
        }
        if !converged {
            return Err(IntegratorError::NotConverged {
                iterations,
                residual: f64::NAN,
            });
        }

        // 3) p'
        ham.grad_q(&q_new, &p_half, &mut g);
        for i in 0..n {
            p[i] = p_half[i] - half * g[i];
        }
        q.copy_from_slice(&q_new);
        Ok(iterations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::DiagonalKinetic;
    use crate::systems::{Harmonic, HenonHeiles};

    #[test]
    fn yoshida_weights_sum_to_one() {
        let s: f64 = Scheme::Yoshida4.weights().iter().sum();
        assert!((s - 1.0).abs() < 1e-15);
        let cbrt2 = 2f64.cbrt();
        assert!((Scheme::Yoshida4.weights()[0] - 1.0 / (2.0 - cbrt2)).abs() < 1e-15);
    }

    #[test]
    fn harmonic_oscillator_period() {
        // ω = 1: depois de t = 2π o estado volta ao início (erro de fase O(h^ordem)).
        let sys = Separable::new(
            DiagonalKinetic::uniform(1, 1.0),
            Harmonic::isotropic(1, 1.0),
        );
        for (scheme, tol) in [(Scheme::StormerVerlet, 1e-5), (Scheme::Yoshida4, 1e-9)] {
            let (mut q, mut p) = ([1.0], [0.0]);
            let steps = 10_000;
            let dt = 2.0 * std::f64::consts::PI / steps as f64;
            Integrator::new(&sys, scheme)
                .run(&mut q, &mut p, dt, steps)
                .unwrap();
            assert!(
                (q[0] - 1.0).abs() < tol && p[0].abs() < tol,
                "{scheme:?}: {q:?} {p:?}"
            );
        }
    }

    #[test]
    fn convergence_order_matches_scheme() {
        let sys = Separable::new(DiagonalKinetic::uniform(2, 1.0), HenonHeiles);
        let reference = {
            let (mut q, mut p) = ([0.1, 0.1], [0.2, -0.1]);
            Integrator::new(&sys, Scheme::Yoshida4)
                .run(&mut q, &mut p, 1e-4, 10_000)
                .unwrap();
            [q[0], q[1], p[0], p[1]]
        };
        for scheme in [Scheme::StormerVerlet, Scheme::Yoshida4] {
            let err = |dt: f64| {
                let (mut q, mut p) = ([0.1, 0.1], [0.2, -0.1]);
                Integrator::new(&sys, scheme)
                    .run(&mut q, &mut p, dt, (1.0 / dt).round() as usize)
                    .unwrap();
                [q[0], q[1], p[0], p[1]]
                    .iter()
                    .zip(&reference)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0, f64::max)
            };
            let ratio = err(0.02) / err(0.01);
            let observed = ratio.log2();
            assert!(
                (observed - scheme.order() as f64).abs() < 0.3,
                "{scheme:?}: ordem observada {observed}"
            );
        }
    }

    #[test]
    fn generalized_leapfrog_matches_verlet_on_separable() {
        let sys = Separable::new(DiagonalKinetic::uniform(2, 1.0), HenonHeiles);
        let (mut q1, mut p1) = ([0.1, -0.2], [0.3, 0.1]);
        let (mut q2, mut p2) = (q1, p1);
        let gl = GeneralizedLeapfrog::default();
        for _ in 0..100 {
            gl.step(&sys, &mut q1, &mut p1, 0.01).unwrap();
        }
        Integrator::new(&sys, Scheme::StormerVerlet)
            .run(&mut q2, &mut p2, 0.01, 100)
            .unwrap();
        for i in 0..2 {
            assert!((q1[i] - q2[i]).abs() < 1e-12 && (p1[i] - p2[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn drift_avoids_overflow_before_momentum_rescales_the_inverse_mass() {
        let sys = Separable::new(
            DiagonalKinetic::from_masses(&[1e-308]).unwrap(),
            Harmonic::isotropic(1, 0.0),
        );
        for scheme in [Scheme::StormerVerlet, Scheme::Yoshida4] {
            let (mut q, mut p) = ([0.0], [1e-300]);
            Integrator::new(&sys, scheme)
                .step(&mut q, &mut p, 2.0)
                .unwrap();
            assert!(q[0].is_finite());
            assert!((q[0] / 2e8 - 1.0).abs() < 1e-14);
            assert_eq!(p, [1e-300]);
        }
    }

    #[test]
    fn integration_entrypoints_reject_nonfinite_input() {
        let sys = Separable::new(
            DiagonalKinetic::uniform(1, 1.0),
            Harmonic::isotropic(1, 1.0),
        );
        for mode in 0..4 {
            let mut integrator = Integrator::new(&sys, Scheme::StormerVerlet);
            let (mut q, mut p) = ([f64::NAN], [0.0]);
            let result = match mode {
                0 => integrator.step(&mut q, &mut p, 0.1),
                1 => integrator.run(&mut q, &mut p, 0.1, 0),
                2 => integrator.run_observed(&mut q, &mut p, 0.1, 0, 1, |_, _, _| {
                    panic!("não deve observar estado inválido");
                }),
                _ => integrator.run_with_tangent(
                    &mut q,
                    &mut p,
                    0.1,
                    0,
                    &mut TangentMap::identity(1),
                ),
            };
            assert_eq!(result, Err(IntegratorError::NonFiniteState));
            assert!(!integrator.force_valid);
        }
    }

    #[test]
    fn nonfinite_force_stops_each_entrypoint_before_observation() {
        struct InvalidForce {
            calls: std::sync::atomic::AtomicUsize,
        }
        impl Potential for InvalidForce {
            fn dim(&self) -> usize {
                1
            }
            fn value(&self, _q: &[f64]) -> f64 {
                0.0
            }
            fn gradient(&self, _q: &[f64], out: &mut [f64]) {
                self.calls
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                out.fill(f64::NAN);
            }
            fn hessian(&self, _q: &[f64], out: &mut [f64]) {
                out.fill(0.0);
            }
        }

        for mode in 0..4 {
            let sys = Separable::new(
                DiagonalKinetic::uniform(1, 1.0),
                InvalidForce {
                    calls: std::sync::atomic::AtomicUsize::new(0),
                },
            );
            let mut integrator = Integrator::new(&sys, Scheme::StormerVerlet);
            let (mut q, mut p) = ([0.0], [0.0]);
            let result = match mode {
                0 => integrator.step(&mut q, &mut p, 0.1),
                1 => integrator.run(&mut q, &mut p, 0.1, 100),
                2 => integrator.run_observed(&mut q, &mut p, 0.1, 100, 1, |_, _, _| {
                    panic!("não deve observar estado inválido");
                }),
                _ => integrator.run_with_tangent(
                    &mut q,
                    &mut p,
                    0.1,
                    100,
                    &mut TangentMap::identity(1),
                ),
            };
            assert_eq!(result, Err(IntegratorError::NonFiniteState));
            assert!(!integrator.force_valid);
            assert_eq!(
                sys.potential
                    .calls
                    .load(std::sync::atomic::Ordering::Relaxed),
                2
            );
        }
    }

    #[test]
    fn rejects_bad_input() {
        let sys = Separable::new(DiagonalKinetic::uniform(2, 1.0), HenonHeiles);
        let mut it = Integrator::new(&sys, Scheme::StormerVerlet);
        assert!(it.step(&mut [0.0], &mut [0.0, 0.0], 0.1).is_err());
        assert!(it.step(&mut [0.0, 0.0], &mut [0.0, 0.0], f64::NAN).is_err());
        assert!(DiagonalKinetic::from_masses(&[0.0]).is_err());
        assert!(DiagonalKinetic::from_masses(&[f64::INFINITY]).is_ok());
    }
}
