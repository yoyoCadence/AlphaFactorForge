//! `seed-v1`: deterministic candidate sub-seed derivation, ported from
//! `src/services/discoverySeed.ts`.
//!
//! Per the PR #66 Resolution D4 the preimage contains ONLY durable inputs: an
//! explicit stored `rootSeed`, the dataset content identity, the candidate's
//! strategy identity, and the purpose. Row ids, thread ids, enumeration order,
//! and completion order never participate, so a resumed or rescheduled run
//! reproduces the same random streams.

use sha2::{Digest, Sha256};

use super::identity::{DATASET_HASH_VERSION, STRATEGY_HASH_VERSION};

pub const DISCOVERY_SEED_VERSION: &str = "seed-v1";
pub const DISCOVERY_SEED_PURPOSES: [&str; 1] = ["random-entry"];
pub const MAX_U32: f64 = 4_294_967_295.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeedError(pub String);

impl std::fmt::Display for SeedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SeedError {}

#[derive(Clone, Debug)]
pub struct DeriveSeedArgs<'a> {
    /// `f64` so a fractional or out-of-range JSON value is rejected with the
    /// reference implementation's own message instead of silently truncating.
    pub root_seed: f64,
    pub dataset_content_hash: &'a str,
    pub strategy_hash: &'a str,
    pub purpose: &'a str,
}

fn assert_u32(value: f64, name: &str) -> Result<u32, SeedError> {
    if !value.is_finite() || value.fract() != 0.0 || !(0.0..=MAX_U32).contains(&value) {
        return Err(SeedError(format!(
            "{name} must be an integer in [0, 4294967295]"
        )));
    }
    Ok(value as u32)
}

/// `<version>:<64 lowercase hex>`. A bare prefix, a truncated digest, or
/// uppercase hex is not a usable identity: accepting one would let a malformed
/// value silently seed a real random stream.
pub fn is_durable_identity(value: &str, prefix: &str) -> bool {
    let marker = format!("{prefix}:");
    match value.strip_prefix(&marker) {
        Some(digest) => {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }
        None => false,
    }
}

fn assert_identity(value: &str, name: &str, prefix: &str) -> Result<(), SeedError> {
    if !is_durable_identity(value, prefix) {
        return Err(SeedError(format!(
            "{name} must be a durable {prefix} identity"
        )));
    }
    Ok(())
}

/// Exact `seed-v1` preimage; see `discoverySeed.ts` for the byte layout.
pub fn discovery_seed_preimage(args: &DeriveSeedArgs<'_>) -> Result<Vec<u8>, SeedError> {
    let root_seed = assert_u32(args.root_seed, "rootSeed")?;
    assert_identity(
        args.dataset_content_hash,
        "datasetContentHash",
        DATASET_HASH_VERSION,
    )?;
    assert_identity(args.strategy_hash, "strategyHash", STRATEGY_HASH_VERSION)?;
    if !DISCOVERY_SEED_PURPOSES.contains(&args.purpose) {
        return Err(SeedError(format!(
            "unsupported seed purpose \"{}\"",
            args.purpose
        )));
    }

    let mut out = Vec::new();
    out.extend_from_slice(DISCOVERY_SEED_VERSION.as_bytes());
    out.push(0);
    out.extend_from_slice(&root_seed.to_be_bytes());
    for value in [args.dataset_content_hash, args.strategy_hash, args.purpose] {
        let length = u32::try_from(value.len())
            .map_err(|_| SeedError("seed field length exceeds u32".into()))?;
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(value.as_bytes());
    }
    Ok(out)
}

/// The first four digest bytes read big-endian.
pub fn derive_discovery_seed(args: &DeriveSeedArgs<'_>) -> Result<u32, SeedError> {
    let digest = Sha256::digest(discovery_seed_preimage(args)?);
    Ok(u32::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3],
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATASET: &str =
        "dataset-content-v2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const STRATEGY: &str =
        "strategy-v2:1111111111111111111111111111111111111111111111111111111111111111";

    fn args<'a>(root_seed: f64) -> DeriveSeedArgs<'a> {
        DeriveSeedArgs {
            root_seed,
            dataset_content_hash: DATASET,
            strategy_hash: STRATEGY,
            purpose: "random-entry",
        }
    }

    #[test]
    fn length_prefixes_every_field() {
        let preimage = discovery_seed_preimage(&args(1.0)).unwrap();
        assert_eq!(&preimage[..8], b"seed-v1\0");
        assert_eq!(&preimage[8..12], &[0, 0, 0, 1]);
        assert_eq!(preimage.len(), 8 + 4 + (4 + 83) + (4 + 76) + (4 + 12));
    }

    #[test]
    fn rejects_out_of_range_seeds_and_non_durable_identities() {
        assert!(discovery_seed_preimage(&args(-1.0)).is_err());
        assert!(discovery_seed_preimage(&args(MAX_U32 + 1.0)).is_err());
        assert!(discovery_seed_preimage(&args(1.5)).is_err());
        assert!(discovery_seed_preimage(&DeriveSeedArgs {
            purpose: "gate",
            ..args(1.0)
        })
        .is_err());
    }

    #[test]
    fn durable_identity_requires_a_full_lowercase_hex_digest() {
        assert!(is_durable_identity(DATASET, DATASET_HASH_VERSION));
        assert!(is_durable_identity(STRATEGY, STRATEGY_HASH_VERSION));
        // A bare prefix, a truncated digest, uppercase hex, and non-hex all
        // fail: each would otherwise seed a real stream from a malformed id.
        assert!(!is_durable_identity("strategy-v2:", STRATEGY_HASH_VERSION));
        assert!(!is_durable_identity(
            "strategy-v2:abc",
            STRATEGY_HASH_VERSION
        ));
        assert!(!is_durable_identity(
            &format!("strategy-v2:{}", "A".repeat(64)),
            STRATEGY_HASH_VERSION
        ));
        assert!(!is_durable_identity(
            &format!("strategy-v2:{}", "g".repeat(64)),
            STRATEGY_HASH_VERSION
        ));
        assert!(!is_durable_identity(
            "legacy-unversioned",
            STRATEGY_HASH_VERSION
        ));
    }
}
