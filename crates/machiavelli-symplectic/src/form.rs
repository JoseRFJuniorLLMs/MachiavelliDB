//! Estrutura simplética canónica `ω = Σ dq_i ∧ dp_i` e a sua matriz `J`.

use crate::linalg::Matrix;

/// `J = [[0, I_n], [−I_n, 0]]` (dimensão `2n × 2n`).
pub fn canonical_j(n: usize) -> Matrix {
    let mut j = Matrix::zeros(2 * n, 2 * n);
    for i in 0..n {
        j[(i, n + i)] = 1.0;
        j[(n + i, i)] = -1.0;
    }
    j
}

/// `ω(u, v) = uᵀ J v = Σ (u_q·v_p − u_p·v_q)` para `u, v ∈ ℝ²ⁿ` no layout `[q | p]`.
pub fn omega(u: &[f64], v: &[f64]) -> f64 {
    assert_eq!(u.len(), v.len());
    assert!(
        u.len().is_multiple_of(2),
        "o espaço de fase tem dimensão par"
    );
    let n = u.len() / 2;
    let (uq, up) = u.split_at(n);
    let (vq, vp) = v.split_at(n);
    crate::simd::bracket_kernel(uq, up, vq, vp)
}

/// `J z = (p, −q)`: o gradiente simplético usado nas equações de Hamilton `ż = J ∇H`.
pub fn apply_j(z: &[f64]) -> Vec<f64> {
    let n = z.len() / 2;
    let mut out = Vec::with_capacity(z.len());
    out.extend_from_slice(&z[n..]);
    out.extend(z[..n].iter().map(|x| -x));
    out
}

/// `max |SᵀJS − J|`: zero exatamente para matrizes simpléticas.
pub fn symplectic_defect(s: &Matrix) -> f64 {
    assert_eq!(s.rows(), s.cols());
    assert!(s.rows().is_multiple_of(2));
    let j = canonical_j(s.rows() / 2);
    s.transpose().mul(&j).mul(s).sub(&j).max_abs()
}

pub fn is_symplectic(s: &Matrix, tol: f64) -> bool {
    symplectic_defect(s) <= tol
}

/// Cisalhamento `[[I, A], [0, I]]`, simplético para `A` simétrica.
pub fn shear_upper(a: &Matrix) -> Matrix {
    assert!(a.is_symmetric(1e-12), "o cisalhamento exige A simétrica");
    let n = a.rows();
    let mut s = Matrix::identity(2 * n);
    for i in 0..n {
        for j in 0..n {
            s[(i, n + j)] = a[(i, j)];
        }
    }
    s
}

/// Cisalhamento `[[I, 0], [B, I]]`, simplético para `B` simétrica.
pub fn shear_lower(b: &Matrix) -> Matrix {
    assert!(b.is_symmetric(1e-12), "o cisalhamento exige B simétrica");
    let n = b.rows();
    let mut s = Matrix::identity(2 * n);
    for i in 0..n {
        for j in 0..n {
            s[(n + i, j)] = b[(i, j)];
        }
    }
    s
}

/// Rotação independente em cada plano `(q_i, p_i)`, simplética para quaisquer ângulos.
pub fn plane_rotations(angles: &[f64]) -> Matrix {
    let n = angles.len();
    let mut s = Matrix::zeros(2 * n, 2 * n);
    for (i, &a) in angles.iter().enumerate() {
        let (sin, cos) = a.sin_cos();
        s[(i, i)] = cos;
        s[(i, n + i)] = sin;
        s[(n + i, i)] = -sin;
        s[(n + i, n + i)] = cos;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j_squares_to_minus_identity_and_is_orthogonal() {
        for n in [1, 3, 64] {
            let j = canonical_j(n);
            let minus_i = Matrix::identity(2 * n).scale(-1.0);
            assert_eq!(j.mul(&j), minus_i);
            assert_eq!(j.transpose().mul(&j), Matrix::identity(2 * n));
            assert_eq!(symplectic_defect(&j), 0.0);
        }
    }

    #[test]
    fn omega_is_antisymmetric_and_nondegenerate() {
        let u = [1.0, 2.0, 3.0, 4.0];
        let v = [-1.0, 0.5, 2.0, -3.0];
        assert_eq!(omega(&u, &v), -omega(&v, &u));
        assert_eq!(omega(&u, &u), 0.0);
        // ω(e_q1, e_p1) = 1
        assert_eq!(omega(&[1.0, 0.0, 0.0, 0.0], &[0.0, 0.0, 1.0, 0.0]), 1.0);
    }

    #[test]
    fn generators_are_symplectic() {
        let a = Matrix::from_rows(&[vec![2.0, 0.5], vec![0.5, -1.0]]);
        let s = shear_upper(&a)
            .mul(&shear_lower(&a.scale(0.3)))
            .mul(&plane_rotations(&[0.7, -1.9]));
        assert!(symplectic_defect(&s) < 1e-13);
        assert!((crate::linalg::determinant(&s) - 1.0).abs() < 1e-12);
    }
}
