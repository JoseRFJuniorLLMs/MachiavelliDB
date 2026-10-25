//! Full-tree alternating CFR+ for finite two-player zero-sum perfect-recall games.
//! Information sets share one strategy. Best responses optimize over information
//! sets, never over hidden histories. Chance and terminals are explicit.
use machiavelli_core::Deadline;
#[path = "deep.rs"]
pub mod deep;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Tree {
    Terminal(f64),
    Chance(Vec<(f64, Tree)>),
    Decision {
        player: usize,
        info: String,
        actions: Vec<(String, Tree)>,
    },
}
#[derive(Clone, serde::Serialize)]
enum Node {
    Terminal(f64),
    Chance(Vec<(f64, usize)>),
    Decision { info: usize, children: Vec<usize> },
}
#[derive(Clone, serde::Serialize)]
struct Info {
    key: String,
    player: usize,
    actions: Vec<String>,
    history: Vec<(String, usize)>,
    nodes: Vec<usize>,
}
pub struct Game {
    nodes: Vec<Node>,
    infos: Vec<Info>,
    root: usize,
    digest: [u8; 32],
}
#[derive(Debug)]
pub struct GameError(pub &'static str);
impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for GameError {}
pub struct Profile {
    pub strategies: BTreeMap<String, Vec<f64>>,
    pub value: f64,
    pub exploitability: f64,
    pub iterations: usize,
    pub checkpoint: Checkpoint,
}
/// A game-bound CFR+ checkpoint. Regrets and linear averaging weights survive restart.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub game_digest: [u8; 32],
    pub iterations: u64,
    pub infos: BTreeMap<String, InfoCheckpoint>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoCheckpoint {
    pub player: u8,
    pub actions: Vec<String>,
    pub regrets: Vec<f64>,
    pub strategy_sum: Vec<f64>,
}
impl Checkpoint {
    pub fn infoset_id(&self, name: &str) -> machiavelli_core::InfosetHash {
        let mut key = b"MachiavelliDB/CFR+/checkpoint/v1".to_vec();
        key.extend_from_slice(&self.game_digest);
        key.extend_from_slice(name.as_bytes());
        machiavelli_core::InfosetHash::of(&key)
    }
}
impl Game {
    pub fn new(tree: Tree) -> Result<Self, GameError> {
        let mut game = Self {
            nodes: Vec::new(),
            infos: Vec::new(),
            root: 0,
            digest: [0; 32],
        };
        game.root = game.build(tree, &[Vec::new(), Vec::new()], 0)?;
        game.digest = *blake3::hash(
            &serde_json::to_vec(&(&game.nodes, &game.infos, game.root))
                .expect("validated finite game"),
        )
        .as_bytes();
        Ok(game)
    }
    fn build(
        &mut self,
        tree: Tree,
        history: &[Vec<(String, usize)>; 2],
        depth: usize,
    ) -> Result<usize, GameError> {
        if depth > 64 || self.nodes.len() >= 100_000 {
            return Err(GameError("tree exceeds resource limits"));
        }
        let id = self.nodes.len();
        self.nodes.push(Node::Terminal(0.0));
        let node = match tree {
            Tree::Terminal(v) => {
                if !v.is_finite() || v.abs() > 1e6 {
                    return Err(GameError("invalid payoff"));
                }
                Node::Terminal(v)
            }
            Tree::Chance(outcomes) => {
                if outcomes.is_empty()
                    || outcomes.iter().any(|(p, _)| !p.is_finite() || *p <= 0.0)
                    || (outcomes.iter().map(|(p, _)| p).sum::<f64>() - 1.0).abs() > 1e-12
                {
                    return Err(GameError("invalid chance distribution"));
                }
                let mut children = Vec::new();
                for (p, tree) in outcomes {
                    children.push((p, self.build(tree, history, depth + 1)?));
                }
                Node::Chance(children)
            }
            Tree::Decision {
                player,
                info,
                actions,
            } => {
                if player > 1
                    || info.is_empty()
                    || actions.is_empty()
                    || actions.len() > 256
                    || actions
                        .iter()
                        .map(|(name, _)| name)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != actions.len()
                {
                    return Err(GameError("invalid decision"));
                }
                let names: Vec<_> = actions.iter().map(|(name, _)| name.clone()).collect();
                let index = if let Some(index) = self.infos.iter().position(|v| v.key == info) {
                    let old = &self.infos[index];
                    if old.player != player
                        || old.actions != names
                        || old.history != history[player]
                    {
                        return Err(GameError(
                            "information set violates action consistency or perfect recall",
                        ));
                    }
                    index
                } else {
                    let index = self.infos.len();
                    self.infos.push(Info {
                        key: info.clone(),
                        player,
                        actions: names,
                        history: history[player].clone(),
                        nodes: Vec::new(),
                    });
                    index
                };
                self.infos[index].nodes.push(id);
                let mut children = Vec::new();
                for (action, (_, tree)) in actions.into_iter().enumerate() {
                    let mut next = history.clone();
                    next[player].push((info.clone(), action));
                    children.push(self.build(tree, &next, depth + 1)?);
                }
                Node::Decision {
                    info: index,
                    children,
                }
            }
        };
        self.nodes[id] = node;
        Ok(id)
    }
    fn uniform(&self) -> Vec<Vec<f64>> {
        self.infos
            .iter()
            .map(|i| vec![1.0 / i.actions.len() as f64; i.actions.len()])
            .collect()
    }
    fn visit(
        &self,
        id: usize,
        player: usize,
        reach: [f64; 2],
        chance: f64,
        profile: &[Vec<f64>],
        delta: &mut [Vec<f64>],
        deadline: &Deadline,
    ) -> Result<f64, GameError> {
        if deadline.expired() {
            return Err(GameError("deadline"));
        }
        Ok(match &self.nodes[id] {
            Node::Terminal(v) => {
                if player == 0 {
                    *v
                } else {
                    -*v
                }
            }
            Node::Chance(children) => {
                let mut value = 0.0;
                for &(p, child) in children {
                    value += p * self.visit(
                        child,
                        player,
                        reach,
                        chance * p,
                        profile,
                        delta,
                        deadline,
                    )?;
                }
                value
            }
            Node::Decision { info, children } => {
                let acting = self.infos[*info].player;
                let mut values = Vec::with_capacity(children.len());
                for (a, &child) in children.iter().enumerate() {
                    let mut next = reach;
                    next[acting] *= profile[*info][a];
                    values.push(self.visit(child, player, next, chance, profile, delta, deadline)?);
                }
                let value = values
                    .iter()
                    .zip(&profile[*info])
                    .map(|(v, p)| v * p)
                    .sum::<f64>();
                if acting == player {
                    let weight = chance * reach[1 - player];
                    for (a, v) in values.into_iter().enumerate() {
                        delta[*info][a] += weight * (v - value);
                    }
                }
                value
            }
        })
    }
    fn average(
        &self,
        id: usize,
        reach: [f64; 2],
        weight: f64,
        profile: &[Vec<f64>],
        sums: &mut [Vec<f64>],
    ) {
        match &self.nodes[id] {
            Node::Terminal(_) => {}
            Node::Chance(children) => {
                for &(_, child) in children {
                    self.average(child, reach, weight, profile, sums);
                }
            }
            Node::Decision { info, children } => {
                let player = self.infos[*info].player;
                for (a, &child) in children.iter().enumerate() {
                    sums[*info][a] += weight * reach[player] * profile[*info][a];
                    let mut next = reach;
                    next[player] *= profile[*info][a];
                    self.average(child, next, weight, profile, sums);
                }
            }
        }
    }
    fn value(
        &self,
        id: usize,
        profile: &[Vec<f64>],
        player: usize,
        choices: &BTreeMap<usize, usize>,
    ) -> f64 {
        match &self.nodes[id] {
            Node::Terminal(v) => {
                if player == 0 {
                    *v
                } else {
                    -*v
                }
            }
            Node::Chance(children) => children
                .iter()
                .map(|(p, c)| p * self.value(*c, profile, player, choices))
                .sum(),
            Node::Decision { info, children } => {
                if self.infos[*info].player == player
                    && let Some(&a) = choices.get(info)
                {
                    return self.value(children[a], profile, player, choices);
                }
                children
                    .iter()
                    .enumerate()
                    .map(|(a, c)| profile[*info][a] * self.value(*c, profile, player, choices))
                    .sum()
            }
        }
    }
    fn reach_other(
        &self,
        id: usize,
        player: usize,
        weight: f64,
        profile: &[Vec<f64>],
        out: &mut [f64],
    ) {
        out[id] = weight;
        match &self.nodes[id] {
            Node::Terminal(_) => {}
            Node::Chance(children) => {
                for &(p, c) in children {
                    self.reach_other(c, player, weight * p, profile, out);
                }
            }
            Node::Decision { info, children } => {
                for (a, &c) in children.iter().enumerate() {
                    let p = if self.infos[*info].player == player {
                        1.0
                    } else {
                        profile[*info][a]
                    };
                    self.reach_other(c, player, weight * p, profile, out);
                }
            }
        }
    }
    fn best_response(&self, player: usize, profile: &[Vec<f64>]) -> f64 {
        let mut reach = vec![0.0; self.nodes.len()];
        self.reach_other(self.root, player, 1.0, profile, &mut reach);
        let mut order: Vec<usize> = (0..self.infos.len())
            .filter(|&i| self.infos[i].player == player)
            .collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.infos[i].history.len()));
        let mut choices = BTreeMap::new();
        for i in order {
            let mut values = vec![0.0; self.infos[i].actions.len()];
            for &node in &self.infos[i].nodes {
                let Node::Decision { children, .. } = &self.nodes[node] else {
                    unreachable!()
                };
                for (a, &child) in children.iter().enumerate() {
                    values[a] += reach[node] * self.value(child, profile, player, &choices);
                }
            }
            let best = (0..values.len())
                .max_by(|&a, &b| values[a].total_cmp(&values[b]))
                .unwrap();
            choices.insert(i, best);
        }
        self.value(self.root, profile, player, &choices)
    }
    pub fn initial_checkpoint(&self) -> Checkpoint {
        let game_digest = self.digest;
        Checkpoint {
            game_digest,
            iterations: 0,
            infos: self
                .infos
                .iter()
                .map(|i| {
                    (
                        i.key.clone(),
                        InfoCheckpoint {
                            player: i.player as u8,
                            actions: i.actions.clone(),
                            regrets: vec![0.0; i.actions.len()],
                            strategy_sum: vec![0.0; i.actions.len()],
                        },
                    )
                })
                .collect(),
        }
    }
    pub fn solve(&self, iterations: usize, deadline: &Deadline) -> Result<Profile, GameError> {
        self.resume(&self.initial_checkpoint(), iterations, deadline)
    }
    pub fn resume(
        &self,
        checkpoint: &Checkpoint,
        iterations: usize,
        deadline: &Deadline,
    ) -> Result<Profile, GameError> {
        if iterations == 0
            || iterations > 1_000_000
            || checkpoint
                .iterations
                .checked_add(iterations as u64)
                .is_none_or(|n| n > 1_000_000_000_000)
        {
            return Err(GameError("invalid iteration limit"));
        }
        let initial = self.initial_checkpoint();
        if checkpoint.game_digest != initial.game_digest
            || checkpoint.infos.len() != self.infos.len()
        {
            return Err(GameError("checkpoint belongs to another game"));
        }
        let mut regrets = Vec::new();
        let mut sums = Vec::new();
        for info in &self.infos {
            let saved = checkpoint
                .infos
                .get(&info.key)
                .ok_or(GameError("missing checkpoint information set"))?;
            if saved.player as usize != info.player
                || saved.actions != info.actions
                || saved.regrets.len() != info.actions.len()
                || saved.strategy_sum.len() != info.actions.len()
                || saved
                    .regrets
                    .iter()
                    .chain(&saved.strategy_sum)
                    .any(|v| !v.is_finite() || *v < 0.0)
            {
                return Err(GameError("invalid checkpoint information set"));
            }
            if checkpoint.iterations == 0
                && saved
                    .regrets
                    .iter()
                    .chain(&saved.strategy_sum)
                    .any(|v| *v != 0.0)
            {
                return Err(GameError("nonempty zero-iteration checkpoint"));
            }
            regrets.push(saved.regrets.clone());
            sums.push(saved.strategy_sum.clone());
        }
        let mut strategy = self.uniform();
        for (r, s) in regrets.iter().zip(&mut strategy) {
            super::matching(r, s);
        }
        let mut completed = 0;
        'outer: for t in 1..=iterations {
            // One iteration is transactional: deadline discards both player's partial updates.
            let mut next_regrets = regrets.clone();
            let mut next_strategy = strategy.clone();
            for player in 0..2 {
                let mut delta: Vec<Vec<f64>> = regrets.iter().map(|v| vec![0.0; v.len()]).collect();
                if self
                    .visit(
                        self.root,
                        player,
                        [1.0, 1.0],
                        1.0,
                        &next_strategy,
                        &mut delta,
                        deadline,
                    )
                    .is_err()
                {
                    break 'outer;
                }
                for i in 0..self.infos.len() {
                    if self.infos[i].player == player {
                        for a in 0..delta[i].len() {
                            next_regrets[i][a] = (next_regrets[i][a] + delta[i][a]).max(0.0);
                        }
                        super::matching(&next_regrets[i], &mut next_strategy[i]);
                    }
                }
            }
            regrets = next_regrets;
            strategy = next_strategy;
            self.average(
                self.root,
                [1.0, 1.0],
                (checkpoint.iterations + t as u64) as f64,
                &strategy,
                &mut sums,
            );
            completed = t;
        }
        let mut average = self.uniform();
        for (sum, average) in sums.iter().zip(&mut average) {
            super::matching(sum, average);
        }
        let value = self.value(self.root, &average, 0, &BTreeMap::new());
        let exploitability =
            (self.best_response(0, &average) + self.best_response(1, &average)).max(0.0);
        let mut saved = initial;
        saved.iterations = checkpoint.iterations + completed as u64;
        for (i, info) in self.infos.iter().enumerate() {
            let state = saved.infos.get_mut(&info.key).unwrap();
            state.regrets = regrets[i].clone();
            state.strategy_sum = sums[i].clone();
        }
        Ok(Profile {
            checkpoint: saved,
            strategies: self
                .infos
                .iter()
                .enumerate()
                .map(|(i, v)| (v.key.clone(), average[i].clone()))
                .collect(),
            value,
            exploitability,
            iterations: completed,
        })
    }
}
/// Kuhn poker: six equiprobable ordered deals; fold/check and call/bet actions.
pub fn kuhn() -> Game {
    fn branch(cards: [usize; 2], history: &str) -> Tree {
        let winner = if cards[0] > cards[1] { 1.0 } else { -1.0 };
        match history {
            "pp" => Tree::Terminal(winner),
            "bp" => Tree::Terminal(1.0),
            "pbp" => Tree::Terminal(-1.0),
            "bb" | "pbb" => Tree::Terminal(2.0 * winner),
            _ => {
                let player = history.len() % 2;
                Tree::Decision {
                    player,
                    info: format!("{}:{}:{}", player, cards[player], history),
                    actions: ["p", "b"]
                        .iter()
                        .map(|a| (a.to_string(), branch(cards, &format!("{history}{a}"))))
                        .collect(),
                }
            }
        }
    }
    let outcomes = (0..3)
        .flat_map(|a| {
            (0..3)
                .filter(move |&b| a != b)
                .map(move |b| (1.0 / 6.0, branch([a, b], "")))
        })
        .collect();
    Game::new(Tree::Chance(outcomes)).unwrap()
}
/// Leduc with two copies of three ranks, one public card, two betting rounds,
/// fixed bets 2/4 and at most two bets/raises per round. Equivalent rank deals
/// are collapsed with their exact card-count probabilities.
pub fn leduc() -> Game {
    #[derive(Clone, serde::Serialize)]
    struct State {
        cards: [usize; 2],
        board: Option<usize>,
        round: usize,
        actor: usize,
        contributions: [i32; 2],
        raises: usize,
        checks: usize,
        history: String,
    }
    fn showdown(s: &State) -> Tree {
        let strength = |rank| {
            if s.board == Some(rank) {
                3 + rank
            } else {
                rank
            }
        };
        let a = strength(s.cards[0]);
        let b = strength(s.cards[1]);
        Tree::Terminal(if a > b {
            s.contributions[1] as f64
        } else if a < b {
            -(s.contributions[0] as f64)
        } else {
            0.0
        })
    }
    fn end_round(s: &State) -> Tree {
        if s.round == 1 {
            return showdown(s);
        }
        let mut outcomes = Vec::new();
        for board in 0..3 {
            let remaining = 2 - usize::from(s.cards[0] == board) - usize::from(s.cards[1] == board);
            if remaining == 0 {
                continue;
            }
            let mut next = s.clone();
            next.board = Some(board);
            next.round = 1;
            next.actor = 0;
            next.raises = 0;
            next.checks = 0;
            next.history.push('/');
            outcomes.push((remaining as f64 / 4.0, branch(next)));
        }
        Tree::Chance(outcomes)
    }
    fn branch(s: State) -> Tree {
        let own = s.actor;
        let other = 1 - own;
        let cost = s.contributions[other] - s.contributions[own];
        let mut actions = Vec::new();
        if cost == 0 {
            let mut check = s.clone();
            check.history.push('k');
            check.checks += 1;
            check.actor = other;
            let child = if check.checks == 2 {
                end_round(&check)
            } else {
                branch(check)
            };
            actions.push(("check".into(), child));
            let mut bet = s.clone();
            bet.history.push('b');
            bet.contributions[own] += if s.round == 0 { 2 } else { 4 };
            bet.raises = 1;
            bet.checks = 0;
            bet.actor = other;
            actions.push(("bet".into(), branch(bet)));
        } else {
            actions.push((
                "fold".into(),
                Tree::Terminal(if own == 0 {
                    -(s.contributions[0] as f64)
                } else {
                    s.contributions[1] as f64
                }),
            ));
            let mut call = s.clone();
            call.history.push('c');
            call.contributions[own] += cost;
            actions.push(("call".into(), end_round(&call)));
            if s.raises < 2 {
                let mut raise = s.clone();
                raise.history.push('r');
                raise.contributions[own] += cost + if s.round == 0 { 2 } else { 4 };
                raise.raises += 1;
                raise.actor = other;
                actions.push(("raise".into(), branch(raise)));
            }
        }
        Tree::Decision {
            player: own,
            info: format!("L:{}:{}:{:?}:{}", own, s.cards[own], s.board, s.history),
            actions,
        }
    }
    let mut deals = Vec::new();
    for a in 0..3 {
        for b in 0..3 {
            let probability = if a == b { 1.0 / 15.0 } else { 2.0 / 15.0 };
            deals.push((
                probability,
                branch(State {
                    cards: [a, b],
                    board: None,
                    round: 0,
                    actor: 0,
                    contributions: [1, 1],
                    raises: 0,
                    checks: 0,
                    history: String::new(),
                }),
            ));
        }
    }
    Game::new(Tree::Chance(deals)).unwrap()
}

pub fn attack_defense() -> Game {
    let response = |attack: usize| Tree::Decision {
        player: 0,
        info: "defender-observation".into(),
        actions: (0..2)
            .map(|defense| {
                (
                    format!("defend-{defense}"),
                    Tree::Terminal(if attack == defense { 1.0 } else { -1.0 }),
                )
            })
            .collect(),
    };
    Game::new(Tree::Decision {
        player: 1,
        info: "attacker-choice".into(),
        actions: (0..2)
            .map(|a| (format!("attack-{a}"), response(a)))
            .collect(),
    })
    .unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kuhn_converges_without_hidden_card_leak() {
        let result = kuhn().solve(20_000, &Deadline::none()).unwrap();
        assert!(
            result.exploitability < 1e-3,
            "gap {}, value {}",
            result.exploitability,
            result.value
        );
        assert!((result.value + 1.0 / 18.0).abs() < 1e-3);
        assert_eq!(result.strategies.len(), 12);
    }
    #[test]
    fn information_sets_cannot_forget_their_own_action() {
        let tree = Tree::Decision {
            player: 0,
            info: "root".into(),
            actions: vec![
                (
                    "a".into(),
                    Tree::Decision {
                        player: 0,
                        info: "same".into(),
                        actions: vec![("x".into(), Tree::Terminal(0.0))],
                    },
                ),
                (
                    "b".into(),
                    Tree::Decision {
                        player: 0,
                        info: "same".into(),
                        actions: vec![("x".into(), Tree::Terminal(0.0))],
                    },
                ),
            ],
        };
        assert!(Game::new(tree).is_err());
    }
    #[test]
    fn deadline_returns_valid_unexploited_information_set_policy() {
        let result = kuhn().solve(100, &Deadline::from_millis(0)).unwrap();
        assert_eq!(result.iterations, 0);
        assert!(result.exploitability > 0.1);
        assert!(result.strategies.values().all(|p| p == &vec![0.5, 0.5]));
    }
}

#[cfg(test)]
mod leduc_tests {
    use super::*;
    #[test]
    fn leduc_preserves_perfect_recall_and_returns_measured_gap() {
        let game = leduc();
        let result = game.solve(20, &Deadline::none()).unwrap();
        assert!(result.strategies.len() > 100);
        assert!(result.value.is_finite() && result.exploitability.is_finite());
        assert_eq!(result.iterations, 20);
        let improved = game.solve(500, &Deadline::none()).unwrap();
        assert!(
            improved.exploitability < result.exploitability,
            "{} -> {}",
            result.exploitability,
            improved.exploitability
        );
    }
}

#[cfg(test)]
mod defense_tests {
    use super::*;
    #[test]
    fn defender_does_not_observe_hidden_attack_route() {
        let result = attack_defense().solve(20000, &Deadline::none()).unwrap();
        assert!(result.exploitability < 1e-3);
        assert!(result.value.abs() < 1e-3);
        assert_eq!(result.strategies.len(), 2);
    }
}
#[cfg(test)]
mod checkpoint_tests {
    use super::*;
    #[test]
    fn resumed_cfr_matches_uninterrupted_training_exactly() {
        let game = kuhn();
        let first = game.solve(500, &Deadline::none()).unwrap();
        let bytes = serde_json::to_vec(&first.checkpoint).unwrap();
        let checkpoint: Checkpoint = serde_json::from_slice(&bytes).unwrap();
        let resumed = game.resume(&checkpoint, 500, &Deadline::none()).unwrap();
        let continuous = game.solve(1000, &Deadline::none()).unwrap();
        assert_eq!(resumed.checkpoint, continuous.checkpoint);
        assert_eq!(resumed.strategies, continuous.strategies);
        assert_eq!(resumed.exploitability, continuous.exploitability);
        assert_eq!(resumed.iterations, 500);
    }
    #[test]
    fn incompatible_or_invalid_checkpoints_are_rejected() {
        let game = kuhn();
        let original = game.solve(10, &Deadline::none()).unwrap().checkpoint;
        let mut changed = original.clone();
        changed.game_digest[0] ^= 1;
        assert!(game.resume(&changed, 1, &Deadline::none()).is_err());
        let mut changed = original.clone();
        changed.infos.values_mut().next().unwrap().regrets[0] = f64::NAN;
        assert!(game.resume(&changed, 1, &Deadline::none()).is_err());
        let mut changed = original.clone();
        changed.infos.values_mut().next().unwrap().actions.reverse();
        assert!(game.resume(&changed, 1, &Deadline::none()).is_err());
        let expired = game
            .resume(&original, 100, &Deadline::from_millis(0))
            .unwrap();
        assert_eq!(expired.checkpoint, original);
        assert_eq!(expired.iterations, 0);
    }
}
