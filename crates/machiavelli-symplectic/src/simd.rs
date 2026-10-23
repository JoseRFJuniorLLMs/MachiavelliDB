//! Kernels vetorizados do colchete de Poisson e do produto interno.
//!
//! A escolha é feita em runtime: AVX-512F → AVX2+FMA → escalar em x86_64, NEON em
//! aarch64. O kernel escalar usa quatro acumuladores e é a referência para os testes.
//! Kernels diferentes somam em ordens diferentes, por isso os resultados podem diferir
//! no último ulp. Quem precisa de reprodutibilidade bit a bit deve forçar
//! [`SimdBackend::Scalar`].

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SimdBackend {
    Avx512,
    Avx2Fma,
    Neon,
    Scalar,
}

impl SimdBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            SimdBackend::Avx512 => "avx512f",
            SimdBackend::Avx2Fma => "avx2+fma",
            SimdBackend::Neon => "neon",
            SimdBackend::Scalar => "scalar",
        }
    }

    /// `true` se o processador atual suporta este backend.
    pub fn is_supported(self) -> bool {
        match self {
            SimdBackend::Scalar => true,
            #[cfg(target_arch = "x86_64")]
            SimdBackend::Avx512 => std::is_x86_feature_detected!("avx512f"),
            #[cfg(target_arch = "x86_64")]
            SimdBackend::Avx2Fma => {
                std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma")
            }
            #[cfg(target_arch = "aarch64")]
            SimdBackend::Neon => std::arch::is_aarch64_feature_detected!("neon"),
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }
}

/// Melhor backend disponível (detetado uma vez).
pub fn detected_backend() -> SimdBackend {
    static BACKEND: OnceLock<SimdBackend> = OnceLock::new();
    *BACKEND.get_or_init(|| {
        [
            SimdBackend::Avx512,
            SimdBackend::Avx2Fma,
            SimdBackend::Neon,
            SimdBackend::Scalar,
        ]
        .into_iter()
        .find(|b| b.is_supported())
        .unwrap_or(SimdBackend::Scalar)
    })
}

/// `Σ fq_i·gp_i − fp_i·gq_i` com o melhor backend.
pub fn bracket_kernel(fq: &[f64], fp: &[f64], gq: &[f64], gp: &[f64]) -> f64 {
    bracket_kernel_with(detected_backend(), fq, fp, gq, gp)
}

/// Igual a [`bracket_kernel`] com backend forçado; recua para escalar se não suportado.
pub fn bracket_kernel_with(
    backend: SimdBackend,
    fq: &[f64],
    fp: &[f64],
    gq: &[f64],
    gp: &[f64],
) -> f64 {
    let n = fq.len();
    assert!(
        fp.len() == n && gq.len() == n && gp.len() == n,
        "gradientes com dimensões diferentes"
    );
    if !backend.is_supported() {
        return bracket_scalar(fq, fp, gq, gp);
    }
    match backend {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: suporte AVX-512F verificado acima; os quatro slices têm comprimento n.
        SimdBackend::Avx512 => unsafe { x86::bracket_avx512(fq, fp, gq, gp) },
        #[cfg(target_arch = "x86_64")]
        // SAFETY: suporte AVX2+FMA verificado acima; os quatro slices têm comprimento n.
        SimdBackend::Avx2Fma => unsafe { x86::bracket_avx2(fq, fp, gq, gp) },
        #[cfg(target_arch = "aarch64")]
        // SAFETY: suporte NEON verificado acima; os quatro slices têm comprimento n.
        SimdBackend::Neon => unsafe { arm::bracket_neon(fq, fp, gq, gp) },
        _ => bracket_scalar(fq, fp, gq, gp),
    }
}

/// Produto interno `Σ a_i·b_i` (quatro acumuladores; o compilador vetoriza o laço).
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let mut acc = [0.0f64; 4];
    let chunks = a.len() / 4;
    for c in 0..chunks {
        for k in 0..4 {
            acc[k] += a[4 * c + k] * b[4 * c + k];
        }
    }
    let mut s = (acc[0] + acc[1]) + (acc[2] + acc[3]);
    for i in 4 * chunks..a.len() {
        s += a[i] * b[i];
    }
    s
}

/// Referência escalar com quatro acumuladores independentes.
pub fn bracket_scalar(fq: &[f64], fp: &[f64], gq: &[f64], gp: &[f64]) -> f64 {
    let n = fq.len();
    let mut acc = [0.0f64; 4];
    let mut i = 0;
    while i + 4 <= n {
        for k in 0..4 {
            acc[k] += fq[i + k] * gp[i + k] - fp[i + k] * gq[i + k];
        }
        i += 4;
    }
    let mut s = (acc[0] + acc[1]) + (acc[2] + acc[3]);
    while i < n {
        s += fq[i] * gp[i] - fp[i] * gq[i];
        i += 1;
    }
    s
}

#[cfg(target_arch = "x86_64")]
mod x86 {
    use std::arch::x86_64::*;

    /// # Safety
    /// O processador tem de suportar AVX-512F e os quatro slices têm o mesmo comprimento.
    #[target_feature(enable = "avx512f")]
    pub unsafe fn bracket_avx512(fq: &[f64], fp: &[f64], gq: &[f64], gp: &[f64]) -> f64 {
        let n = fq.len();
        let mut pos = _mm512_setzero_pd();
        let mut neg = _mm512_setzero_pd();
        let mut i = 0;
        while i + 8 <= n {
            // SAFETY: i + 8 <= n e todos os slices têm comprimento n.
            unsafe {
                let a = _mm512_loadu_pd(fq.as_ptr().add(i));
                let b = _mm512_loadu_pd(fp.as_ptr().add(i));
                let c = _mm512_loadu_pd(gq.as_ptr().add(i));
                let d = _mm512_loadu_pd(gp.as_ptr().add(i));
                pos = _mm512_fmadd_pd(a, d, pos);
                neg = _mm512_fmadd_pd(b, c, neg);
            }
            i += 8;
        }
        let mut s = _mm512_reduce_add_pd(pos) - _mm512_reduce_add_pd(neg);
        while i < n {
            s += fq[i] * gp[i] - fp[i] * gq[i];
            i += 1;
        }
        s
    }

    /// # Safety
    /// O processador tem de suportar AVX2 e FMA e os quatro slices têm o mesmo comprimento.
    #[target_feature(enable = "avx2,fma")]
    pub unsafe fn bracket_avx2(fq: &[f64], fp: &[f64], gq: &[f64], gp: &[f64]) -> f64 {
        let n = fq.len();
        let mut pos0 = _mm256_setzero_pd();
        let mut pos1 = _mm256_setzero_pd();
        let mut neg0 = _mm256_setzero_pd();
        let mut neg1 = _mm256_setzero_pd();
        let mut i = 0;
        while i + 8 <= n {
            // SAFETY: i + 8 <= n e todos os slices têm comprimento n.
            unsafe {
                let a0 = _mm256_loadu_pd(fq.as_ptr().add(i));
                let a1 = _mm256_loadu_pd(fq.as_ptr().add(i + 4));
                let b0 = _mm256_loadu_pd(fp.as_ptr().add(i));
                let b1 = _mm256_loadu_pd(fp.as_ptr().add(i + 4));
                let c0 = _mm256_loadu_pd(gq.as_ptr().add(i));
                let c1 = _mm256_loadu_pd(gq.as_ptr().add(i + 4));
                let d0 = _mm256_loadu_pd(gp.as_ptr().add(i));
                let d1 = _mm256_loadu_pd(gp.as_ptr().add(i + 4));
                pos0 = _mm256_fmadd_pd(a0, d0, pos0);
                pos1 = _mm256_fmadd_pd(a1, d1, pos1);
                neg0 = _mm256_fmadd_pd(b0, c0, neg0);
                neg1 = _mm256_fmadd_pd(b1, c1, neg1);
            }
            i += 8;
        }
        let diff = _mm256_sub_pd(_mm256_add_pd(pos0, pos1), _mm256_add_pd(neg0, neg1));
        let mut lanes = [0.0f64; 4];
        // SAFETY: `lanes` tem exatamente 4 f64.
        unsafe { _mm256_storeu_pd(lanes.as_mut_ptr(), diff) };
        let mut s = (lanes[0] + lanes[1]) + (lanes[2] + lanes[3]);
        while i < n {
            s += fq[i] * gp[i] - fp[i] * gq[i];
            i += 1;
        }
        s
    }
}

#[cfg(target_arch = "aarch64")]
mod arm {
    use std::arch::aarch64::*;

    /// # Safety
    /// O processador tem de suportar NEON e os quatro slices têm o mesmo comprimento.
    #[target_feature(enable = "neon")]
    pub unsafe fn bracket_neon(fq: &[f64], fp: &[f64], gq: &[f64], gp: &[f64]) -> f64 {
        let n = fq.len();
        let mut pos = vdupq_n_f64(0.0);
        let mut neg = vdupq_n_f64(0.0);
        let mut i = 0;
        while i + 2 <= n {
            // SAFETY: i + 2 <= n e todos os slices têm comprimento n.
            unsafe {
                let a = vld1q_f64(fq.as_ptr().add(i));
                let b = vld1q_f64(fp.as_ptr().add(i));
                let c = vld1q_f64(gq.as_ptr().add(i));
                let d = vld1q_f64(gp.as_ptr().add(i));
                pos = vfmaq_f64(pos, a, d);
                neg = vfmaq_f64(neg, b, c);
            }
            i += 2;
        }
        let mut s = vaddvq_f64(pos) - vaddvq_f64(neg);
        while i < n {
            s += fq[i] * gp[i] - fp[i] * gq[i];
            i += 1;
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(n: usize, seed: u64) -> Vec<f64> {
        let mut x = seed;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x % 10_000) as f64 / 5_000.0 - 1.0
            })
            .collect()
    }

    #[test]
    fn every_supported_backend_matches_scalar() {
        for n in [0, 1, 3, 7, 8, 9, 64, 128, 1001] {
            let (a, b, c, d) = (data(n, 1), data(n, 2), data(n, 3), data(n, 4));
            let reference = bracket_scalar(&a, &b, &c, &d);
            for backend in [
                SimdBackend::Avx512,
                SimdBackend::Avx2Fma,
                SimdBackend::Neon,
                SimdBackend::Scalar,
            ] {
                let got = bracket_kernel_with(backend, &a, &b, &c, &d);
                assert!(
                    (got - reference).abs() <= 1e-12 * (1.0 + reference.abs()),
                    "{backend:?} n={n}: {got} vs {reference}"
                );
            }
        }
    }

    #[test]
    fn dot_matches_naive() {
        let a = data(100, 9);
        let b = data(100, 10);
        let naive: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        assert!((dot(&a, &b) - naive).abs() < 1e-12);
    }
}
