//! `PhaseVector`: ponto `z = (q, p)` do fibrado cotangente `T*Q`, mais as coordenadas de Casimir.
//!
//! As coordenadas `c` (constitucionais) vivem no núcleo do tensor de Poisson estendido
//! `B = diag(J, 0)`: nenhum Hamiltoniano as move (`dc/dt = {c, H} = 0`). É nelas que a
//! SPEC-0017 grava as regras invioláveis. Numa variedade simplética pura (`ω` não
//! degenerada) as únicas funções de Casimir são constantes, por isso a estrutura de
//! Poisson tem de ser estendida para que as invariantes constitucionais existam.

use crate::error::CoreError;
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Debug,
    PartialEq,
    Default,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PhaseVector {
    q: Vec<f64>,
    p: Vec<f64>,
    c: Vec<f64>,
}

impl PhaseVector {
    /// Vetor de fase sem coordenadas constitucionais.
    pub fn new(q: Vec<f64>, p: Vec<f64>) -> Result<Self, CoreError> {
        Self::with_constitution(q, p, Vec::new())
    }

    /// Vetor de fase com coordenadas de Casimir `c`.
    pub fn with_constitution(q: Vec<f64>, p: Vec<f64>, c: Vec<f64>) -> Result<Self, CoreError> {
        let v = Self { q, p, c };
        v.validate()?;
        Ok(v)
    }

    /// Estado nulo de dimensão de configuração `n` (vetor de `2n` floats).
    pub fn zeros(n: usize) -> Self {
        Self {
            q: vec![0.0; n],
            p: vec![0.0; n],
            c: Vec::new(),
        }
    }

    /// Reconstrói a partir de `z = [q | p]`.
    pub fn from_z(z: &[f64], c: Vec<f64>) -> Result<Self, CoreError> {
        if !z.len().is_multiple_of(2) || z.is_empty() {
            return Err(CoreError::InvalidPhase(format!(
                "z deve ter dimensão par 2n > 0, tem {}",
                z.len()
            )));
        }
        let n = z.len() / 2;
        Self::with_constitution(z[..n].to_vec(), z[n..].to_vec(), c)
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        if self.q.is_empty() {
            return Err(CoreError::InvalidPhase("dimensão n = 0".into()));
        }
        if self.q.len() != self.p.len() {
            return Err(CoreError::InvalidPhase(format!(
                "dim(q) = {} ≠ dim(p) = {}",
                self.q.len(),
                self.p.len()
            )));
        }
        if let Some(bad) = self
            .q
            .iter()
            .chain(&self.p)
            .chain(&self.c)
            .find(|x| !x.is_finite())
        {
            return Err(CoreError::InvalidPhase(format!(
                "coordenada não finita: {bad}"
            )));
        }
        Ok(())
    }

    /// Dimensão de configuração `n` (o espaço de fase tem dimensão `2n`).
    pub fn dim(&self) -> usize {
        self.q.len()
    }

    pub fn q(&self) -> &[f64] {
        &self.q
    }

    pub fn p(&self) -> &[f64] {
        &self.p
    }

    /// Coordenadas de Casimir (constitucionais). Não há acesso mutável: a dinâmica
    /// nunca as altera e o núcleo de invariantes rejeita qualquer escrita que as mude.
    pub fn c(&self) -> &[f64] {
        &self.c
    }

    pub fn q_mut(&mut self) -> &mut [f64] {
        &mut self.q
    }

    pub fn p_mut(&mut self) -> &mut [f64] {
        &mut self.p
    }

    /// Empréstimo simultâneo de `q` e `p` (para os integradores).
    pub fn qp_mut(&mut self) -> (&mut [f64], &mut [f64]) {
        (&mut self.q, &mut self.p)
    }

    /// `z = [q | p]`.
    pub fn to_z(&self) -> Vec<f64> {
        let mut z = Vec::with_capacity(2 * self.dim());
        z.extend_from_slice(&self.q);
        z.extend_from_slice(&self.p);
        z
    }

    /// Norma euclidiana de `z`.
    pub fn norm(&self) -> f64 {
        self.q
            .iter()
            .chain(&self.p)
            .map(|x| x * x)
            .sum::<f64>()
            .sqrt()
    }

    /// Serialização zero-copy (`rkyv`), formato de `cf_phase_space`.
    pub fn encode(&self) -> Result<Vec<u8>, CoreError> {
        crate::codec::to_rkyv(self)
    }

    /// Desserialização validada.
    pub fn decode(bytes: &[u8]) -> Result<Self, CoreError> {
        let v: Self = crate::codec::from_rkyv(bytes)?;
        v.validate()?;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_mismatched_and_non_finite() {
        assert!(PhaseVector::new(vec![1.0], vec![1.0, 2.0]).is_err());
        assert!(PhaseVector::new(vec![f64::NAN], vec![0.0]).is_err());
        assert!(PhaseVector::new(vec![], vec![]).is_err());
    }

    #[test]
    fn z_roundtrip_and_rkyv() {
        let v = PhaseVector::with_constitution(vec![1.0, 2.0], vec![3.0, 4.0], vec![1.0]).unwrap();
        assert_eq!(v.to_z(), vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(PhaseVector::from_z(&v.to_z(), vec![1.0]).unwrap(), v);
        let bytes = v.encode().unwrap();
        assert_eq!(PhaseVector::decode(&bytes).unwrap(), v);
    }

    #[test]
    fn decode_handles_unaligned_input() {
        let v = PhaseVector::new(vec![0.5; 64], vec![-0.5; 64]).unwrap();
        let bytes = v.encode().unwrap();
        let mut shifted = vec![0u8];
        shifted.extend_from_slice(&bytes);
        assert_eq!(PhaseVector::decode(&shifted[1..]).unwrap(), v);
    }
}
