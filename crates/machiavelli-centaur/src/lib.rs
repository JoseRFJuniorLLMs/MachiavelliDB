//! Bounded counterfactual risk scan. Produces candidate plans without execution.
//! Uses private mmap pages and a reference harmonic rollout; MCTS is not claimed.
use machiavelli_core::{Deadline, NodeId};
use machiavelli_cow_arena::Arena;
use machiavelli_dikes::{Action, CompiledDike};
use machiavelli_fortuna::Fortuna;
use machiavelli_hamiltonian::{AdversarialPotential, StrategicHamiltonian, StrategicMass};
use machiavelli_storage::Snapshot;
use machiavelli_symplectic::{Integrator, Scheme, systems::Harmonic};
use std::sync::Arc;
pub struct Risk {
    pub node: NodeId,
    pub peak_energy: f64,
    pub failed_rollouts: usize,
}
pub struct TickReport {
    pub sequence: u64,
    pub risks: Vec<Risk>,
    pub candidate_dikes: Vec<CompiledDike>,
    pub interrupted: bool,
}
pub fn tick(
    snapshot: &Snapshot,
    seed: u64,
    now_ms: u64,
    deadline: &Deadline,
) -> Result<TickReport, Box<dyn std::error::Error>> {
    let mut random = Fortuna::new(seed, 1.5, 0.01, 10.0)?;
    let mut report = TickReport {
        sequence: snapshot.sequence(),
        risks: Vec::new(),
        candidate_dikes: Vec::new(),
        interrupted: false,
    };
    for node in snapshot.nodes().take(1000) {
        if deadline.expired() {
            report.interrupted = true;
            break;
        }
        let n = node.phase.dim();
        let source: Vec<u8> = node
            .phase
            .to_z()
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let arena = Arc::new(Arena::new(&source)?);
        let Ok(mass) = StrategicMass::from_node(&node.meta, n, 0.0, Default::default()) else {
            continue;
        };
        let model = StrategicHamiltonian::new(
            mass,
            Harmonic::isotropic(n, 1.0),
            AdversarialPotential::new(vec![0.0; n], 0.0, 1.0)?,
        )?;
        let initial = model.evaluate(&node.phase)?.total;
        let mut peak = initial;
        let mut failed = 0;
        for _ in 0..8 {
            if deadline.expired() {
                report.interrupted = true;
                break;
            }
            let mut branch = arena.fork()?;
            let mut phase = node.phase.clone();
            let shock = phase.q()[0] + random.signed();
            if !shock.is_finite() {
                failed += 1;
                continue;
            }
            branch.write(0, &shock.to_le_bytes())?;
            phase.q_mut()[0] = f64::from_le_bytes(branch.bytes()[..8].try_into().unwrap());
            let (q, p) = phase.qp_mut();
            if Integrator::new(model.separable(), Scheme::Yoshida4)
                .run(q, p, 0.001, 100)
                .is_err()
            {
                failed += 1;
                continue;
            }
            match model.evaluate(&phase) {
                Ok(e) => peak = peak.max(e.total),
                Err(_) => failed += 1,
            }
        }
        if peak > initial + 1.0 {
            let ttl = now_ms.checked_add(1000).ok_or("TTL overflow")?;
            report.candidate_dikes.push(CompiledDike::new(
                node.meta.id,
                initial + 1.0,
                ttl,
                snapshot.sequence(),
                (peak - initial).min(1.0),
                vec![Action::Isolate(node.meta.id)],
            )?);
        }
        report.risks.push(Risk {
            node: node.meta.id,
            peak_energy: peak,
            failed_rollouts: failed,
        });
    }
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_core::{ContinentiaMeta, ContinentiaNode, PhaseVector};
    use machiavelli_storage::{Mutation, Store};
    #[test]
    fn risk_scan_never_changes_source_or_constitution() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let node = ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::new(), 0),
            PhaseVector::with_constitution(vec![0.1], vec![0.2], vec![7.0]).unwrap(),
        );
        store.commit(&[Mutation::Put(node.clone())]).unwrap();
        let report = tick(&store.snapshot(), 5, 0, &Deadline::none()).unwrap();
        assert_eq!(report.risks.len(), 1);
        assert_eq!(store.snapshot().get(node.meta.id), Some(&node));
        let deadline = tick(&store.snapshot(), 5, 0, &Deadline::from_millis(0)).unwrap();
        assert!(deadline.interrupted);
        assert!(deadline.risks.is_empty());
    }
}
