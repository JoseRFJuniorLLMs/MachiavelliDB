//! Immutable contingency snapshots. Matches return plans, not external execution.
//! A caller must authorize/audit execution through Trinitas before any side effect.
use arc_swap::ArcSwap;
use machiavelli_core::{ContinentiaNode, NodeId};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub enum Action {
    Isolate(NodeId),
    Fortify(NodeId),
}
#[derive(Clone, Debug)]
pub struct CompiledDike {
    subject: NodeId,
    energy_above: f64,
    expires_ms: u64,
    topology_sequence: u64,
    saved_virtu: f64,
    actions: Vec<Action>,
}
impl CompiledDike {
    pub fn new(
        subject: NodeId,
        energy_above: f64,
        expires_ms: u64,
        topology_sequence: u64,
        saved_virtu: f64,
        actions: Vec<Action>,
    ) -> Result<Self, &'static str> {
        if subject.is_nil()
            || !energy_above.is_finite()
            || !saved_virtu.is_finite()
            || saved_virtu < 0.0
            || actions.is_empty()
        {
            return Err("invalid dike");
        }
        Ok(Self {
            subject,
            energy_above,
            expires_ms,
            topology_sequence,
            saved_virtu,
            actions,
        })
    }
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }
    pub fn expected_saved_virtu(&self) -> f64 {
        self.saved_virtu
    }
    pub fn matches(&self, node: &ContinentiaNode, now_ms: u64, topology_sequence: u64) -> bool {
        self.subject == node.meta.id
            && now_ms < self.expires_ms
            && topology_sequence == self.topology_sequence
            && node.meta.hamiltonian_energy.is_finite()
            && node.meta.hamiltonian_energy > self.energy_above
    }
}
pub struct Dikes {
    plans: ArcSwap<Vec<CompiledDike>>,
}
impl Default for Dikes {
    fn default() -> Self {
        Self {
            plans: ArcSwap::from_pointee(Vec::new()),
        }
    }
}
impl Dikes {
    pub fn replace(&self, plans: Vec<CompiledDike>) {
        self.plans.store(Arc::new(plans));
    }
    pub fn snapshot(&self) -> Arc<Vec<CompiledDike>> {
        self.plans.load_full()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_core::{ContinentiaMeta, PhaseVector};
    #[test]
    fn stale_or_expired_plans_never_fire() {
        let id = NodeId::new();
        let mut node = ContinentiaNode::new(ContinentiaMeta::new(id, 0), PhaseVector::zeros(1));
        node.meta.hamiltonian_energy = 10.0;
        let dike = CompiledDike::new(id, 5.0, 1000, 7, 0.5, vec![Action::Isolate(id)]).unwrap();
        assert!(dike.matches(&node, 999, 7));
        assert!(!dike.matches(&node, 1000, 7));
        assert!(!dike.matches(&node, 999, 8));
        let index = Dikes::default();
        index.replace(vec![dike]);
        let old = index.snapshot();
        index.replace(Vec::new());
        assert_eq!(old.len(), 1);
        assert!(index.snapshot().is_empty());
    }
}
