//! Identificadores de 16 bytes (UUID) e de 32 bytes (BLAKE3).

use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

macro_rules! uuid_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
            rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
        )]
        #[rkyv(derive(Debug, PartialEq, Eq, Hash))]
        #[repr(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub const NIL: Self = Self(Uuid::nil());

            /// Novo identificador ordenável por tempo (UUIDv7).
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(Uuid::from_bytes(bytes))
            }

            pub fn as_bytes(&self) -> &[u8; 16] {
                self.0.as_bytes()
            }

            pub fn is_nil(&self) -> bool {
                self.0.is_nil()
            }

            /// Identificador determinístico derivado de um nome (BLAKE3 truncado, versão 8).
            pub fn derive(namespace: &str, name: &str) -> Self {
                let mut hasher = blake3::Hasher::new();
                hasher.update(namespace.as_bytes());
                hasher.update(&[0]);
                hasher.update(name.as_bytes());
                let digest = hasher.finalize();
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&digest.as_bytes()[..16]);
                Self(uuid::Builder::from_custom_bytes(bytes).into_uuid())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = CoreError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|e| CoreError::InvalidId(format!("{s}: {e}")))
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
    };
}

uuid_id!(
    /// Identidade de um estado `Continentia` (chave de 16 bytes nas column families).
    NodeId
);
uuid_id!(
    /// Identidade de uma aresta estratégica / matriz de payoff.
    EdgeId
);
uuid_id!(
    /// Identidade de um subgrafo, agente ou tenant na economia VCG.
    SubgraphId
);
uuid_id!(
    /// Identidade de uma invariante de Casimir (`cf_invariants`).
    InvariantId
);

macro_rules! hash_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
            rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
        )]
        #[rkyv(derive(Debug, PartialEq, Eq, Hash))]
        #[repr(transparent)]
        pub struct $name(pub [u8; 32]);

        impl $name {
            /// BLAKE3 de uma chave canónica.
            pub fn of(bytes: &[u8]) -> Self {
                Self(*blake3::hash(bytes).as_bytes())
            }

            pub fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            pub fn to_hex(&self) -> String {
                hex::encode(self.0)
            }

            pub fn from_hex(s: &str) -> Result<Self, CoreError> {
                let raw = hex::decode(s).map_err(|e| CoreError::InvalidId(e.to_string()))?;
                let arr: [u8; 32] = raw
                    .try_into()
                    .map_err(|_| CoreError::InvalidId(format!("{s}: esperado 32 bytes")))?;
                Ok(Self(arr))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({}…)", stringify!($name), &self.to_hex()[..12])
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_hex())
            }
        }
    };
}

hash_id!(
    /// Hash de 32 bytes de um conjunto de informação (`cf_infosets`, `cf_regret_tables`).
    InfosetHash
);
hash_id!(
    /// Hash de 32 bytes do predicado de disparo de um Dique (`cf_dikes`).
    TriggerHash
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic_and_namespaced() {
        let a = NodeId::derive("infra_core", "auth_citadel");
        let b = NodeId::derive("infra_core", "auth_citadel");
        let c = NodeId::derive("other", "auth_citadel");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn uuid_roundtrip_text() {
        let id = NodeId::new();
        let parsed: NodeId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn hash_hex_roundtrip() {
        let h = InfosetHash::of(b"fraud_ring_alpha");
        assert_eq!(InfosetHash::from_hex(&h.to_hex()).unwrap(), h);
    }
}
