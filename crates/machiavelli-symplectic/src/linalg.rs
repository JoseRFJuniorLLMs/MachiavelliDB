//! Álgebra linear densa mínima (sem dependências): produtos, LU, determinante,
//! resolução de sistemas e autovalores simétricos por Jacobi cíclico.

use std::fmt;
use std::ops::{Index, IndexMut};

/// Matriz densa em ordem de linhas.
#[derive(Clone, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl fmt::Debug for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Matrix {}x{}", self.rows, self.cols)?;
        for i in 0..self.rows.min(8) {
            let row: Vec<String> = (0..self.cols.min(8))
                .map(|j| format!("{:>10.4e}", self[(i, j)]))
                .collect();
            writeln!(f, "  [{}]", row.join(", "))?;
        }
        Ok(())
    }
}

impl Index<(usize, usize)> for Matrix {
    type Output = f64;
    fn index(&self, (i, j): (usize, usize)) -> &f64 {
        &self.data[i * self.cols + j]
    }
}

impl IndexMut<(usize, usize)> for Matrix {
    fn index_mut(&mut self, (i, j): (usize, usize)) -> &mut f64 {
        &mut self.data[i * self.cols + j]
    }
}

impl Matrix {
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m[(i, i)] = 1.0;
        }
        m
    }

    pub fn from_vec(rows: usize, cols: usize, data: Vec<f64>) -> Self {
        assert_eq!(data.len(), rows * cols, "dimensões incoerentes");
        Self { rows, cols, data }
    }

    pub fn from_rows(rows: &[Vec<f64>]) -> Self {
        let r = rows.len();
        let c = rows.first().map_or(0, Vec::len);
        let mut data = Vec::with_capacity(r * c);
        for row in rows {
            assert_eq!(row.len(), c, "linhas com tamanhos diferentes");
            data.extend_from_slice(row);
        }
        Self::from_vec(r, c, data)
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn data(&self) -> &[f64] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [f64] {
        &mut self.data
    }

    pub fn row(&self, i: usize) -> &[f64] {
        &self.data[i * self.cols..(i + 1) * self.cols]
    }

    pub fn transpose(&self) -> Matrix {
        let mut t = Matrix::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                t[(j, i)] = self[(i, j)];
            }
        }
        t
    }

    pub fn mul(&self, other: &Matrix) -> Matrix {
        assert_eq!(self.cols, other.rows, "produto com dimensões incoerentes");
        let mut out = Matrix::zeros(self.rows, other.cols);
        for i in 0..self.rows {
            for k in 0..self.cols {
                let a = self[(i, k)];
                if a == 0.0 {
                    continue;
                }
                let orow = other.row(k);
                let dst = &mut out.data[i * other.cols..(i + 1) * other.cols];
                for (d, o) in dst.iter_mut().zip(orow) {
                    *d += a * o;
                }
            }
        }
        out
    }

    pub fn mul_vec(&self, v: &[f64]) -> Vec<f64> {
        assert_eq!(self.cols, v.len());
        (0..self.rows)
            .map(|i| self.row(i).iter().zip(v).map(|(a, b)| a * b).sum())
            .collect()
    }

    pub fn sub(&self, other: &Matrix) -> Matrix {
        assert_eq!((self.rows, self.cols), (other.rows, other.cols));
        Matrix::from_vec(
            self.rows,
            self.cols,
            self.data
                .iter()
                .zip(&other.data)
                .map(|(a, b)| a - b)
                .collect(),
        )
    }

    pub fn scale(&self, s: f64) -> Matrix {
        Matrix::from_vec(
            self.rows,
            self.cols,
            self.data.iter().map(|a| a * s).collect(),
        )
    }

    pub fn max_abs(&self) -> f64 {
        self.data.iter().fold(0.0, |m, x| m.max(x.abs()))
    }

    pub fn is_symmetric(&self, tol: f64) -> bool {
        self.rows == self.cols
            && (0..self.rows).all(|i| (0..i).all(|j| (self[(i, j)] - self[(j, i)]).abs() <= tol))
    }

    /// Simetriza `(A + Aᵀ)/2` no lugar.
    pub fn symmetrize(&mut self) {
        assert_eq!(self.rows, self.cols);
        for i in 0..self.rows {
            for j in 0..i {
                let m = 0.5 * (self[(i, j)] + self[(j, i)]);
                self[(i, j)] = m;
                self[(j, i)] = m;
            }
        }
    }
}

/// Decomposição LU com pivotagem parcial. Devolve `(LU, permutação, sinal)` ou `None` se singular.
pub fn lu_decompose(a: &Matrix) -> Option<(Matrix, Vec<usize>, f64)> {
    assert_eq!(a.rows, a.cols, "LU exige matriz quadrada");
    let n = a.rows;
    let mut lu = a.clone();
    let mut perm: Vec<usize> = (0..n).collect();
    let mut sign = 1.0;
    for k in 0..n {
        let (piv, pmax) = (k..n)
            .map(|i| (i, lu[(i, k)].abs()))
            .fold(
                (k, -1.0),
                |best, cur| if cur.1 > best.1 { cur } else { best },
            );
        if pmax == 0.0 {
            return None;
        }
        if piv != k {
            for j in 0..n {
                lu.data.swap(k * n + j, piv * n + j);
            }
            perm.swap(k, piv);
            sign = -sign;
        }
        let pivot = lu[(k, k)];
        for i in k + 1..n {
            let f = lu[(i, k)] / pivot;
            lu[(i, k)] = f;
            if f != 0.0 {
                for j in k + 1..n {
                    let v = lu[(k, j)];
                    lu[(i, j)] -= f * v;
                }
            }
        }
    }
    Some((lu, perm, sign))
}

pub fn determinant(a: &Matrix) -> f64 {
    match lu_decompose(a) {
        None => 0.0,
        Some((lu, _, sign)) => (0..a.rows).fold(sign, |d, i| d * lu[(i, i)]),
    }
}

/// Resolve `A x = b`; `None` se `A` for singular.
pub fn solve(a: &Matrix, b: &[f64]) -> Option<Vec<f64>> {
    let n = a.rows;
    assert_eq!(b.len(), n);
    let (lu, perm, _) = lu_decompose(a)?;
    let mut y: Vec<f64> = perm.iter().map(|&p| b[p]).collect();
    for i in 0..n {
        for k in 0..i {
            y[i] -= lu[(i, k)] * y[k];
        }
    }
    for i in (0..n).rev() {
        for k in i + 1..n {
            y[i] -= lu[(i, k)] * y[k];
        }
        y[i] /= lu[(i, i)];
    }
    Some(y)
}

/// Autovalores e autovetores de uma matriz simétrica (Jacobi cíclico).
/// Devolve os autovalores por ordem crescente e os autovetores nas colunas.
pub fn symmetric_eigen(a: &Matrix) -> (Vec<f64>, Matrix) {
    assert_eq!(a.rows, a.cols);
    let n = a.rows;
    let mut m = a.clone();
    m.symmetrize();
    let mut v = Matrix::identity(n);
    let scale = m.max_abs().max(f64::MIN_POSITIVE);
    for _sweep in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| m[(i, j)] * m[(i, j)])
            .sum::<f64>()
            .sqrt();
        if off <= 1e-14 * scale {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = m[(p, q)];
                if apq.abs() <= 1e-300 {
                    continue;
                }
                let app = m[(p, p)];
                let aqq = m[(q, q)];
                // cot(2φ) = θ; t = tan(φ) é a raiz de menor módulo de t² + 2tθ − 1 = 0.
                let theta = (aqq - app) / (2.0 * apq);
                let sign = if theta >= 0.0 { 1.0 } else { -1.0 };
                let t = sign / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let mkp = m[(k, p)];
                    let mkq = m[(k, q)];
                    m[(k, p)] = c * mkp - s * mkq;
                    m[(k, q)] = s * mkp + c * mkq;
                }
                for k in 0..n {
                    let mpk = m[(p, k)];
                    let mqk = m[(q, k)];
                    m[(p, k)] = c * mpk - s * mqk;
                    m[(q, k)] = s * mpk + c * mqk;
                }
                for k in 0..n {
                    let vkp = v[(k, p)];
                    let vkq = v[(k, q)];
                    v[(k, p)] = c * vkp - s * vkq;
                    v[(k, q)] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| m[(i, i)].total_cmp(&m[(j, j)]));
    let values = order.iter().map(|&i| m[(i, i)]).collect();
    let mut vecs = Matrix::zeros(n, n);
    for (new_col, &old_col) in order.iter().enumerate() {
        for k in 0..n {
            vecs[(k, new_col)] = v[(k, old_col)];
        }
    }
    (values, vecs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determinant_and_solve() {
        let a = Matrix::from_rows(&[
            vec![4.0, 3.0, 2.0],
            vec![2.0, 1.0, 3.0],
            vec![3.0, 2.0, 1.0],
        ]);
        // 4(1−6) − 3(2−9) + 2(4−3) = 3
        assert!((determinant(&a) - 3.0).abs() < 1e-12);
        let x = solve(&a, &[1.0, 2.0, 3.0]).unwrap();
        let back = a.mul_vec(&x);
        for (b, e) in back.iter().zip([1.0, 2.0, 3.0]) {
            assert!((b - e).abs() < 1e-12);
        }
        assert!(solve(&Matrix::zeros(2, 2), &[1.0, 1.0]).is_none());
    }

    #[test]
    fn jacobi_eigen_reconstructs() {
        let a = Matrix::from_rows(&[
            vec![2.0, -1.0, 0.0],
            vec![-1.0, 2.0, -1.0],
            vec![0.0, -1.0, 2.0],
        ]);
        let (vals, vecs) = symmetric_eigen(&a);
        let expected = [2.0 - 2f64.sqrt(), 2.0, 2.0 + 2f64.sqrt()];
        for (v, e) in vals.iter().zip(expected) {
            assert!((v - e).abs() < 1e-12, "{v} vs {e}");
        }
        for (k, &lambda) in vals.iter().enumerate() {
            let col: Vec<f64> = (0..3).map(|i| vecs[(i, k)]).collect();
            let av = a.mul_vec(&col);
            for i in 0..3 {
                assert!((av[i] - lambda * col[i]).abs() < 1e-12);
            }
        }
    }
}
