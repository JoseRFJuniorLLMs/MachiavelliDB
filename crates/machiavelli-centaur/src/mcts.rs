//! Seeded UCT for finite-action, fully observed, two-player zero-sum models.
//! Chance outcomes have explicit probabilities. Rewards always use Max's perspective.
//! Model transitions must be deterministic; stochastic events belong in Position::Chance.
use machiavelli_core::Deadline;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Player {
    Max,
    Min,
}
#[derive(Clone, Debug)]
pub enum Position<A> {
    Terminal(f64),
    Decision { player: Player, actions: Vec<A> },
    Chance(Vec<(f64, A)>),
}
pub trait Model {
    type State: Clone;
    type Action: Clone + PartialEq;
    fn position(&self, state: &Self::State) -> Result<Position<Self::Action>, String>;
    fn transition(&self, state: &Self::State, action: &Self::Action)
    -> Result<Self::State, String>;
    /// Bounded value in [-1,1] when the search reaches its depth limit.
    fn evaluate(&self, state: &Self::State) -> Result<f64, String>;
}
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub iterations: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub exploration: f64,
    pub seed: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            iterations: 2000,
            max_depth: 32,
            max_nodes: 10000,
            exploration: std::f64::consts::SQRT_2,
            seed: 0,
        }
    }
}
#[derive(Debug, Error)]
pub enum SearchError {
    #[error(
        "invalid MCTS budget: iterations 1..1000000, depth 1..128, nodes 1..1000000, finite exploration 0..64"
    )]
    InvalidConfig,
    #[error("invalid MCTS position: {0}")]
    InvalidPosition(&'static str),
    #[error("model: {0}")]
    Model(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RootTurn {
    Max,
    Min,
    Chance,
    Terminal,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActionStats<A> {
    pub action: A,
    pub visits: u64,
    pub mean_value: Option<f64>,
    pub probability: Option<f64>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report<A> {
    pub selected_action: Option<A>,
    pub root_turn: RootTurn,
    pub root_value: f64,
    pub iterations: usize,
    pub nodes: usize,
    pub depth_cutoffs: usize,
    pub deadline_reached: bool,
    pub node_limit_reached: bool,
    pub actions: Vec<ActionStats<A>>,
}
#[derive(Clone, Copy)]
enum Kind {
    Terminal(f64),
    Decision(Player),
    Chance,
}
struct Edge<A> {
    action: A,
    probability: f64,
    child: Option<usize>,
    visits: u64,
    mean: f64,
}
struct Node<S, A> {
    state: S,
    kind: Kind,
    edges: Vec<Edge<A>>,
    visits: u64,
    mean: f64,
}
fn bounded(value: f64) -> Result<f64, SearchError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        Err(SearchError::InvalidPosition("value outside [-1,1]"))
    } else {
        Ok(value)
    }
}
impl<S, A: Clone + PartialEq> Node<S, A> {
    fn new<M: Model<State = S, Action = A>>(model: &M, state: S) -> Result<Self, SearchError>
    where
        S: Clone,
    {
        let position = model.position(&state).map_err(SearchError::Model)?;
        let (kind, options) = match position {
            Position::Terminal(value) => (Kind::Terminal(bounded(value)?), Vec::new()),
            Position::Decision { player, actions } => (
                Kind::Decision(player),
                actions.into_iter().map(|a| (0.0, a)).collect(),
            ),
            Position::Chance(outcomes) => {
                if outcomes.iter().any(|(p, _)| !p.is_finite() || *p < 0.0)
                    || (outcomes.iter().map(|(p, _)| p).sum::<f64>() - 1.0).abs() > 1e-12
                {
                    return Err(SearchError::InvalidPosition("invalid chance probabilities"));
                }
                (Kind::Chance, outcomes)
            }
        };
        if !matches!(kind, Kind::Terminal(_)) && (options.is_empty() || options.len() > 256) {
            return Err(SearchError::InvalidPosition("action count must be 1..256"));
        }
        for (i, (_, action)) in options.iter().enumerate() {
            if options[..i].iter().any(|(_, a)| a == action) {
                return Err(SearchError::InvalidPosition("duplicate actions"));
            }
        }
        Ok(Self {
            state,
            kind,
            edges: options
                .into_iter()
                .map(|(probability, action)| Edge {
                    action,
                    probability,
                    child: None,
                    visits: 0,
                    mean: 0.0,
                })
                .collect(),
            visits: 0,
            mean: 0.0,
        })
    }
    fn random_edge(&self, rng: &mut ChaCha8Rng) -> usize {
        if matches!(self.kind, Kind::Chance) {
            let total: f64 = self.edges.iter().map(|e| e.probability).sum();
            let draw = rng.random::<f64>() * total;
            let mut cumulative = 0.0;
            let mut last = 0;
            for (i, e) in self.edges.iter().enumerate() {
                if e.probability > 0.0 {
                    last = i;
                    cumulative += e.probability;
                    if draw < cumulative {
                        return i;
                    }
                }
            }
            last
        } else {
            rng.random_range(0..self.edges.len())
        }
    }
    fn select(&self, rng: &mut ChaCha8Rng, exploration: f64) -> usize {
        let Kind::Decision(player) = self.kind else {
            return self.random_edge(rng);
        };
        if let Some(index) = self.edges.iter().position(|e| e.visits == 0) {
            return index;
        }
        let sign = if player == Player::Max { 1.0 } else { -1.0 };
        let log = (self.visits.max(1) as f64).ln();
        let score = |e: &Edge<A>| sign * e.mean + exploration * (log / e.visits as f64).sqrt();
        let mut best = 0;
        for i in 1..self.edges.len() {
            if score(&self.edges[i]) > score(&self.edges[best]) {
                best = i;
            }
        }
        best
    }
}
fn rollout<M: Model>(
    model: &M,
    start: &M::State,
    mut depth: usize,
    config: &Config,
    rng: &mut ChaCha8Rng,
    deadline: &Deadline,
) -> Result<Option<(f64, bool)>, SearchError> {
    let mut state = start.clone();
    loop {
        if deadline.expired() {
            return Ok(None);
        }
        let node = Node::new(model, state)?;
        if let Kind::Terminal(value) = node.kind {
            return Ok(Some((value, false)));
        }
        if depth >= config.max_depth {
            return Ok(Some((
                bounded(model.evaluate(&node.state).map_err(SearchError::Model)?)?,
                true,
            )));
        }
        let action = &node.edges[node.random_edge(rng)].action;
        state = model
            .transition(&node.state, action)
            .map_err(SearchError::Model)?;
        depth += 1;
    }
}
/// Search owns private state. Interrupted rollouts never contribute partial values.
/// The deadline is cooperative: implementations must bound the work of model callbacks.
pub fn search<M: Model>(
    model: &M,
    root: &M::State,
    config: Config,
    deadline: &Deadline,
) -> Result<Report<M::Action>, SearchError> {
    if !(1..=1_000_000).contains(&config.iterations)
        || !(1..=128).contains(&config.max_depth)
        || !(1..=1_000_000).contains(&config.max_nodes)
        || !config.exploration.is_finite()
        || !(0.0..=64.0).contains(&config.exploration)
    {
        return Err(SearchError::InvalidConfig);
    }
    let first = Node::new(model, root.clone())?;
    let fallback = if let Kind::Terminal(v) = first.kind {
        v
    } else {
        bounded(model.evaluate(&first.state).map_err(SearchError::Model)?)?
    };
    let mut tree = vec![first];
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
    let mut completed = 0;
    let mut cutoffs = 0;
    let mut node_limit = false;
    if !matches!(tree[0].kind, Kind::Terminal(_)) {
        'iterations: for _ in 0..config.iterations {
            let mut path = Vec::new();
            let mut visited = vec![0];
            let mut current = 0;
            let mut depth = 0;
            let (value, cutoff) = loop {
                if deadline.expired() {
                    break 'iterations;
                }
                if let Kind::Terminal(value) = tree[current].kind {
                    break (value, false);
                }
                if depth >= config.max_depth {
                    break (
                        bounded(
                            model
                                .evaluate(&tree[current].state)
                                .map_err(SearchError::Model)?,
                        )?,
                        true,
                    );
                }
                let edge = tree[current].select(&mut rng, config.exploration);
                path.push((current, edge));
                depth += 1;
                if let Some(child) = tree[current].edges[edge].child {
                    current = child;
                    visited.push(child);
                    continue;
                }
                let state = model
                    .transition(&tree[current].state, &tree[current].edges[edge].action)
                    .map_err(SearchError::Model)?;
                let result = if tree.len() < config.max_nodes {
                    let child = Node::new(model, state)?;
                    let id = tree.len();
                    tree.push(child);
                    tree[current].edges[edge].child = Some(id);
                    visited.push(id);
                    rollout(model, &tree[id].state, depth, &config, &mut rng, deadline)?
                } else {
                    node_limit = true;
                    rollout(model, &state, depth, &config, &mut rng, deadline)?
                };
                match result {
                    Some(value) => break value,
                    None => break 'iterations,
                }
            };
            if deadline.expired() {
                break;
            }
            for id in visited {
                let node = &mut tree[id];
                node.visits += 1;
                node.mean += (value - node.mean) / node.visits as f64;
            }
            for (id, index) in path {
                let edge = &mut tree[id].edges[index];
                edge.visits += 1;
                edge.mean += (value - edge.mean) / edge.visits as f64;
            }
            completed += 1;
            cutoffs += usize::from(cutoff);
        }
    }
    let root = &tree[0];
    let selected = if let Kind::Decision(player) = root.kind {
        let sign = if player == Player::Max { 1.0 } else { -1.0 };
        let mut best = 0;
        for i in 1..root.edges.len() {
            let a = &root.edges[i];
            let b = &root.edges[best];
            if a.visits > b.visits || (a.visits == b.visits && sign * a.mean > sign * b.mean) {
                best = i;
            }
        }
        Some(root.edges[best].action.clone())
    } else {
        None
    };
    Ok(Report {
        selected_action: selected,
        root_turn: match root.kind {
            Kind::Terminal(_) => RootTurn::Terminal,
            Kind::Chance => RootTurn::Chance,
            Kind::Decision(Player::Max) => RootTurn::Max,
            Kind::Decision(Player::Min) => RootTurn::Min,
        },
        root_value: if completed > 0 { root.mean } else { fallback },
        iterations: completed,
        nodes: tree.len(),
        depth_cutoffs: cutoffs,
        deadline_reached: deadline.expired(),
        node_limit_reached: node_limit,
        actions: root
            .edges
            .iter()
            .map(|e| ActionStats {
                action: e.action.clone(),
                visits: e.visits,
                mean_value: if e.visits > 0 { Some(e.mean) } else { None },
                probability: if matches!(root.kind, Kind::Chance) {
                    Some(e.probability)
                } else {
                    None
                },
            })
            .collect(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Game {
        positions: Vec<Position<usize>>,
    }
    impl Model for Game {
        type State = usize;
        type Action = usize;
        fn position(&self, state: &usize) -> Result<Position<usize>, String> {
            self.positions
                .get(*state)
                .cloned()
                .ok_or("unknown state".into())
        }
        fn transition(&self, _: &usize, action: &usize) -> Result<usize, String> {
            Ok(*action)
        }
        fn evaluate(&self, _: &usize) -> Result<f64, String> {
            Ok(0.0)
        }
    }
    fn decision(player: Player, actions: &[usize]) -> Position<usize> {
        Position::Decision {
            player,
            actions: actions.to_vec(),
        }
    }
    fn config(iterations: usize) -> Config {
        Config {
            iterations,
            seed: 42,
            ..Default::default()
        }
    }
    #[test]
    fn uct_avoids_a_delayed_adversarial_trap() {
        let game = Game {
            positions: vec![
                decision(Player::Max, &[1, 2]),
                Position::Terminal(0.25),
                decision(Player::Min, &[3, 4]),
                Position::Terminal(1.0),
                Position::Terminal(-1.0),
            ],
        };
        let report = search(&game, &0, config(20000), &Deadline::none()).unwrap();
        assert_eq!(report.selected_action, Some(1));
        assert!(report.actions[1].mean_value.unwrap() < 0.0);
        assert!(report.actions[0].visits > report.actions[1].visits);
        assert_eq!(report.iterations, 20000);
        assert_eq!(report.actions.iter().map(|a| a.visits).sum::<u64>(), 20000);
        assert_eq!(
            report,
            search(&game, &0, config(20000), &Deadline::none()).unwrap()
        );
    }
    #[test]
    fn minimizing_root_uses_max_player_values_with_the_correct_sign() {
        let game = Game {
            positions: vec![
                decision(Player::Min, &[1, 2]),
                Position::Terminal(0.25),
                Position::Terminal(-0.5),
            ],
        };
        let report = search(&game, &0, config(2000), &Deadline::none()).unwrap();
        assert_eq!(report.selected_action, Some(2));
        assert_eq!(report.root_turn, RootTurn::Min);
        assert!(report.root_value < 0.0);
    }
    #[test]
    fn chance_uses_probabilities_and_never_selects_zero_probability_events() {
        let game = Game {
            positions: vec![
                Position::Chance(vec![(0.25, 1), (0.75, 2), (0.0, 3)]),
                Position::Terminal(1.0),
                Position::Terminal(-1.0),
                Position::Terminal(0.0),
            ],
        };
        let report = search(&game, &0, config(20000), &Deadline::none()).unwrap();
        assert_eq!(report.selected_action, None);
        assert!((report.root_value + 0.5).abs() < 0.03);
        assert_eq!(report.actions[2].visits, 0);
        assert_eq!(report.root_turn, RootTurn::Chance);
    }
    #[test]
    fn exhausted_node_budget_keeps_search_bounded_and_uses_root_bandits() {
        let game = Game {
            positions: vec![
                decision(Player::Max, &[1, 2]),
                Position::Terminal(0.2),
                Position::Terminal(0.8),
            ],
        };
        let report = search(
            &game,
            &0,
            Config {
                max_nodes: 1,
                ..config(1000)
            },
            &Deadline::none(),
        )
        .unwrap();
        assert_eq!(report.nodes, 1);
        assert!(report.node_limit_reached);
        assert_eq!(report.selected_action, Some(2));
        assert_eq!(report.iterations, 1000);
    }
    #[test]
    fn finite_depth_stops_cycles_and_reports_heuristic_cutoffs() {
        let game = Game {
            positions: vec![decision(Player::Max, &[0])],
        };
        let report = search(
            &game,
            &0,
            Config {
                max_depth: 3,
                ..config(100)
            },
            &Deadline::none(),
        )
        .unwrap();
        assert_eq!(report.depth_cutoffs, 100);
        assert_eq!(report.root_value, 0.0);
        assert!(report.nodes <= 4);
    }
    #[test]
    fn expired_deadline_has_no_simulation_evidence_and_terminal_roots_need_no_search() {
        let game = Game {
            positions: vec![
                decision(Player::Max, &[1, 2]),
                Position::Terminal(0.25),
                Position::Terminal(0.75),
            ],
        };
        let report = search(&game, &0, config(1000), &Deadline::from_millis(0)).unwrap();
        assert_eq!(report.iterations, 0);
        assert!(report.deadline_reached);
        assert_eq!(report.selected_action, Some(1));
        assert!(
            report
                .actions
                .iter()
                .all(|a| a.visits == 0 && a.mean_value.is_none())
        );
        let terminal = search(&game, &2, config(1000), &Deadline::none()).unwrap();
        assert_eq!(terminal.root_turn, RootTurn::Terminal);
        assert_eq!(terminal.selected_action, None);
        assert_eq!(terminal.root_value, 0.75);
        assert_eq!(terminal.iterations, 0);
    }
    #[test]
    fn malformed_models_and_numerical_budgets_fail_explicitly() {
        let invalid = [
            Position::Terminal(f64::NAN),
            Position::Terminal(2.0),
            decision(Player::Max, &[]),
            decision(Player::Max, &[1, 1]),
            Position::Chance(vec![(0.2, 1)]),
            Position::Chance(vec![(f64::NAN, 1)]),
            Position::Chance(vec![(-1.0, 1), (2.0, 2)]),
        ];
        for position in invalid {
            let game = Game {
                positions: vec![position],
            };
            assert!(search(&game, &0, config(100), &Deadline::none()).is_err());
        }
        let game = Game {
            positions: vec![Position::Terminal(0.0)],
        };
        for bad in [
            Config {
                iterations: 0,
                ..config(1)
            },
            Config {
                max_nodes: 0,
                ..config(1)
            },
            Config {
                max_depth: 129,
                ..config(1)
            },
            Config {
                exploration: f64::NAN,
                ..config(1)
            },
        ] {
            assert!(matches!(
                search(&game, &0, bad, &Deadline::none()),
                Err(SearchError::InvalidConfig)
            ));
        }
    }
    #[test]
    fn model_errors_do_not_get_converted_into_favorable_rewards() {
        let game = Game {
            positions: vec![decision(Player::Max, &[42])],
        };
        assert!(matches!(
            search(&game, &0, config(100), &Deadline::none()),
            Err(SearchError::Model(_))
        ));
    }
}
