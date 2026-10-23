//! H(q,p) = T(p) + V_Fortuna(q) + Phi_adversarial(q), em snapshot fixo.

use crate::{HamiltonianError, StrategicMass};
use machiavelli_core::PhaseVector;
use machiavelli_symplectic::{Hamiltonian, PhaseFunction, Potential, Separable};

/// Duas componentes potenciais com derivadas somadas analiticamente.
pub struct StrategicPotential<F, A> {
    fortuna: F,
    adversarial: A,
}

impl<F: Potential, A: Potential> StrategicPotential<F, A> {
    pub fn new(fortuna: F, adversarial: A) -> Result<Self, HamiltonianError> {
        if fortuna.dim() == 0 {
            return Err(HamiltonianError::InvalidParameter {
                field: "dimension",
                value: 0.0,
            });
        }
        if fortuna.dim() != adversarial.dim() {
            return Err(HamiltonianError::DimensionMismatch {
                expected: fortuna.dim(),
                actual: adversarial.dim(),
            });
        }
        Ok(Self {
            fortuna,
            adversarial,
        })
    }

    pub fn fortuna(&self) -> &F {
        &self.fortuna
    }
    pub fn adversarial(&self) -> &A {
        &self.adversarial
    }
}

impl<F: Potential, A: Potential> Potential for StrategicPotential<F, A> {
    fn dim(&self) -> usize {
        self.fortuna.dim()
    }

    fn value(&self, q: &[f64]) -> f64 {
        self.fortuna.value(q) + self.adversarial.value(q)
    }

    fn gradient(&self, q: &[f64], out: &mut [f64]) {
        self.fortuna.gradient(q, out);
        self.adversarial.gradient_add(q, out);
    }

    fn hessian(&self, q: &[f64], out: &mut [f64]) {
        self.fortuna.hessian(q, out);
        self.adversarial.hessian_add(q, out);
    }

    fn gradient_add(&self, q: &[f64], out: &mut [f64]) {
        self.fortuna.gradient_add(q, out);
        self.adversarial.gradient_add(q, out);
    }

    fn hessian_add(&self, q: &[f64], out: &mut [f64]) {
        self.fortuna.hessian_add(q, out);
        self.adversarial.hessian_add(q, out);
    }
}

/// Decomposição verificável da energia no ponto de avaliação.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct EnergyBreakdown {
    pub virtu: f64,
    pub fortuna: f64,
    pub adversarial: f64,
    pub total: f64,
}

/// Hamiltoniano estratégico separável com validação na fronteira da API.
/// As implementações de traits mantêm o contrato de slices do kernel numérico.
pub struct StrategicHamiltonian<F, A> {
    mass: StrategicMass,
    system: Separable<StrategicPotential<F, A>>,
}

impl<F: Potential, A: Potential> StrategicHamiltonian<F, A> {
    pub fn new(mass: StrategicMass, fortuna: F, adversarial: A) -> Result<Self, HamiltonianError> {
        let potential = StrategicPotential::new(fortuna, adversarial)?;
        if mass.dim() != potential.dim() {
            return Err(HamiltonianError::DimensionMismatch {
                expected: mass.dim(),
                actual: potential.dim(),
            });
        }
        let system = Separable::new(mass.kinetic().clone(), potential);
        Ok(Self { mass, system })
    }

    pub fn dim(&self) -> usize {
        self.mass.dim()
    }
    pub fn mass(&self) -> &StrategicMass {
        &self.mass
    }
    pub fn potential(&self) -> &StrategicPotential<F, A> {
        &self.system.potential
    }

    /// Sistema autónomo diretamente consumível por Verlet/Yoshida.
    pub fn separable(&self) -> &Separable<StrategicPotential<F, A>> {
        &self.system
    }

    pub fn evaluate(&self, phase: &PhaseVector) -> Result<EnergyBreakdown, HamiltonianError> {
        phase.validate()?;
        if phase.dim() != self.dim() {
            return Err(HamiltonianError::DimensionMismatch {
                expected: self.dim(),
                actual: phase.dim(),
            });
        }
        let virtu = self.mass.energy(phase.p())?;
        let fortuna = self.potential().fortuna.value(phase.q());
        let adversarial = self.potential().adversarial.value(phase.q());
        let total = virtu + (fortuna + adversarial);
        for (field, value) in [
            ("fortuna_energy", fortuna),
            ("adversarial_energy", adversarial),
            ("total_energy", total),
        ] {
            if !value.is_finite() {
                return Err(HamiltonianError::NonFinite { field });
            }
        }
        Ok(EnergyBreakdown {
            virtu,
            fortuna,
            adversarial,
            total,
        })
    }
}

impl<F: Potential, A: Potential> Hamiltonian for StrategicHamiltonian<F, A> {
    fn dim(&self) -> usize {
        self.dim()
    }
    fn energy(&self, q: &[f64], p: &[f64]) -> f64 {
        self.system.energy(q, p)
    }
    fn grad_q(&self, q: &[f64], _p: &[f64], out: &mut [f64]) {
        self.potential().gradient(q, out);
    }
    fn grad_p(&self, _q: &[f64], p: &[f64], out: &mut [f64]) {
        self.system.kinetic.velocity(p, out);
    }
}

impl<F: Potential, A: Potential> PhaseFunction for StrategicHamiltonian<F, A> {
    fn dim(&self) -> usize {
        self.dim()
    }
    fn value(&self, q: &[f64], p: &[f64]) -> f64 {
        Hamiltonian::energy(self, q, p)
    }
    fn gradient(&self, q: &[f64], p: &[f64], gq: &mut [f64], gp: &mut [f64]) {
        self.potential().gradient(q, gq);
        self.system.kinetic.velocity(p, gp);
    }
}
