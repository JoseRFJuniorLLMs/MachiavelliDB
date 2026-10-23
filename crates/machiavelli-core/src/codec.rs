//! Envelope `rkyv` comum. Os bytes que chegam do RocksDB não têm alinhamento garantido,
//! por isso a leitura copia primeiro para um `AlignedVec` antes de validar.

use crate::error::CoreError;
use rkyv::api::high::{HighDeserializer, HighSerializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::Error as RkyvError;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;

/// Serializa um valor para bytes `rkyv`.
pub fn to_rkyv<T>(value: &T) -> Result<Vec<u8>, CoreError>
where
    T: for<'a> rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, RkyvError>>,
{
    rkyv::to_bytes::<RkyvError>(value)
        .map(|v| v.to_vec())
        .map_err(|e| CoreError::Codec(e.to_string()))
}

/// Valida e desserializa bytes `rkyv` (aceita buffers desalinhados).
pub fn from_rkyv<T>(bytes: &[u8]) -> Result<T, CoreError>
where
    T: rkyv::Archive,
    T::Archived: for<'a> CheckBytes<HighValidator<'a, RkyvError>>
        + rkyv::Deserialize<T, HighDeserializer<RkyvError>>,
{
    let mut aligned = AlignedVec::<16>::with_capacity(bytes.len());
    aligned.extend_from_slice(bytes);
    rkyv::from_bytes::<T, RkyvError>(&aligned).map_err(|e| CoreError::Codec(e.to_string()))
}
