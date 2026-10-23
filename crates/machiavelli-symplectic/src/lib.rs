//! # machiavelli-symplectic — SPEC-0001
//!
//! Kernel simplético do MachiavelliDB:
//!
//! - [`form`]: matriz canónica `J`, forma `ω(u, v) = uᵀ J v`, geradores de matrizes simpléticas.
//! - [`poisson`]: colchete de Poisson `{f, g}` com kernels AVX-512 / AVX2+FMA / NEON.
//! - [`system`]: potenciais, energia cinética diagonal `½ pᵀM⁻¹p` e Hamiltonianos.
//! - [`integrators`]: Störmer-Verlet (2ª ordem), Yoshida (4ª ordem) e leapfrog
//!   generalizado (implícito) para Hamiltonianos não separáveis.
//! - [`tangent`]: propagação do mapa tangente para medir o volume de Liouville
//!   (`det Φ = 1`) e o defeito simplético (`ΦᵀJΦ = J`).
//!
//! Por exigência da SPEC-0001 não há Euler explícito nem RK4 neste crate: ambos não são
//! simpléticos e produzem deriva secular de energia.

pub mod form;
pub mod integrators;
pub mod linalg;
pub mod poisson;
pub mod qualification;
pub mod simd;
pub mod system;
pub mod systems;
pub mod tangent;

pub use form::{canonical_j, is_symplectic, omega, symplectic_defect};
pub use integrators::{GeneralizedLeapfrog, Integrator, IntegratorError, Scheme};
pub use linalg::Matrix;
pub use poisson::{PhaseFunction, poisson_bracket, poisson_bracket_of};
pub use simd::SimdBackend;
pub use system::{DiagonalKinetic, Hamiltonian, Potential, Separable};
pub use tangent::TangentMap;
