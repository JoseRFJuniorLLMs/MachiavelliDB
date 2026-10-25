//! A deterministic particle prediction/observation/resampling cycle.
use machiavelli_core::{Deadline, PhaseVector};
use machiavelli_fog::ParticleFilter;
use machiavelli_symplectic::{DiagonalKinetic, Scheme, Separable, systems::Harmonic};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let particles = (-8..=8)
        .map(|i| PhaseVector::with_constitution(vec![i as f64 / 8.0], vec![0.2], vec![7.0]))
        .collect::<Result<Vec<_>, _>>()?;
    let mut filter = ParticleFilter::new(particles, &[1.0; 17], 42)?;
    let system = Separable::new(
        DiagonalKinetic::uniform(1, 1.0),
        Harmonic::isotropic(1, 1.0),
    );
    if !filter.predict(
        &system,
        Scheme::Yoshida4,
        0.01,
        100,
        &Deadline::from_millis(1000),
    )? {
        return Err("prediction deadline".into());
    }
    filter.observe_position(&[0.3], 0.1)?;
    let ess = filter.effective_sample_size();
    let before = filter.mean_phase()?;
    let resampled = filter.resample_if_needed(0.75)?;
    println!(
        "particles={} ESS_before={:.6} ESS_after={:.6} resampled={} posterior_q={:?} Casimir={:?}",
        filter.particles().len(),
        ess,
        filter.effective_sample_size(),
        resampled,
        before.q(),
        before.c()
    );
    Ok(())
}
