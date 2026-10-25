//! Deterministic local tactical transactions. Credits use exact integer units.
use crate::external::Error;
use machiavelli_core::{EdgeId, EdgeKind, NodeId, StrategicEdge, StrategicPosture, TriggerHash};
use machiavelli_dikes::{Action as DikeAction, CompiledDike};
use machiavelli_storage::{Family, Mutation, RecordKey, Snapshot, StrategicRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Observe {
        target: NodeId,
        observation_id: String,
        observed_at_ms: u64,
        position: Vec<f64>,
        standard_deviation: f64,
        initial_particles: Option<Vec<machiavelli_core::PhaseVector>>,
        seed: u64,
    },
    Ally {
        from: NodeId,
        to: NodeId,
    },
    Tribute {
        from: NodeId,
        to: NodeId,
        amount: u64,
    },
    Betray {
        target: NodeId,
        beneficiary: NodeId,
    },
    Fortify {
        target: NodeId,
    },
    Dike {
        target: NodeId,
        above: f64,
        ttl_ms: u64,
        actions: Vec<DikeAction>,
    },
    FireDike {
        id: TriggerHash,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedVote {
    pub signer: String,
    pub signature: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub operation_id: String,
    pub expected_sequence: u64,
    pub expires_ms: u64,
    pub action: Action,
    #[serde(default)]
    pub votes: Vec<SignedVote>,
}
impl Request {
    /// Four-eyes/BFT witnesses sign this digest with the configured epoch using
    /// machiavelli_fortify::signing_message, never a request-supplied membership.
    pub fn signing_digest(&self) -> [u8; 32] {
        *blake3::hash(
            &serde_json::to_vec(&(
                "machiavelli-action-v1",
                &self.operation_id,
                self.expected_sequence,
                self.expires_ms,
                &self.action,
            ))
            .expect("action serializes"),
        )
        .as_bytes()
    }
}
pub struct Policy {
    pub quorum: Option<machiavelli_fortify::Quorum>,
}
impl Policy {
    pub fn verify(&self, request: &Request, now_ms: u64) -> Result<(), Error> {
        let privileged = matches!(
            request.action,
            Action::Betray { .. } | Action::Fortify { .. } | Action::FireDike { .. }
        );
        if privileged {
            let quorum = self
                .quorum
                .as_ref()
                .ok_or("configure trusted quorum for this action")?;
            let votes = request
                .votes
                .iter()
                .map(|v| {
                    Ok(machiavelli_fortify::Vote {
                        signer: hex::decode(&v.signer)?
                            .try_into()
                            .map_err(|_| "invalid signer length")?,
                        signature: hex::decode(&v.signature)?
                            .try_into()
                            .map_err(|_| "invalid signature length")?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            quorum.verify(
                request.signing_digest(),
                request.expected_sequence,
                request.expires_ms,
                now_ms,
                &votes,
            )?;
        }
        Ok(())
    }
}
fn node(snapshot: &Snapshot, id: NodeId) -> Result<machiavelli_core::ContinentiaNode, Error> {
    let node = snapshot.get(id).ok_or("tactical node absent")?.clone();
    if matches!(
        node.meta.posture,
        StrategicPosture::Fortified | StrategicPosture::Betrayed
    ) {
        return Err(
            "fortified/betrayed nodes are frozen; explicit quorum migration required".into(),
        );
    }
    Ok(node)
}
fn balance(value: f64) -> Result<u64, Error> {
    if !value.is_finite() || value < 0.0 || value > (1u64 << 53) as f64 || value.fract() != 0.0 {
        return Err("tactical credits require exact integer units in 0..2^53".into());
    }
    Ok(value as u64)
}
fn transfer(
    snapshot: &Snapshot,
    from: NodeId,
    to: NodeId,
    amount: u64,
) -> Result<Vec<Mutation>, Error> {
    if from == to || amount == 0 || amount > 1u64 << 53 {
        return Err("invalid tribute".into());
    }
    let (mut debit, mut credit) = (node(snapshot, from)?, node(snapshot, to)?);
    let remaining = balance(debit.meta.vcg_credit_balance)?
        .checked_sub(amount)
        .ok_or("insufficient credit")?;
    let received = balance(credit.meta.vcg_credit_balance)?
        .checked_add(amount)
        .filter(|n| *n <= 1u64 << 53)
        .ok_or("credit overflow")?;
    debit.meta.vcg_credit_balance = remaining as f64;
    credit.meta.vcg_credit_balance = received as f64;
    Ok(vec![Mutation::Put(debit), Mutation::Put(credit)])
}
fn isolate(
    snapshot: &Snapshot,
    id: NodeId,
    operations: &mut Vec<Mutation>,
    removed: &mut BTreeSet<EdgeId>,
) -> Result<(), Error> {
    node(snapshot, id)?;
    for adj in snapshot.adjacency(id) {
        if !removed.insert(adj.edge) {
            continue;
        }
        let key = RecordKey::new(Family::Payoffs, adj.edge.as_bytes());
        if snapshot.record(&key).is_some() {
            operations.push(Mutation::DeleteStrategic(key));
        }
        operations.push(Mutation::DeleteStrategic(RecordKey::new(
            Family::Edges,
            adj.edge.as_bytes(),
        )));
    }
    Ok(())
}
pub fn plan(snapshot: &Snapshot, request: &Request, now_ms: u64) -> Result<Vec<Mutation>, Error> {
    let mut operations = Vec::new();
    match &request.action {
        Action::Observe {
            target,
            observation_id,
            observed_at_ms,
            position,
            standard_deviation,
            initial_particles,
            seed,
        } => {
            node(snapshot, *target)?;
            if *observed_at_ms > now_ms || observation_id.is_empty() || observation_id.len() > 128 {
                return Err("invalid observation timestamp/identity".into());
            }
            let key = RecordKey::new(Family::Beliefs, target.as_bytes());
            let (particles, weights) = match snapshot.record(&key) {
                Some(StrategicRecord::Belief {
                    particles,
                    weights,
                    observed_at_ms: before,
                    ..
                }) => {
                    if initial_particles.is_some() || observed_at_ms <= before {
                        return Err("observation must advance the persisted prior".into());
                    }
                    (particles.clone(), weights.clone())
                }
                _ => {
                    let particles = initial_particles
                        .clone()
                        .ok_or("first observation requires explicit prior particles")?;
                    let weights = vec![1.0 / particles.len().max(1) as f64; particles.len()];
                    (particles, weights)
                }
            };
            let mut filter = machiavelli_fog::ParticleFilter::new(particles, &weights, *seed)?;
            filter.observe_position(position, *standard_deviation)?;
            operations.push(Mutation::PutStrategic(StrategicRecord::Belief {
                node: *target,
                observation_id: observation_id.clone(),
                observed_at_ms: *observed_at_ms,
                particles: filter.particles().to_vec(),
                weights: filter.weights().to_vec(),
            }));
        }
        Action::Ally { from, to } => {
            node(snapshot, *from)?;
            node(snapshot, *to)?;
            if from == to {
                return Err("cannot ally with self".into());
            }
            operations.push(Mutation::PutStrategic(StrategicRecord::Edge {
                edge: StrategicEdge {
                    id: EdgeId::derive("trinitas-ally-v1", &request.operation_id),
                    from: *from,
                    to: *to,
                    kind: EdgeKind::Ally,
                    weight: 1.0,
                },
            }));
        }
        Action::Tribute { from, to, amount } => {
            operations = transfer(snapshot, *from, *to, *amount)?
        }
        Action::Betray {
            target,
            beneficiary,
        } => {
            let mut target_node = node(snapshot, *target)?;
            if target_node.meta.posture != StrategicPosture::Expendable
                || !target_node.phase.c().is_empty()
            {
                return Err("betray requires an expendable, non-constitutional node".into());
            }
            let credits = balance(target_node.meta.vcg_credit_balance)?;
            if credits > 0 {
                operations = transfer(snapshot, *target, *beneficiary, credits)?;
                operations.remove(0);
            } else {
                node(snapshot, *beneficiary)?;
                if target == beneficiary {
                    return Err("invalid beneficiary".into());
                }
            }
            isolate(snapshot, *target, &mut operations, &mut BTreeSet::new())?;
            target_node.meta.vcg_credit_balance = 0.0;
            target_node.meta.posture = StrategicPosture::Betrayed;
            target_node.phase.p_mut().fill(0.0);
            operations.push(Mutation::Put(target_node));
        }
        Action::Fortify { target } => {
            let mut n = node(snapshot, *target)?;
            n.meta.posture = StrategicPosture::Fortified;
            operations.push(Mutation::Put(n));
        }
        Action::Dike {
            target,
            above,
            ttl_ms,
            actions,
        } => {
            let n = node(snapshot, *target)?;
            if *ttl_ms == 0 || *ttl_ms > 86_400_000 {
                return Err("dike TTL requires 1..86400000ms".into());
            }
            let expires = now_ms.checked_add(*ttl_ms).ok_or("TTL overflow")?;
            let plan = CompiledDike::new(
                *target,
                *above,
                expires,
                request
                    .expected_sequence
                    .checked_add(3)
                    .ok_or("sequence overflow")?,
                n.meta.virtu.into(),
                actions.clone(),
            )?;
            // Pending, applied, acknowledged each consume one sequence.
            operations.push(Mutation::PutStrategic(StrategicRecord::Dike {
                id: TriggerHash::of(request.operation_id.as_bytes()),
                plan,
            }));
        }
        Action::FireDike { id } => {
            let key = RecordKey::new(Family::Dikes, id.as_bytes());
            let Some(StrategicRecord::Dike { plan, .. }) = snapshot.record(&key) else {
                return Err("dike absent".into());
            };
            let subject = snapshot.get(plan.subject()).ok_or("dike subject absent")?;
            if !plan.matches(subject, now_ms, request.expected_sequence) {
                return Err("dike expired, stale or predicate false".into());
            }
            let mut removed = BTreeSet::new();
            let mut fortified = BTreeSet::new();
            for action in plan.actions() {
                match action {
                    DikeAction::Isolate(id) => {
                        isolate(snapshot, *id, &mut operations, &mut removed)?
                    }
                    DikeAction::Fortify(id) => {
                        fortified.insert(*id);
                    }
                }
            }
            for id in fortified {
                let mut n = node(snapshot, id)?;
                n.meta.posture = StrategicPosture::Fortified;
                operations.push(Mutation::Put(n));
            }
            operations.push(Mutation::DeleteStrategic(key));
        }
    }
    if operations.len() > 255 {
        return Err("tactical action exceeds 255 mutations".into());
    }
    let mut branch = snapshot.branch();
    branch.apply(&operations)?;
    Ok(operations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use machiavelli_core::{ContinentiaMeta, ContinentiaNode, PhaseVector};
    use machiavelli_storage::Store;
    fn funded(id: NodeId, credits: f64) -> ContinentiaNode {
        let mut meta = ContinentiaMeta::new(id, 1);
        meta.vcg_credit_balance = credits;
        ContinentiaNode::new(meta, PhaseVector::zeros(1))
    }
    #[test]
    fn integer_transfers_conserve_credits_and_reject_overdraft_fractional_frozen_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let a = NodeId::new();
        let b = NodeId::new();
        store
            .commit(&[
                Mutation::Put(funded(a, 100.0)),
                Mutation::Put(funded(b, 3.0)),
            ])
            .unwrap();
        let mut request = Request {
            operation_id: "transfer".into(),
            expected_sequence: 1,
            expires_ms: 10_000,
            action: Action::Tribute {
                from: a,
                to: b,
                amount: 40,
            },
            votes: vec![],
        };
        store
            .commit(&plan(&store.snapshot(), &request, 1).unwrap())
            .unwrap();
        assert_eq!(
            store.snapshot().get(a).unwrap().meta.vcg_credit_balance,
            60.0
        );
        assert_eq!(
            store.snapshot().get(b).unwrap().meta.vcg_credit_balance,
            43.0
        );
        request.action = Action::Tribute {
            from: a,
            to: b,
            amount: 61,
        };
        assert!(plan(&store.snapshot(), &request, 1).is_err());
        let mut frozen = funded(b, 43.0);
        frozen.meta.posture = StrategicPosture::Fortified;
        store.commit(&[Mutation::Put(frozen)]).unwrap();
        request.action = Action::Tribute {
            from: a,
            to: b,
            amount: 1,
        };
        assert!(plan(&store.snapshot(), &request, 1).is_err());
    }
    fn request(operation_id: &str, expected_sequence: u64, action: Action) -> Request {
        Request {
            operation_id: operation_id.into(),
            expected_sequence,
            expires_ms: 10_000,
            action,
            votes: vec![],
        }
    }
    fn execute(store: &mut Store, request: &Request) {
        store
            .commit(&plan(&store.snapshot(), request, 1).unwrap())
            .unwrap();
    }
    #[test]
    fn betray_moves_all_credits_cuts_every_edge_and_freezes_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let (target, beneficiary, other) = (NodeId::new(), NodeId::new(), NodeId::new());
        let mut expendable = funded(target, 7.0);
        expendable.meta.posture = StrategicPosture::Expendable;
        expendable.phase.p_mut()[0] = 0.4;
        store
            .commit(&[
                Mutation::Put(expendable),
                Mutation::Put(funded(beneficiary, 3.0)),
                Mutation::Put(funded(other, 0.0)),
            ])
            .unwrap();
        execute(
            &mut store,
            &request(
                "ally-out",
                1,
                Action::Ally {
                    from: target,
                    to: other,
                },
            ),
        );
        execute(
            &mut store,
            &request(
                "ally-in",
                2,
                Action::Ally {
                    from: beneficiary,
                    to: target,
                },
            ),
        );
        assert_eq!(store.snapshot().adjacency(target).count(), 2);
        let betray = request(
            "betray",
            3,
            Action::Betray {
                target,
                beneficiary,
            },
        );
        execute(&mut store, &betray);
        let snapshot = store.snapshot();
        let frozen = snapshot.get(target).unwrap();
        assert_eq!(frozen.meta.posture, StrategicPosture::Betrayed);
        assert_eq!(frozen.meta.vcg_credit_balance, 0.0);
        assert!(frozen.phase.p().iter().all(|p| *p == 0.0));
        assert_eq!(
            snapshot.get(beneficiary).unwrap().meta.vcg_credit_balance,
            10.0
        );
        assert_eq!(snapshot.adjacency(target).count(), 0);
        assert_eq!(snapshot.adjacency(other).count(), 0);
        assert!(plan(&snapshot, &betray, 1).is_err());
        let tribute = Action::Tribute {
            from: beneficiary,
            to: target,
            amount: 1,
        };
        assert!(plan(&snapshot, &request("after", 4, tribute), 1).is_err());
        let loyal = Action::Betray {
            target: beneficiary,
            beneficiary: other,
        };
        assert!(plan(&snapshot, &request("loyal", 4, loyal), 1).is_err());
    }
    #[test]
    fn dike_fires_only_on_its_topology_isolates_and_fortifies_then_disappears() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let (subject, guard, neighbour) = (NodeId::new(), NodeId::new(), NodeId::new());
        let mut hot = funded(subject, 0.0);
        hot.meta.hamiltonian_energy = 10.0;
        store
            .commit(&[
                Mutation::Put(hot),
                Mutation::Put(funded(guard, 0.0)),
                Mutation::Put(funded(neighbour, 0.0)),
            ])
            .unwrap();
        execute(
            &mut store,
            &request(
                "edge",
                1,
                Action::Ally {
                    from: subject,
                    to: neighbour,
                },
            ),
        );
        let create = request(
            "dike",
            2,
            Action::Dike {
                target: subject,
                above: 5.0,
                ttl_ms: 60_000,
                actions: vec![DikeAction::Isolate(subject), DikeAction::Fortify(guard)],
            },
        );
        execute(&mut store, &create);
        let id = TriggerHash::of(b"dike");
        // The plan is bound to the sequence after pending/applied/ack commits.
        let fire = |sequence| request("fire", sequence, Action::FireDike { id });
        assert!(plan(&store.snapshot(), &fire(3), 2).is_err());
        store
            .commit(&[Mutation::Put(funded(NodeId::new(), 0.0))])
            .unwrap();
        store
            .commit(&[Mutation::Put(funded(NodeId::new(), 0.0))])
            .unwrap();
        assert!(plan(&store.snapshot(), &fire(5), 60_001).is_err());
        execute(&mut store, &fire(5));
        let snapshot = store.snapshot();
        assert_eq!(snapshot.adjacency(subject).count(), 0);
        assert_eq!(snapshot.adjacency(neighbour).count(), 0);
        assert_eq!(
            snapshot.get(guard).unwrap().meta.posture,
            StrategicPosture::Fortified
        );
        assert!(
            snapshot
                .record(&RecordKey::new(Family::Dikes, id.as_bytes()))
                .is_none()
        );
        assert!(plan(&snapshot, &fire(6), 2).is_err());
    }
    #[test]
    fn observations_need_an_explicit_prior_and_must_advance_in_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let target = NodeId::new();
        store.commit(&[Mutation::Put(funded(target, 0.0))]).unwrap();
        let prior: Vec<_> = [-0.5, 0.0, 0.5]
            .iter()
            .map(|q| machiavelli_core::PhaseVector::new(vec![*q], vec![0.0]).unwrap())
            .collect();
        let observe = |id: &str, at: u64, particles: Option<Vec<_>>| Action::Observe {
            target,
            observation_id: id.into(),
            observed_at_ms: at,
            position: vec![0.5],
            standard_deviation: 0.2,
            initial_particles: particles,
            seed: 3,
        };
        let without_prior = request("obs", 1, observe("o1", 5, None));
        assert!(plan(&store.snapshot(), &without_prior, 10).is_err());
        let future = request("obs", 1, observe("o1", 11, Some(prior.clone())));
        assert!(plan(&store.snapshot(), &future, 10).is_err());
        let first = request("obs", 1, observe("o1", 5, Some(prior.clone())));
        store
            .commit(&plan(&store.snapshot(), &first, 10).unwrap())
            .unwrap();
        let key = RecordKey::new(Family::Beliefs, target.as_bytes());
        let Some(StrategicRecord::Belief { weights, .. }) = store.snapshot().record(&key).cloned()
        else {
            panic!("belief not persisted");
        };
        assert!(weights[2] > weights[1] && weights[1] > weights[0]);
        let stale = request("obs", 2, observe("o2", 5, None));
        assert!(plan(&store.snapshot(), &stale, 10).is_err());
        let reprior = request("obs", 2, observe("o2", 6, Some(prior)));
        assert!(plan(&store.snapshot(), &reprior, 10).is_err());
        let next = request("obs", 2, observe("o2", 6, None));
        store
            .commit(&plan(&store.snapshot(), &next, 10).unwrap())
            .unwrap();
    }
    #[test]
    fn privileged_actions_bind_real_quorum_to_identity_sequence_action_and_expiry() {
        let keys: Vec<_> = (1..=4).map(|n| SigningKey::from_bytes(&[n; 32])).collect();
        let policy = Policy {
            quorum: Some(
                machiavelli_fortify::Quorum::new(
                    keys.iter().map(|k| k.verifying_key().to_bytes()).collect(),
                    1,
                    7,
                )
                .unwrap(),
            ),
        };
        let mut request = Request {
            operation_id: "fortify".into(),
            expected_sequence: 1,
            expires_ms: 1000,
            action: Action::Fortify {
                target: NodeId::new(),
            },
            votes: vec![],
        };
        assert!(policy.verify(&request, 1).is_err());
        let msg = machiavelli_fortify::signing_message(request.signing_digest(), 1, 7, 1000);
        request.votes = keys
            .iter()
            .take(3)
            .map(|k| SignedVote {
                signer: hex::encode(k.verifying_key().to_bytes()),
                signature: hex::encode(k.sign(&msg).to_bytes()),
            })
            .collect();
        policy.verify(&request, 999).unwrap();
        assert!(policy.verify(&request, 1000).is_err());
        request.expected_sequence = 2;
        assert!(policy.verify(&request, 999).is_err());
    }
}
