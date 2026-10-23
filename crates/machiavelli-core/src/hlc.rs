//! Relógio lógico híbrido (HLC): 48 bits de milissegundos físicos + 16 bits lógicos.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct Hlc {
    last: AtomicU64,
}

impl Hlc {
    pub const fn new() -> Self {
        Self {
            last: AtomicU64::new(0),
        }
    }

    fn physical_now() -> u64 {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        (ms & 0x0000_FFFF_FFFF_FFFF) << 16
    }

    /// Carimbo estritamente crescente enquanto houver espaço em u64.
    /// Ao atingir u64::MAX, satura nesse valor e permanece não decrescente.
    pub fn now(&self) -> u64 {
        let physical = Self::physical_now();
        let mut last = self.last.load(Ordering::Relaxed);
        loop {
            let next = if physical > last {
                physical
            } else {
                last.saturating_add(1)
            };
            match self
                .last
                .compare_exchange_weak(last, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => return next,
                Err(actual) => last = actual,
            }
        }
    }

    /// Incorpora um carimbo remoto e devolve um carimbo posterior a ambos,
    /// salvo quando o espaço de u64 se esgota: nesse caso devolve u64::MAX.
    /// Observar u64::MAX satura o relógio permanentemente.
    pub fn observe(&self, remote: u64) -> u64 {
        let physical = Self::physical_now();
        let mut last = self.last.load(Ordering::Relaxed);
        loop {
            let next = physical
                .max(remote.saturating_add(1))
                .max(last.saturating_add(1));
            match self
                .last
                .compare_exchange_weak(last, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => return next,
                Err(actual) => last = actual,
            }
        }
    }

    pub fn physical_ms(ts: u64) -> u64 {
        ts >> 16
    }

    pub fn logical(ts: u64) -> u16 {
        (ts & 0xFFFF) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictly_monotonic_under_contention() {
        let hlc = std::sync::Arc::new(Hlc::new());
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let h = hlc.clone();
                std::thread::spawn(move || (0..10_000).map(|_| h.now()).collect::<Vec<_>>())
            })
            .collect();
        let mut all: Vec<u64> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "carimbos repetidos");
    }

    #[test]
    fn now_saturates_without_wrapping() {
        let hlc = Hlc {
            last: AtomicU64::new(u64::MAX - 1),
        };
        assert_eq!(hlc.now(), u64::MAX);
        assert_eq!(hlc.now(), u64::MAX);
        assert_eq!(hlc.observe(0), u64::MAX);
    }

    #[test]
    fn observing_maximum_saturates_permanently() {
        let hlc = Hlc::new();
        assert_eq!(hlc.observe(u64::MAX), u64::MAX);
        assert_eq!(hlc.now(), u64::MAX);
        assert_eq!(hlc.observe(u64::MAX), u64::MAX);
        assert_eq!(hlc.observe(0), u64::MAX);
    }

    #[test]
    fn observe_moves_past_remote() {
        let hlc = Hlc::new();
        let remote = hlc.now() + (1 << 20);
        assert!(hlc.observe(remote) > remote);
        assert!(hlc.now() > remote);
    }
}
