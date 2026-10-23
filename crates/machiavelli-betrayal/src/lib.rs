//! Pure isolation planner. Plans are not network or durable storage mutations.
//! External execution requires an unforgeable SPEC-0089 token, currently unavailable.
use machiavelli_core::{AuditKind, AuditRecord, ContinentiaNode, StrategicEdge, StrategicPosture};
use machiavelli_invariants::TacticalTarget;
pub struct IsolationPlan {
    pub isolated: ContinentiaNode,
    pub surviving_edges: Vec<StrategicEdge>,
    pub audit: AuditRecord,
}
pub fn plan(
    target: &TacticalTarget,
    node: &ContinentiaNode,
    edges: &[StrategicEdge],
    reason: &str,
    at_hlc: u64,
) -> Result<IsolationPlan, &'static str> {
    if target.id() != node.meta.id || reason.trim().is_empty() {
        return Err("invalid isolation target/reason");
    }
    if node.meta.posture == StrategicPosture::Fortified {
        return Err("citadel isolation requires verified quorum");
    }
    node.meta.validate().map_err(|_| "invalid metadata")?;
    node.phase.validate().map_err(|_| "invalid phase")?;
    let mut isolated = node.clone();
    isolated.meta.posture = StrategicPosture::Betrayed;
    isolated.phase.p_mut().fill(0.0);
    let surviving_edges = edges
        .iter()
        .filter(|edge| edge.from != target.id() && edge.to != target.id())
        .copied()
        .collect();
    let audit = AuditRecord {
        kind: AuditKind::Betrayal,
        subject: target.id(),
        summary: reason.into(),
        payload: serde_json::json!({"mode":"plan","previous_posture":node.meta.posture,"credits_pending_transfer":node.meta.vcg_credit_balance}),
        at_hlc,
    };
    Ok(IsolationPlan {
        isolated,
        surviving_edges,
        audit,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_core::{ContinentiaMeta, EdgeId, EdgeKind, NodeId, PhaseVector};
    use machiavelli_invariants::Constitution;
    #[test]
    fn isolation_plan_cuts_both_directions_and_preserves_source_and_casimir() {
        let id = NodeId::new();
        let other = NodeId::new();
        let target = Constitution::default().tactical_target(id).unwrap();
        let source = ContinentiaNode::new(
            ContinentiaMeta::new(id, 1),
            PhaseVector::with_constitution(vec![1.0], vec![2.0], vec![3.0]).unwrap(),
        );
        let edge = |from, to| StrategicEdge {
            id: EdgeId::new(),
            from,
            to,
            kind: EdgeKind::Ally,
            weight: 1.0,
        };
        let edges = vec![edge(id, other), edge(other, id), edge(other, other)];
        let plan = plan(&target, &source, &edges, "test compromised node", 2).unwrap();
        assert_eq!(plan.surviving_edges.len(), 1);
        assert_eq!(plan.isolated.phase.p(), &[0.0]);
        assert_eq!(plan.isolated.phase.c(), &[3.0]);
        assert_eq!(source.phase.p(), &[2.0]);
    }
}
