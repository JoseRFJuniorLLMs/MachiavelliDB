//! `ContinentiaMeta`: os 128 bytes de metadados táticos de cada estado (secção 4 do README).

use crate::error::CoreError;
use crate::ids::NodeId;
use serde::{Deserialize, Serialize};
use std::mem::{align_of, offset_of, size_of};
use uuid::Uuid;

/// Postura estratégica de um nó (1 byte).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum StrategicPosture {
    /// Nó em expansão: postura normal, aceita mutações.
    #[default]
    Expanding = 0,
    /// Cidadela (SPEC-0010): mutações exigem quórum BFT e massa ×γ.
    Fortified = 1,
    /// Ativo descartável (batedor/scout): massa leve, candidato a `BETRAY`.
    Expendable = 2,
    /// Isca da Operação Mandragola (SPEC-0011).
    Decoy = 3,
    /// Amputado por `BETRAY` (SPEC-0009): congelado e isolado.
    Betrayed = 4,
}

impl StrategicPosture {
    pub const ALL: [StrategicPosture; 5] = [
        StrategicPosture::Expanding,
        StrategicPosture::Fortified,
        StrategicPosture::Expendable,
        StrategicPosture::Decoy,
        StrategicPosture::Betrayed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            StrategicPosture::Expanding => "expanding",
            StrategicPosture::Fortified => "fortified",
            StrategicPosture::Expendable => "expendable",
            StrategicPosture::Decoy => "decoy",
            StrategicPosture::Betrayed => "betrayed",
        }
    }
}

impl TryFrom<u8> for StrategicPosture {
    type Error = CoreError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => StrategicPosture::Expanding,
            1 => StrategicPosture::Fortified,
            2 => StrategicPosture::Expendable,
            3 => StrategicPosture::Decoy,
            4 => StrategicPosture::Betrayed,
            other => return Err(CoreError::UnknownPosture(other)),
        })
    }
}

/// Metadados leves de estado, exatamente 128 bytes (duas cache lines de 64 bytes).
///
/// A disposição segue a secção 4 do README campo a campo. O `repr(C)` insere 4 bytes de
/// alinhamento antes de `hamiltonian_energy` (offset 28 → 32); aqui esse alinhamento é o
/// campo explícito `_pad_energy`, para que a codificação nunca leia bytes não inicializados.
/// Com ele, os 24 bytes de `_reserved` fecham os 128 bytes como a SPEC indica.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContinentiaMeta {
    /// 16B — identidade única do estado.
    pub id: NodeId,
    /// 4B — `[0, 1]`: capacidade de execução e precisão histórica.
    pub virtu: f32,
    /// 4B — `[0, 1]`: vulnerabilidade a choques exógenos.
    pub fortuna_exposure: f32,
    /// 4B — inércia (resistência a mudanças bruscas de momento), `> 0`.
    pub strategic_mass: f32,
    /// 4B — alinhamento explícito de `hamiltonian_energy`.
    #[serde(skip)]
    pub _pad_energy: [u8; 4],
    /// 8B — `H(q, p)` instantâneo.
    pub hamiltonian_energy: f64,
    /// 4B — probabilidade de este estado ser alcançado no Equilíbrio de Nash.
    pub nash_reach_prob: f32,
    /// 4B — arrependimento contrafactual acumulado.
    pub cumulative_regret: f32,
    /// 8B — créditos de Virtù para o leilão VCG.
    pub vcg_credit_balance: f64,
    /// 1B — postura estratégica.
    pub posture: StrategicPosture,
    /// 1B — opacidade de informação (0 = visível, 255 = neblina total).
    pub fog_level: u8,
    /// 2B — coalizão atual.
    pub alliance_cluster: u16,
    /// 4B — diques anexados.
    pub dike_count: u32,
    /// 16B — origem conceitual no NietzscheDB (nil se não houver).
    pub parent_nietzsche_id: Uuid,
    /// 8B — último LSN auditado no HeraclitusDB.
    pub last_heraclitus_lsn: u64,
    /// 8B — carimbo HLC de criação.
    pub created_at_hlc: u64,
    /// 8B — tempo limite antes de o nó ser considerado passivo descartável.
    pub betrayal_horizon_secs: u64,
    /// 24B — reservado (zeros).
    #[serde(skip)]
    pub _reserved: [u8; 24],
}

const _: () = assert!(size_of::<ContinentiaMeta>() == 128);
const _: () = assert!(align_of::<ContinentiaMeta>() == 64);
const _: () = assert!(offset_of!(ContinentiaMeta, hamiltonian_energy) == 32);
const _: () = assert!(offset_of!(ContinentiaMeta, posture) == 56);
const _: () = assert!(offset_of!(ContinentiaMeta, parent_nietzsche_id) == 64);
const _: () = assert!(offset_of!(ContinentiaMeta, _reserved) == 104);

impl ContinentiaMeta {
    /// Tamanho da codificação binária (igual ao tamanho em memória).
    pub const ENCODED_LEN: usize = 128;

    /// Metadados neutros para um nó novo.
    pub fn new(id: NodeId, created_at_hlc: u64) -> Self {
        Self {
            id,
            virtu: 0.5,
            fortuna_exposure: 0.0,
            strategic_mass: 1.0,
            _pad_energy: [0; 4],
            hamiltonian_energy: 0.0,
            nash_reach_prob: 0.0,
            cumulative_regret: 0.0,
            vcg_credit_balance: 0.0,
            posture: StrategicPosture::Expanding,
            fog_level: 0,
            alliance_cluster: 0,
            dike_count: 0,
            parent_nietzsche_id: Uuid::nil(),
            last_heraclitus_lsn: 0,
            created_at_hlc,
            betrayal_horizon_secs: 3_600,
            _reserved: [0; 24],
        }
    }

    /// Verifica os domínios declarados na secção 4.
    pub fn validate(&self) -> Result<(), CoreError> {
        check_unit("virtu", self.virtu as f64)?;
        check_unit("fortuna_exposure", self.fortuna_exposure as f64)?;
        check_unit("nash_reach_prob", self.nash_reach_prob as f64)?;
        if !(self.strategic_mass.is_finite() && self.strategic_mass > 0.0) {
            return Err(CoreError::OutOfDomain {
                field: "strategic_mass",
                value: self.strategic_mass as f64,
                domain: "(0, +inf)",
            });
        }
        if !self.hamiltonian_energy.is_finite() {
            return Err(CoreError::OutOfDomain {
                field: "hamiltonian_energy",
                value: self.hamiltonian_energy,
                domain: "finito",
            });
        }
        if !self.vcg_credit_balance.is_finite() || self.vcg_credit_balance < 0.0 {
            return Err(CoreError::OutOfDomain {
                field: "vcg_credit_balance",
                value: self.vcg_credit_balance,
                domain: "[0, +inf)",
            });
        }
        if !self.cumulative_regret.is_finite() {
            return Err(CoreError::OutOfDomain {
                field: "cumulative_regret",
                value: self.cumulative_regret as f64,
                domain: "finito",
            });
        }
        Ok(())
    }

    /// Fração de neblina em `[0, 1]`.
    pub fn fog_fraction(&self) -> f64 {
        self.fog_level as f64 / 255.0
    }

    /// Codificação little-endian com os offsets exatos do layout `repr(C)`.
    pub fn encode(&self) -> [u8; 128] {
        let mut out = [0u8; 128];
        out[0..16].copy_from_slice(self.id.as_bytes());
        out[16..20].copy_from_slice(&self.virtu.to_le_bytes());
        out[20..24].copy_from_slice(&self.fortuna_exposure.to_le_bytes());
        out[24..28].copy_from_slice(&self.strategic_mass.to_le_bytes());
        // 28..32: alinhamento (zeros)
        out[32..40].copy_from_slice(&self.hamiltonian_energy.to_le_bytes());
        out[40..44].copy_from_slice(&self.nash_reach_prob.to_le_bytes());
        out[44..48].copy_from_slice(&self.cumulative_regret.to_le_bytes());
        out[48..56].copy_from_slice(&self.vcg_credit_balance.to_le_bytes());
        out[56] = self.posture as u8;
        out[57] = self.fog_level;
        out[58..60].copy_from_slice(&self.alliance_cluster.to_le_bytes());
        out[60..64].copy_from_slice(&self.dike_count.to_le_bytes());
        out[64..80].copy_from_slice(self.parent_nietzsche_id.as_bytes());
        out[80..88].copy_from_slice(&self.last_heraclitus_lsn.to_le_bytes());
        out[88..96].copy_from_slice(&self.created_at_hlc.to_le_bytes());
        out[96..104].copy_from_slice(&self.betrayal_horizon_secs.to_le_bytes());
        // 104..128: reservado (zeros)
        out
    }

    /// Descodifica e valida (postura conhecida, domínios numéricos).
    pub fn decode(bytes: &[u8]) -> Result<Self, CoreError> {
        if bytes.len() != Self::ENCODED_LEN {
            return Err(CoreError::InvalidLength {
                expected: Self::ENCODED_LEN,
                actual: bytes.len(),
            });
        }
        let f32_at = |o: usize| f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let f64_at = |o: usize| f64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());
        let u64_at = |o: usize| u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());
        let meta = Self {
            id: NodeId::from_bytes(bytes[0..16].try_into().unwrap()),
            virtu: f32_at(16),
            fortuna_exposure: f32_at(20),
            strategic_mass: f32_at(24),
            _pad_energy: [0; 4],
            hamiltonian_energy: f64_at(32),
            nash_reach_prob: f32_at(40),
            cumulative_regret: f32_at(44),
            vcg_credit_balance: f64_at(48),
            posture: StrategicPosture::try_from(bytes[56])?,
            fog_level: bytes[57],
            alliance_cluster: u16::from_le_bytes([bytes[58], bytes[59]]),
            dike_count: u32::from_le_bytes(bytes[60..64].try_into().unwrap()),
            parent_nietzsche_id: Uuid::from_bytes(bytes[64..80].try_into().unwrap()),
            last_heraclitus_lsn: u64_at(80),
            created_at_hlc: u64_at(88),
            betrayal_horizon_secs: u64_at(96),
            _reserved: [0; 24],
        };
        meta.validate()?;
        Ok(meta)
    }
}

fn check_unit(field: &'static str, value: f64) -> Result<(), CoreError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(CoreError::OutOfDomain {
            field,
            value,
            domain: "[0, 1]",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_exactly_two_cache_lines() {
        assert_eq!(size_of::<ContinentiaMeta>(), 128);
        assert_eq!(align_of::<ContinentiaMeta>(), 64);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let mut m = ContinentiaMeta::new(NodeId::new(), 42);
        m.virtu = 0.75;
        m.fortuna_exposure = 0.2;
        m.strategic_mass = 3.5;
        m.hamiltonian_energy = -12.25;
        m.posture = StrategicPosture::Fortified;
        m.fog_level = 200;
        m.alliance_cluster = 7;
        m.dike_count = 3;
        m.vcg_credit_balance = 1000.0;
        m.last_heraclitus_lsn = 99;
        let bytes = m.encode();
        assert_eq!(ContinentiaMeta::decode(&bytes).unwrap(), m);
    }

    #[test]
    fn decode_rejects_unknown_posture_and_bad_domains() {
        let m = ContinentiaMeta::new(NodeId::new(), 1);
        let mut bytes = m.encode();
        bytes[56] = 9;
        assert!(matches!(
            ContinentiaMeta::decode(&bytes),
            Err(CoreError::UnknownPosture(9))
        ));
        let mut bytes = m.encode();
        bytes[16..20].copy_from_slice(&1.5f32.to_le_bytes());
        assert!(matches!(
            ContinentiaMeta::decode(&bytes),
            Err(CoreError::OutOfDomain { field: "virtu", .. })
        ));
        assert!(ContinentiaMeta::decode(&bytes[..10]).is_err());
    }
}
