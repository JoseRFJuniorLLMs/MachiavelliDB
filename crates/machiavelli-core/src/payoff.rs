//! Matrizes de payoff bimatriciais (`cf_payoff_tensors`).

use crate::error::CoreError;
use serde::{Deserialize, Serialize};

/// Jogo bimatricial `rows × cols`: `row_payoff[i·cols + j]` para o jogador linha e
/// `col_payoff[i·cols + j]` para o jogador coluna.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PayoffMatrix {
    pub rows: u32,
    pub cols: u32,
    pub row_payoff: Vec<f64>,
    pub col_payoff: Vec<f64>,
}

impl PayoffMatrix {
    pub fn new(
        rows: usize,
        cols: usize,
        row_payoff: Vec<f64>,
        col_payoff: Vec<f64>,
    ) -> Result<Self, CoreError> {
        let m = Self {
            rows: u32::try_from(rows).map_err(|_| CoreError::OutOfDomain {
                field: "rows",
                value: rows as f64,
                domain: "[1, u32::MAX]",
            })?,
            cols: u32::try_from(cols).map_err(|_| CoreError::OutOfDomain {
                field: "cols",
                value: cols as f64,
                domain: "[1, u32::MAX]",
            })?,
            row_payoff,
            col_payoff,
        };
        m.validate()?;
        Ok(m)
    }

    /// Jogo de soma zero a partir da matriz do jogador linha.
    pub fn zero_sum(rows: usize, cols: usize, row_payoff: Vec<f64>) -> Result<Self, CoreError> {
        let col_payoff = row_payoff.iter().map(|x| -x).collect();
        Self::new(rows, cols, row_payoff, col_payoff)
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        let len = (self.rows as usize)
            .checked_mul(self.cols as usize)
            .ok_or_else(|| CoreError::Codec("dimensões de payoff excedem usize".into()))?;
        if len == 0 || self.row_payoff.len() != len || self.col_payoff.len() != len {
            return Err(CoreError::InvalidLength {
                expected: len,
                actual: self.row_payoff.len().max(self.col_payoff.len()),
            });
        }
        if self
            .row_payoff
            .iter()
            .chain(&self.col_payoff)
            .any(|x| !x.is_finite())
        {
            return Err(CoreError::Codec("payoff não finito".into()));
        }
        Ok(())
    }

    pub fn row(&self, i: usize, j: usize) -> f64 {
        self.row_payoff[i * self.cols as usize + j]
    }

    pub fn col(&self, i: usize, j: usize) -> f64 {
        self.col_payoff[i * self.cols as usize + j]
    }

    pub fn is_zero_sum(&self, tol: f64) -> bool {
        self.row_payoff
            .iter()
            .zip(&self.col_payoff)
            .all(|(a, b)| (a + b).abs() <= tol)
    }

    pub fn encode(&self) -> Result<Vec<u8>, CoreError> {
        crate::codec::to_rkyv(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CoreError> {
        let m: Self = crate::codec::from_rkyv(bytes)?;
        m.validate()?;
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn rejects_dimensions_that_would_truncate_to_a_valid_shape() {
        let too_large = u32::MAX as usize + 2;
        assert!(matches!(
            PayoffMatrix::new(too_large, 1, vec![1.0], vec![-1.0]),
            Err(CoreError::OutOfDomain { field: "rows", .. })
        ));
        assert!(matches!(
            PayoffMatrix::new(1, too_large, vec![1.0], vec![-1.0]),
            Err(CoreError::OutOfDomain { field: "cols", .. })
        ));
    }

    #[test]
    fn rejects_shapes_too_large_for_the_supplied_values() {
        let matrix = PayoffMatrix {
            rows: u32::MAX,
            cols: u32::MAX,
            row_payoff: Vec::new(),
            col_payoff: Vec::new(),
        };
        assert!(matrix.validate().is_err());
    }

    #[test]
    fn valid_zero_sum_matrix_roundtrips() {
        let matrix = PayoffMatrix::zero_sum(2, 2, vec![1.0, -1.0, -1.0, 1.0]).unwrap();
        assert!(matrix.is_zero_sum(0.0));
        assert_eq!(matrix.row(1, 0), -1.0);
        assert_eq!(matrix.col(1, 0), 1.0);
        assert_eq!(
            PayoffMatrix::decode(&matrix.encode().unwrap()).unwrap(),
            matrix
        );
    }
}
