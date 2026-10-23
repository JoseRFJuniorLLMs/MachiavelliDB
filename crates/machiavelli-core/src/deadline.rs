//! Orçamento de tempo das consultas PQL (SPEC-0013, execução *anytime*).

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct Deadline {
    start: Instant,
    at: Option<Instant>,
}

impl Deadline {
    /// Sem limite (uso interno e testes).
    pub fn none() -> Self {
        Self {
            start: Instant::now(),
            at: None,
        }
    }

    pub fn after(budget: Duration) -> Self {
        let start = Instant::now();
        Self {
            start,
            at: Some(start + budget),
        }
    }

    pub fn from_millis(ms: u64) -> Self {
        Self::after(Duration::from_millis(ms))
    }

    pub fn is_bounded(&self) -> bool {
        self.at.is_some()
    }

    pub fn expired(&self) -> bool {
        self.at.is_some_and(|at| Instant::now() >= at)
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.at
            .map(|at| at.saturating_duration_since(Instant::now()))
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Sub-orçamento com uma fração do tempo restante (o resto fica para os passos seguintes).
    pub fn portion(&self, fraction: f64) -> Deadline {
        match self.remaining() {
            None => Deadline::none(),
            Some(rem) => Deadline::after(rem.mul_f64(fraction.clamp(0.0, 1.0))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_and_portion() {
        let d = Deadline::from_millis(0);
        assert!(d.expired());
        assert!(!Deadline::none().expired());
        let big = Deadline::after(Duration::from_secs(10));
        let half = big.portion(0.5);
        assert!(half.remaining().unwrap() <= Duration::from_secs(5));
    }
}
