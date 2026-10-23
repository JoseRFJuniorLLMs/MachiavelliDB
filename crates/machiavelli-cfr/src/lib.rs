//! Regret-matching+ for two-player zero-sum normal-form games.
//! This is the one-information-set case, not an extensive-form/Kuhn Poker solver.
pub mod extensive;
use machiavelli_core::Deadline;
use machiavelli_nash::{Certificate, GameError, ZeroSumGame};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SolverError {
    #[error("iterations must be between 1 and 10000000")]
    InvalidIterations,
    #[error(transparent)]
    Game(#[from] GameError),
}
pub struct Solution {
    pub row_strategy: Vec<f64>,
    pub col_strategy: Vec<f64>,
    pub certificate: Certificate,
    pub iterations: usize,
    pub deadline_reached: bool,
}
fn matching(regrets: &[f64], out: &mut [f64]) {
    let sum: f64 = regrets.iter().sum();
    if sum > 0.0 {
        for (p, r) in out.iter_mut().zip(regrets) {
            *p = r / sum;
        }
    } else {
        let n = out.len() as f64;
        out.fill(1.0 / n);
    }
}
/// Alternating regret updates and linearly weighted average strategies.
/// Deadline interruption returns the accumulated strategy and its measured gap.
pub fn solve(
    game: &ZeroSumGame,
    limit: usize,
    deadline: &Deadline,
) -> Result<Solution, SolverError> {
    if limit == 0 || limit > 10_000_000 {
        return Err(SolverError::InvalidIterations);
    }
    let (n, m) = (game.rows(), game.cols());
    let (mut r, mut c) = (vec![0.0; n], vec![0.0; m]);
    let (mut x, mut y) = (vec![1.0 / n as f64; n], vec![1.0 / m as f64; m]);
    let (mut avg_x, mut avg_y) = (x.clone(), y.clone());
    let (mut ux, mut uy) = (vec![0.0; n], vec![0.0; m]);
    let mut completed = 0;
    let mut total_weight = 0.0;
    for t in 1..=limit {
        if deadline.expired() {
            break;
        }
        matching(&r, &mut x);
        matching(&c, &mut y);
        for i in 0..n {
            ux[i] = (0..m).map(|j| game.payoff(i, j) * y[j]).sum();
        }
        let expected: f64 = x.iter().zip(&ux).map(|(p, v)| p * v).sum();
        for i in 0..n {
            r[i] = (r[i] + ux[i] - expected).max(0.0);
        }
        // Alternating player update uses the updated maximizing strategy.
        matching(&r, &mut x);
        for j in 0..m {
            uy[j] = -(0..n).map(|i| x[i] * game.payoff(i, j)).sum::<f64>();
        }
        let expected: f64 = y.iter().zip(&uy).map(|(p, v)| p * v).sum();
        for j in 0..m {
            c[j] = (c[j] + uy[j] - expected).max(0.0);
        }
        matching(&c, &mut y);
        let weight = t as f64;
        total_weight += weight;
        let blend = weight / total_weight;
        for i in 0..n {
            avg_x[i] += blend * (x[i] - avg_x[i]);
        }
        for j in 0..m {
            avg_y[j] += blend * (y[j] - avg_y[j]);
        }
        completed = t;
    }
    let certificate = game.certify(&avg_x, &avg_y)?;
    Ok(Solution {
        row_strategy: avg_x,
        col_strategy: avg_y,
        certificate,
        iterations: completed,
        deadline_reached: completed < limit,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_core::PayoffMatrix;
    fn run(rows: usize, cols: usize, payoff: Vec<f64>) -> Solution {
        let game = ZeroSumGame::new(&PayoffMatrix::zero_sum(rows, cols, payoff).unwrap()).unwrap();
        solve(&game, 50_000, &Deadline::none()).unwrap()
    }
    #[test]
    fn matching_pennies_and_rps_have_small_measured_gap() {
        let pennies = run(2, 2, vec![1.0, -1.0, -1.0, 1.0]);
        assert!(pennies.certificate.exploitability < 1e-3);
        let rps = run(3, 3, vec![0.0, -1.0, 1.0, 1.0, 0.0, -1.0, -1.0, 1.0, 0.0]);
        assert!(rps.certificate.exploitability < 1e-3);
    }
    #[test]
    fn asymmetric_mixed_and_dominated_games_converge() {
        let mixed = run(2, 2, vec![3.0, 0.0, 0.0, 1.0]);
        assert!(
            mixed.certificate.exploitability < 0.02,
            "{}",
            mixed.certificate.exploitability
        );
        assert!((mixed.row_strategy[0] - 0.25).abs() < 0.02);
        let dominated = run(2, 2, vec![3.0, 2.0, 1.0, 0.0]);
        assert!(dominated.certificate.exploitability < 0.001);
        assert!(dominated.row_strategy[0] > 0.999);
    }
    #[test]
    fn expired_deadline_returns_valid_uniform_fallback() {
        let game =
            ZeroSumGame::new(&PayoffMatrix::zero_sum(1, 2, vec![1.0, -1.0]).unwrap()).unwrap();
        let deadline = Deadline::after(std::time::Duration::ZERO);
        let result = solve(&game, 100, &deadline).unwrap();
        assert_eq!(result.iterations, 0);
        assert!(result.deadline_reached);
        assert_eq!(result.col_strategy, vec![0.5, 0.5]);
        assert!(solve(&game, 0, &deadline).is_err());
    }
}
