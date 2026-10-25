//! CPU Deep CFR reference: external sampling, bounded reservoir memories,
//! two advantage MLPs and a shared average-strategy MLP. Explicit finite-game
//! encodings; no GPU backend or scalability claim. Checkpoints bind the game.
use super::{Game, GameError, Node};
use machiavelli_core::Deadline;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub traversals: usize,
    pub train_steps: usize,
    pub hidden: usize,
    pub memory: usize,
    pub learning_rate: f64,
    pub seed: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            traversals: 32,
            train_steps: 256,
            hidden: 32,
            memory: 4096,
            learning_rate: 0.01,
            seed: 42,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    info: usize,
    iteration: u64,
    target: Vec<f64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Memory {
    seen: u64,
    samples: Vec<Sample>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Network {
    input: usize,
    hidden: usize,
    output: usize,
    first: Vec<f64>,
    bias: Vec<f64>,
    second: Vec<f64>,
    out_bias: Vec<f64>,
}
fn random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}
fn unit(state: &mut u64) -> f64 {
    (random(state) >> 11) as f64 / ((1u64 << 53) as f64)
}
fn index(state: &mut u64, n: u64) -> usize {
    let threshold = n.wrapping_neg() % n;
    loop {
        let r = random(state);
        if r >= threshold {
            return (r % n) as usize;
        }
    }
}
fn matching(values: &[f64]) -> Vec<f64> {
    let max = values.iter().copied().fold(0.0_f64, f64::max);
    if max == 0.0 {
        return vec![1.0 / values.len() as f64; values.len()];
    }
    let sum: f64 = values.iter().map(|v| (v / max).max(0.0)).sum();
    values.iter().map(|v| (v / max).max(0.0) / sum).collect()
}
fn pick(state: &mut u64, weights: &[f64]) -> usize {
    let mut r = unit(state);
    for (i, p) in weights.iter().enumerate() {
        if r < *p {
            return i;
        }
        r -= p;
    }
    weights.len() - 1
}
impl Memory {
    fn new() -> Self {
        Self {
            seen: 0,
            samples: Vec::new(),
        }
    }
    fn push(&mut self, sample: Sample, capacity: usize, rng: &mut u64) -> Result<(), GameError> {
        self.seen = self
            .seen
            .checked_add(1)
            .ok_or(GameError("Deep CFR reservoir exhausted"))?;
        if self.samples.len() < capacity {
            self.samples.push(sample);
        } else {
            let replace = index(rng, self.seen);
            if replace < capacity {
                self.samples[replace] = sample;
            }
        }
        Ok(())
    }
}
impl Network {
    fn new(input: usize, hidden: usize, output: usize, rng: &mut u64) -> Self {
        let scale = (6.0 / (input + hidden) as f64).sqrt();
        let first = (0..input * hidden)
            .map(|_| (unit(rng) * 2.0 - 1.0) * scale)
            .collect();
        let scale = (6.0 / (hidden + output) as f64).sqrt();
        let second = (0..hidden * output)
            .map(|_| (unit(rng) * 2.0 - 1.0) * scale)
            .collect();
        Self {
            input,
            hidden,
            output,
            first,
            bias: vec![0.0; hidden],
            second,
            out_bias: vec![0.0; output],
        }
    }
    fn forward(&self, info: usize) -> (Vec<f64>, Vec<f64>) {
        let hidden: Vec<_> = (0..self.hidden)
            .map(|h| (self.first[h * self.input + info] + self.bias[h]).tanh())
            .collect();
        let output = (0..self.output)
            .map(|a| {
                self.out_bias[a]
                    + (0..self.hidden)
                        .map(|h| self.second[a * self.hidden + h] * hidden[h])
                        .sum::<f64>()
            })
            .collect();
        (hidden, output)
    }
    fn train(&mut self, sample: &Sample, weight: f64, rate: f64) {
        let (hidden, output) = self.forward(sample.info);
        let errors: Vec<f64> = output
            .iter()
            .zip(&sample.target)
            .map(|(a, b)| (2.0 * weight * (a - b) / sample.target.len() as f64).clamp(-10.0, 10.0))
            .collect();
        let hidden_errors: Vec<f64> = (0..self.hidden)
            .map(|h| {
                (1.0 - hidden[h] * hidden[h])
                    * errors
                        .iter()
                        .enumerate()
                        .map(|(a, e)| e * self.second[a * self.hidden + h])
                        .sum::<f64>()
            })
            .collect();
        for (a, error) in errors.iter().enumerate() {
            self.out_bias[a] -= rate * error;
            for (h, value) in hidden.iter().enumerate() {
                self.second[a * self.hidden + h] -= rate * error * value;
            }
        }
        for (h, error) in hidden_errors.iter().enumerate() {
            self.bias[h] -= rate * error;
            self.first[h * self.input + sample.info] -= rate * error;
        }
    }
    fn validate(&self, input: usize, hidden: usize, output: usize) -> bool {
        self.input == input
            && self.hidden == hidden
            && self.output == output
            && self.first.len() == input * hidden
            && self.bias.len() == hidden
            && self.second.len() == hidden * output
            && self.out_bias.len() == output
            && self
                .first
                .iter()
                .chain(&self.bias)
                .chain(&self.second)
                .chain(&self.out_bias)
                .all(|v| v.is_finite())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub version: u32,
    pub game_digest: [u8; 32],
    pub iterations: u64,
    pub config: Config,
    rng: u64,
    advantage: [Network; 2],
    strategy: Network,
    advantage_memory: [Memory; 2],
    strategy_memory: Memory,
}
#[derive(Serialize)]
pub struct Report {
    pub backend: &'static str,
    pub strategies: BTreeMap<String, Vec<f64>>,
    pub value: f64,
    pub exploitability: f64,
    pub completed_iterations: usize,
    pub deadline_reached: bool,
    pub checkpoint: Checkpoint,
}
fn dimensions(game: &Game, config: &Config) -> Result<(usize, usize), GameError> {
    let input = game.infos.len();
    let output = game
        .infos
        .iter()
        .map(|i| i.actions.len())
        .max()
        .unwrap_or(0);
    if input == 0
        || input > 512
        || output == 0
        || output > 32
        || !(1..=128).contains(&config.hidden)
        || !(1..=4096).contains(&config.traversals)
        || !(1..=100_000).contains(&config.train_steps)
        || !(1..=16384).contains(&config.memory)
        || !(0.000001..=0.1).contains(&config.learning_rate)
    {
        return Err(GameError("Deep CFR resource/configuration bounds exceeded"));
    }
    Ok((input, output))
}
pub fn initial(game: &Game, config: Config) -> Result<Checkpoint, GameError> {
    let (input, output) = dimensions(game, &config)?;
    let mut rng = config.seed;
    Ok(Checkpoint {
        version: 1,
        game_digest: game.digest,
        iterations: 0,
        advantage: [
            Network::new(input, config.hidden, output, &mut rng),
            Network::new(input, config.hidden, output, &mut rng),
        ],
        strategy: Network::new(input, config.hidden, output, &mut rng),
        advantage_memory: [Memory::new(), Memory::new()],
        strategy_memory: Memory::new(),
        rng,
        config,
    })
}
pub fn validate(game: &Game, cp: &Checkpoint) -> Result<(), GameError> {
    let (input, output) = dimensions(game, &cp.config)?;
    if cp.version != 1
        || cp.game_digest != game.digest
        || cp.iterations > 1_000_000
        || !cp
            .advantage
            .iter()
            .chain(std::iter::once(&cp.strategy))
            .all(|n| n.validate(input, cp.config.hidden, output))
    {
        return Err(GameError("invalid or foreign Deep CFR checkpoint"));
    }
    for memory in cp
        .advantage_memory
        .iter()
        .chain(std::iter::once(&cp.strategy_memory))
    {
        if memory.samples.len() > cp.config.memory
            || memory.seen < memory.samples.len() as u64
            || memory.samples.iter().any(|s| {
                s.info >= input
                    || s.iteration == 0
                    || s.iteration > cp.iterations
                    || s.target.len() != game.infos[s.info].actions.len()
                    || s.target.iter().any(|v| !v.is_finite())
            })
        {
            return Err(GameError("invalid Deep CFR replay memory"));
        }
    }
    Ok(())
}
fn traverse(
    game: &Game,
    id: usize,
    player: usize,
    cp: &mut Checkpoint,
    scale: f64,
    deadline: &Deadline,
) -> Result<f64, GameError> {
    if deadline.expired() {
        return Err(GameError("deadline"));
    }
    Ok(match &game.nodes[id] {
        Node::Terminal(value) => {
            if player == 0 {
                *value / scale
            } else {
                -*value / scale
            }
        }
        Node::Chance(children) => {
            let probabilities: Vec<_> = children.iter().map(|(p, _)| *p).collect();
            let child = children[pick(&mut cp.rng, &probabilities)].1;
            traverse(game, child, player, cp, scale, deadline)?
        }
        Node::Decision { info, children } => {
            let owner = game.infos[*info].player;
            let strategy = matching(&cp.advantage[owner].forward(*info).1[..children.len()]);
            if owner == player {
                let values = children
                    .iter()
                    .map(|child| traverse(game, *child, player, cp, scale, deadline))
                    .collect::<Result<Vec<_>, _>>()?;
                let value: f64 = values.iter().zip(&strategy).map(|(v, p)| v * p).sum();
                cp.advantage_memory[player].push(
                    Sample {
                        info: *info,
                        iteration: cp.iterations,
                        target: values.iter().map(|v| v - value).collect(),
                    },
                    cp.config.memory,
                    &mut cp.rng,
                )?;
                value
            } else {
                cp.strategy_memory.push(
                    Sample {
                        info: *info,
                        iteration: cp.iterations,
                        target: strategy.clone(),
                    },
                    cp.config.memory,
                    &mut cp.rng,
                )?;
                let action = pick(&mut cp.rng, &strategy);
                traverse(game, children[action], player, cp, scale, deadline)?
            }
        }
    })
}
fn fit(
    network: &mut Network,
    memory: &Memory,
    config: &Config,
    iteration: u64,
    rng: &mut u64,
    deadline: &Deadline,
) -> Result<(), GameError> {
    if memory.samples.is_empty() {
        return Ok(());
    }
    for _ in 0..config.train_steps {
        if deadline.expired() {
            return Err(GameError("deadline"));
        }
        let sample = &memory.samples[index(rng, memory.samples.len() as u64)];
        network.train(
            sample,
            sample.iteration as f64 / iteration as f64,
            config.learning_rate,
        );
    }
    Ok(())
}
pub fn resume(
    game: &Game,
    checkpoint: &Checkpoint,
    iterations: usize,
    deadline: &Deadline,
) -> Result<Report, GameError> {
    validate(game, checkpoint)?;
    if iterations == 0
        || iterations > 100_000
        || checkpoint
            .iterations
            .checked_add(iterations as u64)
            .is_none_or(|n| n > 1_000_000)
    {
        return Err(GameError(
            "Deep CFR iterations outside 1..100000 or total exceeds 1000000",
        ));
    }
    let scale = game
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Terminal(v) => Some(v.abs()),
            _ => None,
        })
        .fold(1.0_f64, f64::max);
    let mut cp = checkpoint.clone();
    let mut completed = 0;
    for _ in 0..iterations {
        if deadline.expired() {
            break;
        }
        let mut next = cp.clone();
        next.iterations += 1;
        let iteration = (|| -> Result<(), GameError> {
            for player in 0..2 {
                for _ in 0..next.config.traversals {
                    traverse(game, game.root, player, &mut next, scale, deadline)?;
                }
                next.advantage[player] = Network::new(
                    game.infos.len(),
                    next.config.hidden,
                    next.strategy.output,
                    &mut next.rng,
                );
                fit(
                    &mut next.advantage[player],
                    &next.advantage_memory[player],
                    &next.config,
                    next.iterations,
                    &mut next.rng,
                    deadline,
                )?;
            }
            next.strategy = Network::new(
                game.infos.len(),
                next.config.hidden,
                next.strategy.output,
                &mut next.rng,
            );
            fit(
                &mut next.strategy,
                &next.strategy_memory,
                &next.config,
                next.iterations,
                &mut next.rng,
                deadline,
            )?;
            validate(game, &next)
        })();
        match iteration {
            Ok(()) => {
                cp = next;
                completed += 1;
            }
            Err(GameError("deadline")) => break,
            Err(error) => return Err(error),
        }
    }
    let profile: Vec<Vec<f64>> = game
        .infos
        .iter()
        .enumerate()
        .map(|(i, info)| matching(&cp.strategy.forward(i).1[..info.actions.len()]))
        .collect();
    let value = game.value(game.root, &profile, 0, &BTreeMap::new());
    let exploitability =
        ((game.best_response(0, &profile) + game.best_response(1, &profile)) / 2.0).max(0.0);
    Ok(Report {
        backend: "cpu_deep_cfr_reference",
        strategies: game
            .infos
            .iter()
            .zip(profile)
            .map(|(i, s)| (i.key.clone(), s))
            .collect(),
        value,
        exploitability,
        completed_iterations: completed,
        deadline_reached: completed < iterations,
        checkpoint: cp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoints_resume_exactly_bind_the_game_and_roll_back_expired_iterations() {
        let game = super::super::kuhn();
        let config = Config {
            traversals: 8,
            train_steps: 64,
            hidden: 16,
            memory: 256,
            ..Default::default()
        };
        let first = initial(&game, config).unwrap();
        let full = resume(&game, &first, 8, &Deadline::none()).unwrap();
        let a = resume(&game, &first, 3, &Deadline::none()).unwrap();
        let encoded = serde_json::to_vec(&a.checkpoint).unwrap();
        let restored = serde_json::from_slice(&encoded).unwrap();
        let b = resume(&game, &restored, 5, &Deadline::none()).unwrap();
        assert_eq!(full.checkpoint, b.checkpoint);
        assert!(full.exploitability.is_finite());
        for probabilities in full.strategies.values() {
            assert!((probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        }
        let stopped = resume(&game, &first, 8, &Deadline::from_millis(0)).unwrap();
        assert_eq!(stopped.checkpoint, first);
        let mut forged = first.clone();
        forged.game_digest[0] ^= 1;
        assert!(resume(&game, &forged, 1, &Deadline::none()).is_err());
    }
}
