use machiavelli_core::PhaseVector;
use machiavelli_hamiltonian::{
    AdversarialPotential, FortunaPotential, HamiltonianError, RiskSnapshot, RiskWeights,
    StrategicHamiltonian, StrategicMass,
};
use machiavelli_symplectic::systems::Harmonic;
use machiavelli_symplectic::{
    Hamiltonian, Integrator, PhaseFunction, Potential, Scheme, TangentMap,
};

#[test]
fn decomposition_and_derivatives_match_the_same_hamiltonian() {
    let snapshot = RiskSnapshot::new(42, vec![0.2, 0.7], vec![0.1, 0.4], vec![0.3, 0.2]).unwrap();
    let fortuna =
        FortunaPotential::new(snapshot, RiskWeights::new(1.0, 2.0, 0.5).unwrap(), 2.0).unwrap();
    let adversarial = AdversarialPotential::new(vec![0.5, -0.3], 1.5, 0.8).unwrap();
    let sys = StrategicHamiltonian::new(
        StrategicMass::new(vec![2.0, 4.0]).unwrap(),
        fortuna,
        adversarial,
    )
    .unwrap();
    let phase =
        PhaseVector::with_constitution(vec![0.2, -0.4], vec![2.0, -4.0], vec![1.0]).unwrap();
    let energy = sys.evaluate(&phase).unwrap();
    assert!((energy.virtu - 3.0).abs() < 1e-14);
    assert_eq!(
        energy.total,
        energy.virtu + (energy.fortuna + energy.adversarial)
    );
    assert!((energy.total - sys.separable().energy(phase.q(), phase.p())).abs() < 1e-14);

    let mut gq = [0.0; 2];
    let mut gp = [0.0; 2];
    PhaseFunction::gradient(&sys, phase.q(), phase.p(), &mut gq, &mut gp);
    for i in 0..2 {
        let mut plus = phase.clone();
        let mut minus = phase.clone();
        plus.q_mut()[i] += 1e-6;
        minus.q_mut()[i] -= 1e-6;
        let dq = (sys.evaluate(&plus).unwrap().total - sys.evaluate(&minus).unwrap().total) / 2e-6;
        assert!((gq[i] - dq).abs() < 1e-8);
        plus = phase.clone();
        minus = phase.clone();
        plus.p_mut()[i] += 1e-6;
        minus.p_mut()[i] -= 1e-6;
        let dp = (sys.evaluate(&plus).unwrap().total - sys.evaluate(&minus).unwrap().total) / 2e-6;
        assert!((gp[i] - dp).abs() < 1e-8);
    }
}

#[test]
fn tactical_model_preserves_energy_and_constitution_during_rollout() {
    let fortuna = Harmonic::isotropic(2, 0.7);
    let adversarial = AdversarialPotential::new(vec![0.2, -0.1], 0.3, 1.2).unwrap();
    let sys = StrategicHamiltonian::new(
        StrategicMass::new(vec![2.0, 3.0]).unwrap(),
        fortuna,
        adversarial,
    )
    .unwrap();
    let mut phase =
        PhaseVector::with_constitution(vec![0.3, -0.2], vec![0.2, 0.1], vec![7.0, 11.0]).unwrap();
    let original = phase.clone();
    let initial = sys.evaluate(&phase).unwrap().total;
    let mut integrator = Integrator::new(sys.separable(), Scheme::Yoshida4).compensated();
    let mut max_relative_error: f64 = 0.0;
    for _ in 0..100 {
        let (q, p) = phase.qp_mut();
        integrator.run(q, p, 1e-3, 1_000).unwrap();
        let energy = sys.evaluate(&phase).unwrap().total;
        max_relative_error = max_relative_error.max(((energy - initial) / initial).abs());
    }
    assert!(
        max_relative_error < 1e-9,
        "energy error {max_relative_error:e}"
    );
    assert_eq!(phase.c(), original.c());
    // A trajetória contrafactual usa uma cópia; o ponto de partida permanece intacto.
    assert_eq!(original.q(), &[0.3, -0.2]);
    assert_ne!(phase.q(), original.q());
}

#[test]
fn tactical_hessian_preserves_symplectic_tangent_map() {
    let sys = StrategicHamiltonian::new(
        StrategicMass::new(vec![1.0, 1.5]).unwrap(),
        Harmonic::isotropic(2, 0.4),
        AdversarialPotential::new(vec![0.2, -0.1], 0.1, 0.9).unwrap(),
    )
    .unwrap();
    let (mut q, mut p) = ([0.1, -0.2], [0.2, 0.1]);
    let mut tangent = TangentMap::identity(2);
    Integrator::new(sys.separable(), Scheme::Yoshida4)
        .run_with_tangent(&mut q, &mut p, 1e-3, 10_000, &mut tangent)
        .unwrap();
    assert!((tangent.liouville_volume() - 1.0).abs() < 1e-9);
    assert!(tangent.symplectic_defect() < 1e-9);
}

#[test]
fn model_rejects_mismatched_or_corrupted_phase_at_api_boundary() {
    let model = StrategicHamiltonian::new(
        StrategicMass::new(vec![1.0]).unwrap(),
        Harmonic::isotropic(1, 1.0),
        AdversarialPotential::new(vec![0.0], 0.0, 1.0).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        model.evaluate(&PhaseVector::zeros(2)),
        Err(HamiltonianError::DimensionMismatch { .. })
    ));
    let mut phase = PhaseVector::zeros(1);
    phase.q_mut()[0] = f64::INFINITY;
    assert!(model.evaluate(&phase).is_err());
    assert!(
        StrategicHamiltonian::new(
            StrategicMass::new(vec![1.0, 1.0]).unwrap(),
            Harmonic::isotropic(1, 1.0),
            AdversarialPotential::new(vec![0.0], 0.0, 1.0).unwrap(),
        )
        .is_err()
    );

    let overflow = StrategicHamiltonian::new(
        StrategicMass::new(vec![1.0]).unwrap(),
        Harmonic::isotropic(1, f64::MAX),
        AdversarialPotential::new(vec![0.0], 0.0, 1.0).unwrap(),
    )
    .unwrap();
    assert!(
        overflow
            .evaluate(&PhaseVector::new(vec![10.0], vec![0.0]).unwrap())
            .is_err()
    );
    assert!(Hamiltonian::energy(&model, &[0.0], &[f64::NAN]).is_nan());
    assert_eq!(model.potential().dim(), 1);
}
#[test]
fn all_energy_entrypoints_agree_at_large_momenta_and_with_cancellation() {
    struct Constant(f64);
    impl Potential for Constant {
        fn dim(&self) -> usize {
            1
        }
        fn value(&self, _q: &[f64]) -> f64 {
            self.0
        }
        fn gradient(&self, _q: &[f64], out: &mut [f64]) {
            out.fill(0.0);
        }
        fn hessian(&self, _q: &[f64], out: &mut [f64]) {
            out.fill(0.0);
        }
    }
    for (mass, momentum, fortuna, adversarial) in
        [(1e300, 1e200, 0.0, 0.0), (1.0, 1.5e154, 1e308, -1e308)]
    {
        let sys = StrategicHamiltonian::new(
            StrategicMass::new(vec![mass]).unwrap(),
            Constant(fortuna),
            Constant(adversarial),
        )
        .unwrap();
        let phase = PhaseVector::new(vec![0.0], vec![momentum]).unwrap();
        let checked = sys.evaluate(&phase).unwrap().total;
        assert!(checked.is_finite());
        assert_eq!(checked, Hamiltonian::energy(&sys, phase.q(), phase.p()));
        assert_eq!(checked, sys.separable().energy(phase.q(), phase.p()));
    }
}

#[test]
fn overflowing_builtin_potential_is_an_error_in_checked_evaluation() {
    let risk = FortunaPotential::new(
        RiskSnapshot::new(0, vec![1.0], vec![1.0], vec![1.0]).unwrap(),
        RiskWeights::default(),
        f64::MAX,
    )
    .unwrap();
    let sys = StrategicHamiltonian::new(
        StrategicMass::new(vec![1.0]).unwrap(),
        risk,
        AdversarialPotential::new(vec![0.0], 0.0, 1.0).unwrap(),
    )
    .unwrap();
    let phase = PhaseVector::new(vec![f64::MAX], vec![0.0]).unwrap();
    assert!(matches!(
        sys.evaluate(&phase),
        Err(HamiltonianError::NonFinite { .. })
    ));
}
