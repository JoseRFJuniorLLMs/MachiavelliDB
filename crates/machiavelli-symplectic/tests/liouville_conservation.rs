//! Invariante de qualificação da SPEC-0001: depois de 10⁵ passos num sistema fechado,
//! `|ΔH/H₀| < 10⁻⁹` e o volume de Liouville conservado.

use machiavelli_symplectic::qualification::{fpu_conservation, fpu_liouville};
use machiavelli_symplectic::systems::{Harmonic, HenonHeiles};
use machiavelli_symplectic::{DiagonalKinetic, Integrator, Scheme, Separable, TangentMap};

const STEPS: usize = 100_000;
const LIMIT: f64 = 1e-9;

#[test]
fn energy_yoshida4_fpu_chain_n64() {
    let r = fpu_conservation(Scheme::Yoshida4, 64, 0.25, 1.0, 1e-3, STEPS, false);
    println!("{r:#?}");
    assert!(r.max_rel_error < LIMIT, "|ΔH/H₀| = {:e}", r.max_rel_error);
    assert!(r.secular_drift < LIMIT);
}

#[test]
fn energy_stormer_verlet_fpu_chain_n64() {
    let r = fpu_conservation(Scheme::StormerVerlet, 64, 0.25, 1.0, 2e-5, STEPS, false);
    println!("{r:#?}");
    assert!(r.max_rel_error < LIMIT, "|ΔH/H₀| = {:e}", r.max_rel_error);
    assert!(r.secular_drift < LIMIT);
}

#[test]
fn energy_yoshida4_fpu_chain_n128() {
    let r = fpu_conservation(Scheme::Yoshida4, 128, 0.25, 1.0, 1e-3, STEPS, true);
    println!("{r:#?}");
    assert!(r.max_rel_error < LIMIT, "|ΔH/H₀| = {:e}", r.max_rel_error);
}

#[test]
fn liouville_volume_fpu_chain() {
    for scheme in [Scheme::StormerVerlet, Scheme::Yoshida4] {
        let r = fpu_liouville(scheme, 8, 0.25, 0.3, 1e-2, STEPS);
        println!("{r:#?}");
        assert!(
            r.volume_error < LIMIT,
            "{scheme:?}: |det Φ − 1| = {:e}",
            r.volume_error
        );
        assert!(
            r.symplectic_defect < LIMIT,
            "{scheme:?}: defeito simplético {:e}",
            r.symplectic_defect
        );
    }
}

#[test]
fn liouville_volume_henon_heiles_regular_orbit() {
    let sys = Separable::new(DiagonalKinetic::uniform(2, 1.0), HenonHeiles);
    let (mut q, mut p) = ([0.0, 0.1], [0.25, 0.0]);
    let mut tangent = TangentMap::identity(2);
    Integrator::new(&sys, Scheme::Yoshida4)
        .run_with_tangent(&mut q, &mut p, 1e-2, STEPS, &mut tangent)
        .unwrap();
    let vol = (tangent.liouville_volume() - 1.0).abs();
    let defect = tangent.symplectic_defect();
    println!("Hénon–Heiles: |det Φ − 1| = {vol:e}, defeito = {defect:e}");
    assert!(vol < LIMIT && defect < LIMIT);
}

/// Controlo negativo: o Euler explícito (proibido pela SPEC-0001) viola o mesmo critério
/// por várias ordens de grandeza. Garante que o teste distingue os esquemas.
#[test]
fn explicit_euler_fails_the_same_criterion() {
    let sys = Separable::new(
        DiagonalKinetic::uniform(1, 1.0),
        Harmonic::isotropic(1, 1.0),
    );
    let (mut q, mut p) = (1.0f64, 0.0f64);
    let h0 = sys.energy(&[q], &[p]);
    let dt = 1e-3;
    for _ in 0..STEPS {
        let (nq, np) = (q + dt * p, p - dt * q);
        q = nq;
        p = np;
    }
    let err = (sys.energy(&[q], &[p]) - h0).abs() / h0;
    println!("Euler explícito: |ΔH/H₀| = {err:e}");
    assert!(err > 1e3 * LIMIT);
}

#[test]
fn m1_million_verlet_steps_fpu_n64() {
    let report = fpu_conservation(Scheme::StormerVerlet, 64, 0.25, 1.0, 2e-5, 1_000_000, false);
    assert!(
        report.max_rel_error < 1e-9,
        "M1 maximum relative error {:e}",
        report.max_rel_error
    );
    assert!(report.secular_drift < 1e-9);
}
