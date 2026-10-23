//! Escalares táticos limitados a `[0, 1]`.

use serde::{Deserialize, Serialize};

macro_rules! unit_score {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
        #[repr(transparent)]
        pub struct $name(f32);

        impl $name {
            pub const ZERO: Self = Self(0.0);
            pub const ONE: Self = Self(1.0);

            /// Satura em `[0, 1]`; `NaN` vira 0.
            pub fn saturating(value: f64) -> Self {
                if value.is_nan() {
                    Self(0.0)
                } else {
                    Self(value.clamp(0.0, 1.0) as f32)
                }
            }

            /// Construtor estrito.
            pub fn new(value: f64) -> Option<Self> {
                (value.is_finite() && (0.0..=1.0).contains(&value)).then(|| Self(value as f32))
            }

            pub fn get(self) -> f64 {
                self.0 as f64
            }
        }
    };
}

unit_score!(
    /// Capacidade de execução interna e precisão histórica.
    VirtuScore
);
unit_score!(
    /// Exposição a choques exógenos.
    FortunaRisk
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturates_and_rejects() {
        assert_eq!(VirtuScore::saturating(2.0).get(), 1.0);
        assert_eq!(FortunaRisk::saturating(f64::NAN).get(), 0.0);
        assert!(VirtuScore::new(-0.1).is_none());
        assert_eq!(VirtuScore::new(0.25).unwrap().get(), 0.25);
    }
}
