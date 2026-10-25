//! Explicit native CUDA ABI. Library path comes from trusted operator config.
//! No automatic fallback can report a CPU result as GPU execution.
use std::{ffi::CString, path::Path};
struct Library(*mut libc::c_void);
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: handle is owned, remains open through the FFI call, closes once.
        unsafe {
            libc::dlclose(self.0);
        }
    }
}
type Verlet =
    unsafe extern "C" fn(*mut f64, *mut f64, *const f64, *const f64, u64, u32, f64, u32) -> i32;
pub fn verlet_batch(
    library: &Path,
    q: &mut [f64],
    p: &mut [f64],
    masses: &[f64],
    stiffness: &[f64],
    dt: f64,
    steps: usize,
) -> Result<(), String> {
    if steps > 1_000_000 || masses.len() > u32::MAX as usize {
        return Err("CUDA resource bounds exceeded".into());
    }
    let (mut next_q, mut next_p) = (q.to_vec(), p.to_vec());
    // Shared validation, including finite phase/parameters, without advancing CPU.
    super::verlet_batch(&mut next_q, &mut next_p, masses, stiffness, dt, 0)?;
    let name = CString::new(library.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
    // SAFETY: NUL-terminated path, flags defined by the platform; null is checked.
    let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        return Err(
            "CUDA native library unavailable; run scripts/cuda.sh with CUDA toolkit and GPU".into(),
        );
    }
    let handle = Library(handle);
    // SAFETY: symbols are checked, kept alive by handle; this is a trusted ABI v1
    // library built from this repository. Arbitrary libraries are executable code.
    let (version, kernel) = unsafe {
        (
            libc::dlsym(handle.0, c"machiavelli_cuda_abi_version".as_ptr()),
            libc::dlsym(handle.0, c"machiavelli_cuda_verlet".as_ptr()),
        )
    };
    if version.is_null() || kernel.is_null() {
        return Err("CUDA ABI symbols missing".into());
    }
    // SAFETY: exact signatures exported by cuda/host.cu, version checked first.
    let version: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(version) };
    if unsafe { version() } != 1 {
        return Err("unsupported CUDA ABI".into());
    }
    // SAFETY: exact ABI after version verification; all slices validated above.
    let kernel: Verlet = unsafe { std::mem::transmute(kernel) };
    let code = unsafe {
        kernel(
            next_q.as_mut_ptr(),
            next_p.as_mut_ptr(),
            masses.as_ptr(),
            stiffness.as_ptr(),
            q.len() as u64,
            masses.len() as u32,
            dt,
            steps as u32,
        )
    };
    if code != 0 {
        return Err(format!(
            "CUDA execution failed ({code}); no local output published"
        ));
    }
    if next_q.iter().chain(&next_p).any(|v| !v.is_finite()) {
        return Err("non-finite CUDA result".into());
    }
    q.copy_from_slice(&next_q);
    p.copy_from_slice(&next_p);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_cuda_library_fails_without_mutating_input() {
        let (mut q, mut p) = (vec![1.0], vec![0.0]);
        assert!(
            verlet_batch(
                Path::new("/nonexistent/machiavelli_cuda.so"),
                &mut q,
                &mut p,
                &[1.0],
                &[1.0],
                0.01,
                1
            )
            .is_err()
        );
        assert_eq!(q, vec![1.0]);
        assert_eq!(p, vec![0.0]);
    }
    #[test]
    #[ignore = "requires native CUDA library and a real Linux GPU"]
    fn cuda_matches_cpu_on_hardware() {
        let library = std::env::var("MACHIAVELLI_CUDA_LIBRARY").expect("set native library path");
        let (mut q, mut p) = (vec![0.1; 384], vec![0.3; 384]);
        let (mut cpu_q, mut cpu_p) = (q.clone(), p.clone());
        let mass = [1.0, 2.0, 3.0];
        let stiffness = [0.5, 1.0, 2.0];
        super::super::verlet_batch(&mut cpu_q, &mut cpu_p, &mass, &stiffness, 0.001, 1000).unwrap();
        verlet_batch(
            Path::new(&library),
            &mut q,
            &mut p,
            &mass,
            &stiffness,
            0.001,
            1000,
        )
        .unwrap();
        for (gpu, cpu) in q.iter().chain(&p).zip(cpu_q.iter().chain(&cpu_p)) {
            assert!((gpu - cpu).abs() < 1e-10);
        }
    }
}
