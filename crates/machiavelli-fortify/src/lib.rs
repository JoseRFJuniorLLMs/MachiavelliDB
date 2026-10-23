//! Software Ed25519 quorum verification. This is not hardware-backed MinBFT.
use ed25519_dalek::{Signature, VerifyingKey};
use std::collections::BTreeSet;
#[derive(Clone, Debug)]
pub struct Vote {
    pub signer: [u8; 32],
    pub signature: [u8; 64],
}
pub struct Quorum {
    keys: BTreeSet<[u8; 32]>,
    faults: usize,
    epoch: u64,
}
pub struct Verified {
    digest: [u8; 32],
    sequence: u64,
    epoch: u64,
}
impl Verified {
    pub fn matches(&self, digest: [u8; 32], sequence: u64, epoch: u64) -> bool {
        self.digest == digest && self.sequence == sequence && self.epoch == epoch
    }
}
pub fn signing_message(digest: [u8; 32], sequence: u64, epoch: u64, expires_ms: u64) -> Vec<u8> {
    let mut message = b"MachiavelliDB/quorum/v1".to_vec();
    message.extend_from_slice(&digest);
    for word in [sequence, epoch, expires_ms] {
        message.extend_from_slice(&word.to_le_bytes());
    }
    message
}
impl Quorum {
    /// Standard software BFT configuration: n = 3f+1, quorum 2f+1.
    /// Keys and epoch come from a trusted administrative configuration.
    pub fn new(keys: Vec<[u8; 32]>, faults: usize, epoch: u64) -> Result<Self, &'static str> {
        let unique: BTreeSet<_> = keys.iter().copied().collect();
        let required = faults
            .checked_mul(3)
            .and_then(|v| v.checked_add(1))
            .ok_or("quorum overflow")?;
        if unique.len() != keys.len() || unique.len() != required {
            return Err("invalid quorum membership");
        }
        for key in &unique {
            VerifyingKey::from_bytes(key).map_err(|_| "invalid public key")?;
        }
        Ok(Self {
            keys: unique,
            faults,
            epoch,
        })
    }
    pub fn verify(
        &self,
        digest: [u8; 32],
        sequence: u64,
        expires_ms: u64,
        now_ms: u64,
        votes: &[Vote],
    ) -> Result<Verified, &'static str> {
        if now_ms >= expires_ms {
            return Err("expired certificate");
        }
        let message = signing_message(digest, sequence, self.epoch, expires_ms);
        let mut seen = BTreeSet::new();
        for vote in votes {
            if !self.keys.contains(&vote.signer) || !seen.insert(vote.signer) {
                return Err("unknown or duplicate signer");
            }
            VerifyingKey::from_bytes(&vote.signer)
                .map_err(|_| "invalid key")?
                .verify_strict(&message, &Signature::from_bytes(&vote.signature))
                .map_err(|_| "invalid signature")?;
        }
        if seen.len() < 2 * self.faults + 1 {
            return Err("insufficient quorum");
        }
        Ok(Verified {
            digest,
            sequence,
            epoch: self.epoch,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn verifies_unique_bound_and_fresh_votes() {
        let keys: Vec<_> = (1..=4).map(|n| SigningKey::from_bytes(&[n; 32])).collect();
        let quorum = Quorum::new(
            keys.iter().map(|k| k.verifying_key().to_bytes()).collect(),
            1,
            7,
        )
        .unwrap();
        let digest = *blake3::hash(b"transaction").as_bytes();
        let message = signing_message(digest, 5, 7, 1000);
        let votes: Vec<_> = keys
            .iter()
            .take(3)
            .map(|k| Vote {
                signer: k.verifying_key().to_bytes(),
                signature: k.sign(&message).to_bytes(),
            })
            .collect();
        assert!(
            quorum
                .verify(digest, 5, 1000, 999, &votes)
                .unwrap()
                .matches(digest, 5, 7)
        );
        assert!(quorum.verify(digest, 6, 1000, 999, &votes).is_err());
        assert!(quorum.verify(digest, 5, 1000, 1000, &votes).is_err());
        assert!(quorum.verify(digest, 5, 1000, 0, &votes[..2]).is_err());
        assert!(
            quorum
                .verify(
                    digest,
                    5,
                    1000,
                    0,
                    &[votes[0].clone(), votes[0].clone(), votes[1].clone()]
                )
                .is_err()
        );
    }
}
