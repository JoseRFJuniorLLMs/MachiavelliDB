//! Constitutional and protected-channel guards.
use machiavelli_core::{GuardViolation, NodeId, Transition, TransitionGuard};
use std::collections::BTreeSet;
#[derive(Debug, Default)]
pub struct Constitution {
    protected: BTreeSet<NodeId>,
}
impl Constitution {
    pub fn new(protected: impl IntoIterator<Item = NodeId>) -> Self {
        Self {
            protected: protected.into_iter().collect(),
        }
    }
}
impl TransitionGuard for Constitution {
    fn name(&self) -> &'static str {
        "constitution"
    }
    fn check(&self, t: &Transition<'_>) -> Result<(), GuardViolation> {
        let reject = |reason: &str| {
            Err(GuardViolation {
                guard: self.name().into(),
                node: t.node,
                reason: reason.into(),
            })
        };
        if self.protected.contains(&t.node) {
            return reject(
                "protected operator/audit channel cannot be mutated by tactical operations",
            );
        }
        if let Some(before) = t.before_phase {
            match t.after_phase {
                Some(after) if before.c() != after.c() => {
                    return reject("Casimir coordinates changed");
                }
                None if !before.c().is_empty() => return reject("constitutional node removed"),
                _ => {}
            }
        }
        Ok(())
    }
}
/// Tactical APIs can accept this type, never the protected administrative channel.
pub struct TacticalTarget(NodeId);
pub struct ProtectedChannel(NodeId);
impl TacticalTarget {
    pub fn id(&self) -> NodeId {
        self.0
    }
}
impl ProtectedChannel {
    pub fn new(id: NodeId) -> Self {
        Self(id)
    }
    pub fn id(&self) -> NodeId {
        self.0
    }
}
impl Constitution {
    pub fn tactical_target(&self, id: NodeId) -> Result<TacticalTarget, &'static str> {
        if self.protected.contains(&id) {
            Err("protected target")
        } else {
            Ok(TacticalTarget(id))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cannot_obtain_tactical_capability_for_root_or_audit() {
        let id = NodeId::derive("constitutional", "audit");
        let constitution = Constitution::new([id]);
        assert!(constitution.tactical_target(id).is_err());
        assert!(constitution.tactical_target(NodeId::new()).is_ok());
    }
}
