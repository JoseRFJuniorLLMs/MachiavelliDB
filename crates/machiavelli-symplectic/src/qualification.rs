//! Medições de conservação usadas pela SPEC-0001 (invariante de qualificação) e pelo
//! gate M1 da SPEC-0018.
//!
//! Um integrador simplético não conserva `H` exatamente: conserva um Hamiltoniano
//! modificado `H̃ = H + O(hᵖ)`. Por isso o erro de energia oscila com amplitude limitada
//! (proporcional a `hᵖ`) e **não tem deriva secular**. Este relatório separa as duas
//! grandezas:
//! - `max_rel_error`: amplitude máxima de `|H(t) − H₀| / |H₀|`, que depende do passo;
//! - `secular_drift`: diferença entre as médias do erro no último e no primeiro décimo
//!   da corrida. Para um esquema simplético fica no nível do arredondamento,
//!   qualquer que seja o passo.

use crate::integrators::{Integrator, Scheme};
use crate::system::{DiagonalKinetic, Separable};
use crate::systems::FpuChain;
use crate::tangent::TangentMap;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConservationReport {
    pub system: String,
    pub scheme: Scheme,
    pub dim: usize,
    pub dt: f64,
    pub steps: usize,
    pub initial_energy: f64,
    pub max_rel_error: f64,
    pub final_rel_error: f64,
    pub secular_drift: f64,
    pub compensated: bool,
    pub elapsed_ms: f64,
    pub simd_backend: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiouvilleReport {
    pub system: String,
    pub scheme: Scheme,
    pub dim: usize,
    pub dt: f64,
    pub steps: usize,
    /// `|det Φ − 1|`.
    pub volume_error: f64,
    /// `max |ΦᵀJΦ − J|` relativo a `max|Φ|²`.
    pub symplectic_defect: f64,
    pub elapsed_ms: f64,
}

/// Estado inicial determinístico da cadeia FPU: modos baixos com amplitude `amplitude`.
pub fn fpu_initial_state(n: usize, amplitude: f64) -> (Vec<f64>, Vec<f64>) {
    let q = (1..=n)
        .map(|k| {
            let x = k as f64 / (n + 1) as f64 * std::f64::consts::PI;
            amplitude * (x.sin() + 0.5 * (3.0 * x).sin() - 0.25 * (5.0 * x).sin())
        })
        .collect();
    let p = (1..=n)
        .map(|k| {
            let x = k as f64 / (n + 1) as f64 * std::f64::consts::PI;
            0.5 * amplitude * (2.0 * x).sin()
        })
        .collect();
    (q, p)
}

/// Integra a cadeia FPU-β e mede a conservação de energia.
pub fn fpu_conservation(
    scheme: Scheme,
    n: usize,
    beta: f64,
    amplitude: f64,
    dt: f64,
    steps: usize,
    compensated: bool,
) -> ConservationReport {
    let sys = Separable::new(
        DiagonalKinetic::uniform(n, 1.0),
        FpuChain {
            n,
            alpha: 0.0,
            beta,
        },
    );
    let (mut q, mut p) = fpu_initial_state(n, amplitude);
    let h0 = sys.energy(&q, &p);
    let mut integrator = Integrator::new(&sys, scheme);
    if compensated {
        integrator = integrator.compensated();
    }
    let samples_target = 2_000usize;
    let every = (steps / samples_target).max(1);
    let mut errors = Vec::with_capacity(steps / every + 1);
    let mut max_err: f64 = 0.0;
    let start = Instant::now();
    integrator
        .run_observed(&mut q, &mut p, dt, steps, every, |_, q, p| {
            let e = (sys.energy(q, p) - h0) / h0.abs();
            max_err = max_err.max(e.abs());
            errors.push(e);
        })
        .expect("dimensões coerentes");
    let elapsed_ms = start.elapsed().as_secs_f64() * 1e3;
    let final_err = (sys.energy(&q, &p) - h0) / h0.abs();
    max_err = max_err.max(final_err.abs());
    let tenth = (errors.len() / 10).max(1);
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    let secular_drift = if errors.len() >= 2 * tenth {
        (mean(&errors[errors.len() - tenth..]) - mean(&errors[..tenth])).abs()
    } else {
        final_err.abs()
    };
    ConservationReport {
        system: format!("FPU-β (β = {beta}, A = {amplitude})"),
        scheme,
        dim: n,
        dt,
        steps,
        initial_energy: h0,
        max_rel_error: max_err,
        final_rel_error: final_err.abs(),
        secular_drift,
        compensated,
        elapsed_ms,
        simd_backend: crate::simd::detected_backend().as_str().to_string(),
    }
}

/// Transporta o mapa tangente ao longo da cadeia FPU e mede volume e defeito simplético.
pub fn fpu_liouville(
    scheme: Scheme,
    n: usize,
    beta: f64,
    amplitude: f64,
    dt: f64,
    steps: usize,
) -> LiouvilleReport {
    let sys = Separable::new(
        DiagonalKinetic::uniform(n, 1.0),
        FpuChain {
            n,
            alpha: 0.0,
            beta,
        },
    );
    let (mut q, mut p) = fpu_initial_state(n, amplitude);
    let mut tangent = TangentMap::identity(n);
    let start = Instant::now();
    Integrator::new(&sys, scheme)
        .run_with_tangent(&mut q, &mut p, dt, steps, &mut tangent)
        .expect("dimensões coerentes");
    LiouvilleReport {
        system: format!("FPU-β (β = {beta}, A = {amplitude})"),
        scheme,
        dim: n,
        dt,
        steps,
        volume_error: (tangent.liouville_volume() - 1.0).abs(),
        symplectic_defect: tangent.symplectic_defect(),
        elapsed_ms: start.elapsed().as_secs_f64() * 1e3,
    }
}
