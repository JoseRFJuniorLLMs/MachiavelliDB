//! Massa estratégica diagonal, constante dentro de cada simulação.

use crate::HamiltonianError;
use machiavelli_core::{ContinentiaMeta, StrategicPosture};
use machiavelli_symplectic::DiagonalKinetic;

/// Política de inércia: m = base × (1 + gain × centrality) × postura.
/// Cidadelas têm multiplicador >= 100; scouts têm multiplicador em (0, 1].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassPolicy {
    citadel_multiplier: f64,
    scout_multiplier: f64,
    centrality_gain: f64,
}

impl Default for MassPolicy {
    fn default() -> Self {
        Self {
            citadel_multiplier: 100.0,
            scout_multiplier: 0.25,
            centrality_gain: 4.0,
        }
    }
}

impl MassPolicy {
    pub fn new(
        citadel_multiplier: f64,
        scout_multiplier: f64,
        centrality_gain: f64,
    ) -> Result<Self, HamiltonianError> {
        for (field, value, valid) in [
            (
                "citadel_multiplier",
                citadel_multiplier,
                citadel_multiplier.is_finite() && citadel_multiplier >= 100.0,
            ),
            (
                "scout_multiplier",
                scout_multiplier,
                scout_multiplier.is_finite() && scout_multiplier > 0.0 && scout_multiplier <= 1.0,
            ),
            (
                "centrality_gain",
                centrality_gain,
                centrality_gain.is_finite() && centrality_gain >= 0.0,
            ),
        ] {
            if !valid {
                return Err(HamiltonianError::InvalidParameter { field, value });
            }
        }
        Ok(Self {
            citadel_multiplier,
            scout_multiplier,
            centrality_gain,
        })
    }

    pub fn citadel_multiplier(self) -> f64 {
        self.citadel_multiplier
    }
    pub fn scout_multiplier(self) -> f64 {
        self.scout_multiplier
    }
    pub fn centrality_gain(self) -> f64 {
        self.centrality_gain
    }
}

/// Tensor M = diag(m_i), validado, com inversas finitas e estritamente positivas.
/// Não aceita massa infinita: congelar apenas q não isola um nó traído.
#[derive(Clone, Debug, PartialEq)]
pub struct StrategicMass {
    masses: Vec<f64>,
    kinetic: DiagonalKinetic,
}

impl StrategicMass {
    pub fn new(masses: Vec<f64>) -> Result<Self, HamiltonianError> {
        if masses.is_empty() {
            return Err(HamiltonianError::InvalidParameter {
                field: "dimension",
                value: 0.0,
            });
        }
        for &mass in &masses {
            if !mass.is_finite() || mass <= 0.0 || !(1.0 / mass).is_finite() {
                return Err(HamiltonianError::InvalidParameter {
                    field: "mass",
                    value: mass,
                });
            }
        }
        let kinetic = DiagonalKinetic::from_masses(&masses)?;
        Ok(Self { masses, kinetic })
    }

    /// Constrói massa uniforme para as coordenadas de um nó. A centralidade é
    /// normalizada em [0, 1] e calculada pelo chamador no snapshot do grafo.
    pub fn from_node(
        node: &ContinentiaMeta,
        dim: usize,
        centrality: f64,
        policy: MassPolicy,
    ) -> Result<Self, HamiltonianError> {
        node.validate()?;
        if !centrality.is_finite() || !(0.0..=1.0).contains(&centrality) {
            return Err(HamiltonianError::InvalidParameter {
                field: "centrality",
                value: centrality,
            });
        }
        let posture_factor = match node.posture {
            StrategicPosture::Fortified => policy.citadel_multiplier,
            StrategicPosture::Expendable => policy.scout_multiplier,
            StrategicPosture::Betrayed => return Err(HamiltonianError::InactiveNode),
            StrategicPosture::Expanding | StrategicPosture::Decoy => 1.0,
        };
        let mass = f64::from(node.strategic_mass)
            * (1.0 + policy.centrality_gain * centrality)
            * posture_factor;
        Self::new(vec![mass; dim])
    }

    pub fn dim(&self) -> usize {
        self.masses.len()
    }
    pub fn masses(&self) -> &[f64] {
        &self.masses
    }
    pub fn kinetic(&self) -> &DiagonalKinetic {
        &self.kinetic
    }

    pub fn energy(&self, momentum: &[f64]) -> Result<f64, HamiltonianError> {
        if momentum.len() != self.dim() {
            return Err(HamiltonianError::DimensionMismatch {
                expected: self.dim(),
                actual: momentum.len(),
            });
        }
        if momentum.iter().any(|p| !p.is_finite()) {
            return Err(HamiltonianError::NonFinite { field: "momentum" });
        }
        let energy = self.kinetic.energy(momentum);
        if !energy.is_finite() {
            return Err(HamiltonianError::NonFinite {
                field: "kinetic_energy",
            });
        }
        Ok(energy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_core::NodeId;

    #[test]
    fn posture_and_centrality_control_inertia() {
        let mut node = ContinentiaMeta::new(NodeId::new(), 0);
        let policy = MassPolicy::default();
        let normal = StrategicMass::from_node(&node, 2, 0.0, policy).unwrap();
        node.posture = StrategicPosture::Fortified;
        let citadel = StrategicMass::from_node(&node, 2, 0.0, policy).unwrap();
        assert_eq!(citadel.masses()[0], 100.0 * normal.masses()[0]);
        node.posture = StrategicPosture::Expendable;
        let scout = StrategicMass::from_node(&node, 2, 0.0, policy).unwrap();
        assert!(scout.masses()[0] < normal.masses()[0]);
        assert!(
            StrategicMass::from_node(&node, 2, 1.0, policy)
                .unwrap()
                .masses()[0]
                > scout.masses()[0]
        );
        node.posture = StrategicPosture::Betrayed;
        assert_eq!(
            StrategicMass::from_node(&node, 2, 0.0, policy),
            Err(HamiltonianError::InactiveNode)
        );
    }

    #[test]
    fn rejects_invalid_mass_and_policy() {
        for bad in [0.0, -1.0, f64::INFINITY, f64::NAN, f64::from_bits(1)] {
            assert!(StrategicMass::new(vec![bad]).is_err());
        }
        assert!(StrategicMass::new(vec![]).is_err());
        assert!(MassPolicy::new(99.0, 0.5, 1.0).is_err());
        assert!(MassPolicy::new(100.0, 0.0, 1.0).is_err());
        assert!(MassPolicy::new(100.0, 1.1, 1.0).is_err());
        assert!(MassPolicy::new(100.0, 0.5, f64::INFINITY).is_err());
    }

    #[test]
    fn kinetic_energy_checks_dimension_and_extreme_inputs() {
        let mass = StrategicMass::new(vec![2.0, 4.0]).unwrap();
        assert!((mass.energy(&[2.0, 4.0]).unwrap() - 3.0).abs() < 1e-14);
        assert!(mass.energy(&[1.0]).is_err());
        assert!(mass.energy(&[f64::NAN, 0.0]).is_err());
        assert!(mass.energy(&[f64::MAX, 0.0]).is_err());
        let huge = StrategicMass::new(vec![1e300]).unwrap();
        assert!((huge.energy(&[1e200]).unwrap() / 5e99 - 1.0).abs() < 1e-14);
    }
}
