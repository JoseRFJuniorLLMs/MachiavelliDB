//! Normal-form, two-player zero-sum games and best-response certificates.
use machiavelli_core::PayoffMatrix;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum GameError {
    #[error("invalid game or strategy: {0}")]
    Invalid(&'static str),
}
pub struct ZeroSumGame {
    rows: usize,
    cols: usize,
    values: Vec<f64>,
    scale: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Certificate {
    pub value: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
    /// Sum of both players' unilateral best-response improvements.
    pub exploitability: f64,
}
impl ZeroSumGame {
    pub fn new(matrix: &PayoffMatrix) -> Result<Self, GameError> {
        matrix
            .validate()
            .map_err(|_| GameError::Invalid("payoff shape or numbers"))?;
        if !matrix.is_zero_sum(0.0) {
            return Err(GameError::Invalid("requires exact zero-sum payoffs"));
        }
        let magnitude = matrix
            .row_payoff
            .iter()
            .fold(0.0_f64, |m, v| m.max(v.abs()));
        if magnitude > f64::MAX / 4.0 {
            return Err(GameError::Invalid("payoffs too large"));
        }
        let scale = if magnitude == 0.0 { 1.0 } else { magnitude };
        Ok(Self {
            rows: matrix.rows as usize,
            cols: matrix.cols as usize,
            values: matrix.row_payoff.iter().map(|v| v / scale).collect(),
            scale,
        })
    }
    pub fn rows(&self) -> usize {
        self.rows
    }
    pub fn cols(&self) -> usize {
        self.cols
    }
    /// Normalized payoff, used by iterative solvers to avoid arithmetic overflow.
    pub fn payoff(&self, row: usize, col: usize) -> f64 {
        assert!(row < self.rows && col < self.cols);
        self.values[row * self.cols + col]
    }
    pub fn certify(&self, row: &[f64], col: &[f64]) -> Result<Certificate, GameError> {
        for (strategy, dim) in [(row, self.rows), (col, self.cols)] {
            if strategy.len() != dim
                || strategy.iter().any(|v| !v.is_finite() || *v < 0.0)
                || (strategy.iter().sum::<f64>() - 1.0).abs() > 1e-10
            {
                return Err(GameError::Invalid("strategy must be a probability simplex"));
            }
        }
        let row_values: Vec<f64> = (0..self.rows)
            .map(|i| {
                col.iter()
                    .enumerate()
                    .map(|(j, p)| p * self.payoff(i, j))
                    .sum()
            })
            .collect();
        let col_values: Vec<f64> = (0..self.cols)
            .map(|j| {
                row.iter()
                    .enumerate()
                    .map(|(i, p)| p * self.payoff(i, j))
                    .sum()
            })
            .collect();
        let lower = col_values.iter().copied().fold(f64::INFINITY, f64::min);
        let upper = row_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let value = row.iter().zip(row_values).map(|(p, v)| p * v).sum::<f64>() * self.scale;
        Ok(Certificate {
            value,
            lower_bound: lower * self.scale,
            upper_bound: upper * self.scale,
            exploitability: (upper - lower).max(0.0) * self.scale,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn certificate_detects_exploitable_and_equilibrium_profiles() {
        let game =
            ZeroSumGame::new(&PayoffMatrix::zero_sum(2, 2, vec![1.0, -1.0, -1.0, 1.0]).unwrap())
                .unwrap();
        assert_eq!(
            game.certify(&[0.5, 0.5], &[0.5, 0.5])
                .unwrap()
                .exploitability,
            0.0
        );
        assert_eq!(
            game.certify(&[1.0, 0.0], &[1.0, 0.0])
                .unwrap()
                .exploitability,
            2.0
        );
        assert!(game.certify(&[f64::NAN, 0.0], &[0.5, 0.5]).is_err());
        assert!(game.certify(&[0.2, 0.2], &[0.5, 0.5]).is_err());
    }
}
