//! Immutable contingency snapshots. Matches return plans, not external execution.
//! A caller must authorize/audit execution through Trinitas before any side effect.
use arc_swap::ArcSwap;
use machiavelli_core::{ContinentiaNode, NodeId};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Isolate(NodeId),
    Fortify(NodeId),
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
        let plan = Self {
            subject,
            energy_above,
            expires_ms,
            topology_sequence,
            saved_virtu,
            actions,
        };
        plan.validate()?;
        Ok(plan)
    }
    pub fn subject(&self) -> NodeId {
        self.subject
    }
    pub fn references(&self, id: NodeId) -> bool {
        self.subject == id
            || self.actions.iter().any(|action| match action {
                Action::Isolate(n) | Action::Fortify(n) => *n == id,
            })
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.subject.is_nil()
            || !self.energy_above.is_finite()
            || !self.saved_virtu.is_finite()
            || self.saved_virtu < 0.0
            || self.actions.is_empty()
            || self.actions.len() > 1024
            || self.actions.iter().any(|action| match action {
                Action::Isolate(n) | Action::Fortify(n) => n.is_nil(),
            })
        {
            return Err("invalid dike");
        }
        Ok(())
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
