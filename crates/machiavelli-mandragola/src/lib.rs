//! Explicit decoy views for an untrusted observer, with an unmodified operator view.
use machiavelli_core::{ContinentiaNode, NodeId, PhaseVector};
use machiavelli_invariants::TacticalTarget;
pub struct Decoy {
    subject: NodeId,
    phase: PhaseVector,
}
impl Decoy {
    pub fn new(
        target: &TacticalTarget,
        phase: PhaseVector,
    ) -> Result<Self, machiavelli_core::CoreError> {
        phase.validate()?;
        Ok(Self {
            subject: target.id(),
            phase,
        })
    }
    pub fn untrusted_view(&self, target: &TacticalTarget) -> Result<&PhaseVector, &'static str> {
        if target.id() != self.subject {
            return Err("decoy subject mismatch");
        }
        Ok(&self.phase)
    }
}
/// Operator-facing path has no decoy parameter and always returns actual state.
pub fn operator_view(node: &ContinentiaNode) -> &ContinentiaNode {
    node
}
#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_invariants::Constitution;
    #[test]
    fn decoys_cannot_be_created_for_protected_channels() {
        let protected = NodeId::new();
        let guard = Constitution::new([protected]);
        assert!(guard.tactical_target(protected).is_err());
        let id = NodeId::new();
        let target = guard.tactical_target(id).unwrap();
        let decoy = Decoy::new(&target, PhaseVector::zeros(1)).unwrap();
        assert_eq!(decoy.untrusted_view(&target).unwrap().q(), &[0.0]);
        let other = guard.tactical_target(NodeId::new()).unwrap();
        assert!(decoy.untrusted_view(&other).is_err());
    }
}
