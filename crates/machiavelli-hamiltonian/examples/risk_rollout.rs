//! Demonstração local: snapshot de risco -> energia -> trajetória contrafactual.
//! Execute: scripts/cargo.ps1 run -p machiavelli-hamiltonian --example risk_rollout

use machiavelli_core::PhaseVector;
use machiavelli_hamiltonian::{
    AdversarialPotential, FortunaPotential, RiskSnapshot, RiskWeights, StrategicHamiltonian,
    StrategicMass,
};
use machiavelli_symplectic::{Integrator, Scheme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = RiskSnapshot::new(1, vec![0.25, 0.80], vec![0.05, 0.30], vec![0.10, 0.60])?;
    let fortuna = FortunaPotential::new(snapshot, RiskWeights::new(1.0, 2.0, 1.0)?, 1.0)?;
    let adversarial = AdversarialPotential::new(vec![0.5, 0.5], 0.4, 0.8)?;
    let model =
        StrategicHamiltonian::new(StrategicMass::new(vec![100.0, 1.0])?, fortuna, adversarial)?;
    let initial = PhaseVector::with_constitution(vec![0.2, 0.2], vec![0.1, 0.1], vec![1.0])?;
    let mut rollout = initial.clone();
    let before = model.evaluate(&initial)?;
    let (q, p) = rollout.qp_mut();
    Integrator::new(model.separable(), Scheme::Yoshida4)
        .compensated()
        .run(q, p, 1e-3, 10_000)?;
    let after = model.evaluate(&rollout)?;
    println!("Snapshot fixo; 10.000 passos contrafactuais; nenhuma acao externa.");
    println!("Energia inicial: {before:?}");
    println!("Energia final:   {after:?}");
    println!(
        "Erro relativo: {:.3e}",
        ((after.total - before.total) / before.total).abs()
    );
    println!("Posicao inicial: {:?}", initial.q());
    println!("Posicao simulada: {:?}", rollout.q());
    println!("Constituicao preservada: {}", initial.c() == rollout.c());
    Ok(())
}
