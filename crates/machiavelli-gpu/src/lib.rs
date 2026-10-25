//! Deterministic CPU batch fallback and CUDA reference kernels.
//! CUDA is an explicit Linux feature loading a separately compiled native library.
#[cfg(all(feature = "cuda", target_os = "linux"))]
pub mod cuda;
use machiavelli_symplectic::{DiagonalKinetic, Integrator, Scheme, Separable, systems::Harmonic};
pub const CUDA_SOURCE: &str = include_str!("../cuda/kernels.cu");
pub fn verlet_batch(
    q: &mut [f64],
    p: &mut [f64],
    masses: &[f64],
    stiffness: &[f64],
    dt: f64,
    steps: usize,
) -> Result<(), String> {
    let n = masses.len();
    if n == 0
        || stiffness.len() != n
        || q.len() != p.len()
        || !q.len().is_multiple_of(n)
        || stiffness.iter().any(|k| !k.is_finite() || *k < 0.0)
    {
        return Err("invalid batch dimensions/parameters".into());
    }
    let kinetic = DiagonalKinetic::from_masses(masses).map_err(|e| e.to_string())?;
    let sys = Separable::new(
        kinetic,
        Harmonic {
            stiffness: stiffness.to_vec(),
            center: vec![0.0; n],
        },
    );
    let (mut next_q, mut next_p) = (q.to_vec(), p.to_vec());
    for (q, p) in next_q.chunks_mut(n).zip(next_p.chunks_mut(n)) {
        Integrator::new(&sys, Scheme::StormerVerlet)
            .run(q, p, dt, steps)
            .map_err(|e| e.to_string())?;
    }
    q.copy_from_slice(&next_q);
    p.copy_from_slice(&next_p);
    Ok(())
}
pub fn regret_matching_plus(regrets: &mut [f64], delta: &[f64]) -> Result<Vec<f64>, &'static str> {
    if regrets.is_empty()
        || regrets.len() != delta.len()
        || regrets.iter().chain(delta).any(|v| !v.is_finite())
    {
        return Err("invalid regrets");
    }
    let next: Vec<_> = regrets
        .iter()
        .zip(delta)
        .map(|(r, d)| (r + d).max(0.0))
        .collect();
    if next.iter().any(|v| !v.is_finite()) {
        return Err("regret overflow");
    }
    let maximum = next.iter().copied().fold(0.0_f64, f64::max);
    let strategy = if maximum == 0.0 {
        vec![1.0 / next.len() as f64; next.len()]
    } else {
        let sum: f64 = next.iter().map(|v| v / maximum).sum();
        next.iter().map(|v| (v / maximum) / sum).collect()
    };
    regrets.copy_from_slice(&next);
    Ok(strategy)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_matches_individual_reference_and_is_transactional() {
        let (mut q, mut p) = (vec![1.0, 2.0], vec![0.0, 0.1]);
        verlet_batch(&mut q, &mut p, &[1.0], &[2.0], 0.01, 100).unwrap();
        let sys = Separable::new(
            DiagonalKinetic::uniform(1, 1.0),
            Harmonic::isotropic(1, 2.0),
        );
        let (mut rq, mut rp) = ([2.0], [0.1]);
        Integrator::new(&sys, Scheme::StormerVerlet)
            .run(&mut rq, &mut rp, 0.01, 100)
            .unwrap();
        assert_eq!(q[1], rq[0]);
        assert_eq!(p[1], rp[0]);
        let original = q.clone();
        assert!(verlet_batch(&mut q, &mut p, &[1.0], &[2.0], f64::NAN, 1).is_err());
        assert_eq!(q, original);
        let mut regrets = [-2.0, 3.0];
        assert_eq!(
            regret_matching_plus(&mut regrets, &[1.0, 1.0]).unwrap(),
            vec![0.0, 1.0]
        );
    }
}
