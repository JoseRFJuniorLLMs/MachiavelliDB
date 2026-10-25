//! Read-only reference planner: harmonic phase dynamics, bounded controls and explicit shocks.
//! Control choices and their costs are illustrative. Recommendations never execute a mutation.
use crate::mcts::{self, Model, Player, Position};
use machiavelli_core::{ContinentiaNode, Deadline, NodeId, PhaseVector};
use machiavelli_hamiltonian::{AdversarialPotential, StrategicHamiltonian, StrategicMass};
use machiavelli_storage::Snapshot;
use machiavelli_symplectic::{Integrator, Scheme, systems::Harmonic};
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub iterations: usize,
    pub horizon: usize,
    pub seed: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Hold,
    Damp,
    Retreat,
    NegativeShock,
    NoShock,
    PositiveShock,
}
#[derive(Clone)]
struct State {
    phase: PhaseVector,
    completed: usize,
    pending: Option<Control>,
    cost: f64,
}
struct DefenseModel {
    #[cfg(all(feature = "cuda", target_os = "linux"))]
    cuda_library: Option<std::path::PathBuf>,
    hamiltonian: StrategicHamiltonian<Harmonic, AdversarialPotential>,
    horizon: usize,
    shock: f64,
}
#[derive(Debug, Error)]
pub enum PlanningError {
    #[error("node not found")]
    NotFound,
    #[error(
        "planning requires 1..100000 iterations, horizon 1..32, phase dimension 1..128 and at most 128 Casimirs"
    )]
    InvalidBudget,
    #[error("invalid reference model: {0}")]
    Model(String),
    #[error(transparent)]
    Search(#[from] mcts::SearchError),
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Plan {
    pub node: NodeId,
    pub source_sequence: u64,
    pub model: &'static str,
    pub simulation_backend: &'static str,
    pub horizon: usize,
    pub seed: u64,
    pub persisted: bool,
    pub observation_id: Option<String>,
    pub games: Vec<EdgeGame>,
    pub search: mcts::Report<Control>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EdgeGame {
    pub edge: machiavelli_core::EdgeId,
    pub row_strategy: Vec<f64>,
    pub col_strategy: Vec<f64>,
    pub value: f64,
    pub exploitability: f64,
    pub iterations: usize,
}
struct BeliefModel {
    physical: DefenseModel,
    weights: Vec<f64>,
}
impl Model for BeliefModel {
    type State = Vec<State>;
    type Action = Control;
    fn position(&self, states: &Vec<State>) -> Result<Position<Control>, String> {
        match self.physical.position(&states[0])? {
            Position::Terminal(_) => Ok(Position::Terminal(self.evaluate(states)?)),
            position => Ok(position),
        }
    }
    fn transition(&self, states: &Vec<State>, action: &Control) -> Result<Vec<State>, String> {
        states
            .iter()
            .map(|s| self.physical.transition(s, action))
            .collect()
    }
    fn evaluate(&self, states: &Vec<State>) -> Result<f64, String> {
        let mut value = 0.0;
        for (state, weight) in states.iter().zip(&self.weights) {
            value += weight * self.physical.evaluate(state)?;
        }
        Ok(value.clamp(-1.0, 1.0))
    }
}
impl Model for DefenseModel {
    type State = State;
    type Action = Control;
    fn position(&self, state: &State) -> Result<Position<Control>, String> {
        if state.completed >= self.horizon {
            return Ok(Position::Terminal(self.evaluate(state)?));
        }
        if state.pending.is_some() {
            Ok(Position::Chance(vec![
                (0.2, Control::NegativeShock),
                (0.6, Control::NoShock),
                (0.2, Control::PositiveShock),
            ]))
        } else {
            Ok(Position::Decision {
                player: Player::Max,
                actions: vec![Control::Hold, Control::Damp, Control::Retreat],
            })
        }
    }
    fn transition(&self, state: &State, action: &Control) -> Result<State, String> {
        if state.completed >= self.horizon {
            return Err("terminal state has no transition".into());
        }
        let mut next = state.clone();
        match (state.pending, action) {
            (None, Control::Hold | Control::Damp | Control::Retreat) => {
                next.pending = Some(*action);
            }
            (Some(control), Control::NegativeShock | Control::NoShock | Control::PositiveShock) => {
                match control {
                    Control::Hold => {}
                    Control::Damp => {
                        next.phase.p_mut().iter_mut().for_each(|p| *p *= 0.5);
                        next.cost += 0.03;
                    }
                    Control::Retreat => {
                        let (q, p) = next.phase.qp_mut();
                        q.iter_mut().chain(p.iter_mut()).for_each(|v| *v *= 0.8);
                        next.cost += 0.08;
                    }
                    _ => return Err("invalid pending control".into()),
                }
                next.phase.p_mut()[0] += match action {
                    Control::NegativeShock => -self.shock,
                    Control::PositiveShock => self.shock,
                    _ => 0.0,
                };
                let (q, p) = next.phase.qp_mut();
                self.integrate(q, p)?;
                next.phase.validate().map_err(|e| e.to_string())?;
                next.pending = None;
                next.completed += 1;
            }
            _ => return Err("illegal control for this turn".into()),
        }
        Ok(next)
    }
    fn evaluate(&self, state: &State) -> Result<f64, String> {
        let energy = self
            .hamiltonian
            .evaluate(&state.phase)
            .map_err(|e| e.to_string())?
            .total;
        Ok(-(energy / (1.0 + energy) + state.cost).min(1.0))
    }
}
impl DefenseModel {
    fn integrate(&self, q: &mut [f64], p: &mut [f64]) -> Result<(), String> {
        #[cfg(all(feature = "cuda", target_os = "linux"))]
        if let Some(library) = &self.cuda_library {
            return machiavelli_gpu::cuda::verlet_batch(
                library,
                q,
                p,
                self.hamiltonian.mass().masses(),
                &vec![1.0; q.len()],
                0.01,
                8,
            );
        }
        Integrator::new(self.hamiltonian.separable(), Scheme::StormerVerlet)
            .run(q, p, 0.01, 8)
            .map_err(|e| e.to_string())
    }
}
fn reference_model(node: &ContinentiaNode, horizon: usize) -> Result<DefenseModel, PlanningError> {
    #[cfg(not(all(feature = "cuda", target_os = "linux")))]
    if std::env::var_os("MACHIAVELLI_CUDA_LIBRARY").is_some() {
        return Err(PlanningError::Model(
            "CUDA requested but this build lacks the Linux cuda feature".into(),
        ));
    }
    node.meta
        .validate()
        .map_err(|e| PlanningError::Model(e.to_string()))?;
    node.phase
        .validate()
        .map_err(|e| PlanningError::Model(e.to_string()))?;
    let n = node.phase.dim();
    let mass = StrategicMass::from_node(&node.meta, n, 0.0, Default::default())
        .map_err(|e| PlanningError::Model(e.to_string()))?;
    let potential = AdversarialPotential::new(vec![0.0; n], 0.0, 1.0)
        .map_err(|e| PlanningError::Model(e.to_string()))?;
    let hamiltonian = StrategicHamiltonian::new(mass, Harmonic::isotropic(n, 1.0), potential)
        .map_err(|e| PlanningError::Model(e.to_string()))?;
    Ok(DefenseModel {
        #[cfg(all(feature = "cuda", target_os = "linux"))]
        cuda_library: std::env::var_os("MACHIAVELLI_CUDA_LIBRARY").map(std::path::PathBuf::from),
        hamiltonian,
        horizon,
        shock: 0.1 + f64::from(node.meta.fortuna_exposure),
    })
}
pub fn plan(
    snapshot: &Snapshot,
    id: NodeId,
    budget: Budget,
    deadline: &Deadline,
) -> Result<Plan, PlanningError> {
    if !(1..=100000).contains(&budget.iterations) || !(1..=32).contains(&budget.horizon) {
        return Err(PlanningError::InvalidBudget);
    }
    let node = snapshot.get(id).ok_or(PlanningError::NotFound)?;
    if !(1..=128).contains(&node.phase.dim()) || node.phase.c().len() > 128 {
        return Err(PlanningError::InvalidBudget);
    }
    let model = reference_model(node, budget.horizon)?;
    let state = State {
        phase: node.phase.clone(),
        completed: 0,
        pending: None,
        cost: 0.0,
    };
    let mut config = mcts::Config {
        iterations: budget.iterations,
        max_depth: 2 * budget.horizon,
        max_nodes: (budget.iterations + 1).min(4096),
        seed: budget.seed,
        ..Default::default()
    };
    let mut observation_id = None;
    let belief_key =
        machiavelli_storage::RecordKey::new(machiavelli_storage::Family::Beliefs, id.as_bytes());
    let search = if let Some(machiavelli_storage::StrategicRecord::Belief {
        particles,
        weights,
        observation_id: observed,
        ..
    }) = snapshot.record(&belief_key)
    {
        // Every decision is shared by the whole posterior. Hidden particle
        // identity is never revealed to the policy as a decision branch.
        if particles.len() > 256 {
            return Err(PlanningError::Model(
                "belief planning supports at most 256 particles".into(),
            ));
        }
        let bytes_per_state =
            particles.len() * (96 + 8 * (2 * node.phase.dim() + node.phase.c().len()));
        config.max_nodes = config
            .max_nodes
            .min((32 * 1024 * 1024 / bytes_per_state).max(1));
        observation_id = Some(observed.clone());
        let states = particles
            .iter()
            .map(|phase| State {
                phase: phase.clone(),
                completed: 0,
                pending: None,
                cost: 0.0,
            })
            .collect();
        mcts::search(
            &BeliefModel {
                physical: model,
                weights: weights.clone(),
            },
            &states,
            config,
            deadline,
        )?
    } else {
        mcts::search(&model, &state, config, deadline)?
    };
    let mut games = Vec::new();
    for adjacency in snapshot.adjacency(id).take(16) {
        let key = machiavelli_storage::RecordKey::new(
            machiavelli_storage::Family::Payoffs,
            adjacency.edge.as_bytes(),
        );
        if let Some(machiavelli_storage::StrategicRecord::Payoff { matrix, .. }) =
            snapshot.record(&key)
        {
            if matrix.rows > 32 || matrix.cols > 32 {
                continue;
            }
            let game = machiavelli_nash::ZeroSumGame::new(matrix)
                .map_err(|e| PlanningError::Model(e.to_string()))?;
            let result = machiavelli_cfr::solve(&game, budget.iterations, deadline)
                .map_err(|e| PlanningError::Model(e.to_string()))?;
            games.push(EdgeGame {
                edge: adjacency.edge,
                row_strategy: result.row_strategy,
                col_strategy: result.col_strategy,
                value: result.certificate.value,
                exploitability: result.certificate.exploitability,
                iterations: result.iterations,
            });
        }
    }
    Ok(Plan {
        node: id,
        source_sequence: snapshot.sequence(),
        model: if observation_id.is_some() {
            "posterior_shared_control_harmonic"
        } else {
            "reference_harmonic_controls_and_shocks"
        },
        simulation_backend: if std::env::var_os("MACHIAVELLI_CUDA_LIBRARY").is_some() {
            "cuda_native"
        } else {
            "cpu"
        },
        horizon: budget.horizon,
        seed: budget.seed,
        persisted: false,
        observation_id,
        games,
        search,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_core::ContinentiaMeta;
    use machiavelli_storage::{Mutation, Store};
    #[test]
    fn recommendations_preserve_live_state_and_are_seed_reproducible() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let id = NodeId::new();
        let node = ContinentiaNode::new(
            ContinentiaMeta::new(id, 1),
            PhaseVector::with_constitution(vec![0.0], vec![4.0], vec![7.0]).unwrap(),
        );
        store.commit(&[Mutation::Put(node.clone())]).unwrap();
        let snapshot = store.snapshot();
        let budget = Budget {
            iterations: 3000,
            horizon: 1,
            seed: 42,
        };
        let a = plan(&snapshot, id, budget, &Deadline::none()).unwrap();
        let b = plan(&snapshot, id, budget, &Deadline::none()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.search.selected_action, Some(Control::Damp));
        assert_eq!(a.search.iterations, 3000);
        assert!(!a.persisted);
        assert_eq!(store.snapshot().get(id), Some(&node));
        assert_eq!(store.snapshot().sequence(), 1);
        let expired = plan(&snapshot, id, budget, &Deadline::from_millis(0)).unwrap();
        assert_eq!(expired.search.iterations, 0);
        assert!(expired.search.deadline_reached);
    }
    #[test]
    fn state_controls_never_change_casimir_and_illegal_turns_are_rejected() {
        let node = ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::new(), 1),
            PhaseVector::with_constitution(vec![0.3], vec![0.7], vec![7.0]).unwrap(),
        );
        let model = reference_model(&node, 1).unwrap();
        let source = State {
            phase: node.phase.clone(),
            completed: 0,
            pending: None,
            cost: 0.0,
        };
        assert!(model.transition(&source, &Control::NoShock).is_err());
        for control in [Control::Hold, Control::Damp, Control::Retreat] {
            let pending = model.transition(&source, &control).unwrap();
            assert!(model.transition(&pending, &control).is_err());
            for shock in [
                Control::NegativeShock,
                Control::NoShock,
                Control::PositiveShock,
            ] {
                let terminal = model.transition(&pending, &shock).unwrap();
                assert_eq!(terminal.phase.c(), &[7.0]);
                assert!(model.transition(&terminal, &control).is_err());
            }
        }
        assert_eq!(source.phase, node.phase);
    }
    #[test]
    fn quiet_state_prefers_holding_and_budget_validation_is_explicit() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let id = NodeId::new();
        store
            .commit(&[Mutation::Put(ContinentiaNode::new(
                ContinentiaMeta::new(id, 1),
                PhaseVector::zeros(1),
            ))])
            .unwrap();
        let budget = Budget {
            iterations: 3000,
            horizon: 1,
            seed: 9,
        };
        let result = plan(&store.snapshot(), id, budget, &Deadline::none()).unwrap();
        assert_eq!(result.search.selected_action, Some(Control::Hold));
        assert!(matches!(
            plan(
                &store.snapshot(),
                id,
                Budget {
                    horizon: 0,
                    ..budget
                },
                &Deadline::none()
            ),
            Err(PlanningError::InvalidBudget)
        ));
        assert!(matches!(
            plan(&store.snapshot(), NodeId::new(), budget, &Deadline::none()),
            Err(PlanningError::NotFound)
        ));
    }
}
