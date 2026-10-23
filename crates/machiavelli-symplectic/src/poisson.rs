//! Colchete de Poisson `{f, g} = Σ (∂f/∂q_i ∂g/∂p_i − ∂f/∂p_i ∂g/∂q_i)`.
//!
//! `{f, g} = 0` mede que duas políticas não interferem; `{f, H} = 0` identifica uma
//! invariante de sobrevivência (constante de movimento).

/// Função suave no espaço de fase com gradiente.
pub trait PhaseFunction {
    fn dim(&self) -> usize;
    fn value(&self, q: &[f64], p: &[f64]) -> f64;
    /// `gq ← ∂f/∂q`, `gp ← ∂f/∂p`.
    fn gradient(&self, q: &[f64], p: &[f64], gq: &mut [f64], gp: &mut [f64]);
}

/// Colchete a partir dos gradientes já calculados (kernel SIMD).
pub fn poisson_bracket(df_dq: &[f64], df_dp: &[f64], dg_dq: &[f64], dg_dp: &[f64]) -> f64 {
    crate::simd::bracket_kernel(df_dq, df_dp, dg_dq, dg_dp)
}

/// `{f, g}` avaliado em `(q, p)`.
pub fn poisson_bracket_of<F, G>(f: &F, g: &G, q: &[f64], p: &[f64]) -> f64
where
    F: PhaseFunction + ?Sized,
    G: PhaseFunction + ?Sized,
{
    let n = q.len();
    let (mut fq, mut fp, mut gq, mut gp) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    f.gradient(q, p, &mut fq, &mut fp);
    g.gradient(q, p, &mut gq, &mut gp);
    poisson_bracket(&fq, &fp, &gq, &gp)
}

/// Coordenada `q_i` como função de fase.
#[derive(Clone, Copy, Debug)]
pub struct CoordQ(pub usize, pub usize);

/// Coordenada `p_i` como função de fase.
#[derive(Clone, Copy, Debug)]
pub struct CoordP(pub usize, pub usize);

impl PhaseFunction for CoordQ {
    fn dim(&self) -> usize {
        self.1
    }
    fn value(&self, q: &[f64], _p: &[f64]) -> f64 {
        q[self.0]
    }
    fn gradient(&self, _q: &[f64], _p: &[f64], gq: &mut [f64], gp: &mut [f64]) {
        gq.fill(0.0);
        gp.fill(0.0);
        gq[self.0] = 1.0;
    }
}

impl PhaseFunction for CoordP {
    fn dim(&self) -> usize {
        self.1
    }
    fn value(&self, _q: &[f64], p: &[f64]) -> f64 {
        p[self.0]
    }
    fn gradient(&self, _q: &[f64], _p: &[f64], gq: &mut [f64], gp: &mut [f64]) {
        gq.fill(0.0);
        gp.fill(0.0);
        gp[self.0] = 1.0;
    }
}

/// Momento angular `L = q_0 p_1 − q_1 p_0` no plano das duas primeiras coordenadas.
#[derive(Clone, Copy, Debug)]
pub struct AngularMomentum2D {
    pub n: usize,
}

impl PhaseFunction for AngularMomentum2D {
    fn dim(&self) -> usize {
        self.n
    }
    fn value(&self, q: &[f64], p: &[f64]) -> f64 {
        q[0] * p[1] - q[1] * p[0]
    }
    fn gradient(&self, q: &[f64], p: &[f64], gq: &mut [f64], gp: &mut [f64]) {
        gq.fill(0.0);
        gp.fill(0.0);
        gq[0] = p[1];
        gq[1] = -p[0];
        gp[0] = -q[1];
        gp[1] = q[0];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{DiagonalKinetic, Separable};
    use crate::systems::Harmonic;

    #[test]
    fn canonical_brackets() {
        let n = 3;
        let q = [0.3, -0.2, 0.9];
        let p = [1.0, 0.5, -0.4];
        for i in 0..n {
            for j in 0..n {
                let qq = poisson_bracket_of(&CoordQ(i, n), &CoordQ(j, n), &q, &p);
                let qp = poisson_bracket_of(&CoordQ(i, n), &CoordP(j, n), &q, &p);
                let pp = poisson_bracket_of(&CoordP(i, n), &CoordP(j, n), &q, &p);
                assert_eq!(qq, 0.0);
                assert_eq!(pp, 0.0);
                assert_eq!(qp, if i == j { 1.0 } else { 0.0 });
            }
        }
    }

    #[test]
    fn angular_momentum_is_invariant_of_isotropic_oscillator() {
        let h = Separable::new(
            DiagonalKinetic::uniform(2, 1.0),
            Harmonic::isotropic(2, 3.0),
        );
        let l = AngularMomentum2D { n: 2 };
        for (q, p) in [([1.0, 0.0], [0.0, 1.0]), ([0.3, -2.0], [1.5, 0.25])] {
            let b = poisson_bracket_of(&l, &h, &q, &p);
            assert!(b.abs() < 1e-14, "{{L, H}} = {b}");
        }
        // Num oscilador anisotrópico L deixa de ser conservado.
        let aniso = Separable::new(
            DiagonalKinetic::uniform(2, 1.0),
            Harmonic {
                stiffness: vec![1.0, 2.0],
                center: vec![0.0, 0.0],
            },
        );
        assert!(poisson_bracket_of(&l, &aniso, &[1.0, 1.0], &[0.0, 0.0]).abs() > 0.5);
    }

    #[test]
    fn bracket_is_antisymmetric() {
        let h = Separable::new(
            DiagonalKinetic::uniform(2, 2.0),
            crate::systems::HenonHeiles,
        );
        let l = AngularMomentum2D { n: 2 };
        let (q, p) = ([0.1, 0.2], [0.3, -0.4]);
        let a = poisson_bracket_of(&l, &h, &q, &p);
        let b = poisson_bracket_of(&h, &l, &q, &p);
        assert!((a + b).abs() < 1e-15);
    }
}
