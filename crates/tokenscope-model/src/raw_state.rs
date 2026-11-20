//! Generic raw account state for target-agnostic execution evidence.
//!
//! These types capture the objective SVM state independently of any program's domain model.
//! The escrow-specific [`super::Snapshot`] remains the domain-level interpretation produced
//! by the escrow adapter; these generic types are the raw evidence layer beneath it.

use serde::{Deserialize, Serialize};

/// A raw Solana account as observed from the SVM, with no domain interpretation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawAccount {
    /// Base58 address.
    pub address: String,
    /// Program that owns this account.
    pub owner: String,
    /// Lamport balance.
    pub lamports: u64,
    /// Whether the account is executable.
    pub executable: bool,
    /// Raw account data bytes, hex-encoded for JSON portability.
    pub data_hex: String,
    /// Length of the data in bytes (redundant with `data_hex` but useful for display).
    pub data_len: usize,
}

/// A set of raw account observations at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RawSnapshot {
    /// Accounts observed, keyed by role label (e.g. "vault", "escrow_state").
    pub accounts: Vec<RawAccount>,
    /// Total lamports across all observed accounts (for conservation checks).
    pub total_lamports: u64,
}

impl RawSnapshot {
    /// Look up a raw account by its address.
    pub fn by_address(&self, address: &str) -> Option<&RawAccount> {
        self.accounts.iter().find(|a| a.address == address)
    }

    /// Look up a raw account by its label (position in the accounts list matched against
    /// [`super::AccountRef`] labels maintained by the caller).
    pub fn get(&self, index: usize) -> Option<&RawAccount> {
        self.accounts.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_snapshot_round_trips_through_json() {
        let snap = RawSnapshot {
            accounts: vec![RawAccount {
                address: "Abc123".into(),
                owner: "TokenProgram".into(),
                lamports: 1_000_000,
                executable: false,
                data_hex: "deadbeef".into(),
                data_len: 4,
            }],
            total_lamports: 1_000_000,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: RawSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, back);
    }

    #[test]
    fn by_address_lookup_works() {
        let snap = RawSnapshot {
            accounts: vec![
                RawAccount {
                    address: "A".into(),
                    owner: "O".into(),
                    lamports: 10,
                    executable: false,
                    data_hex: String::new(),
                    data_len: 0,
                },
                RawAccount {
                    address: "B".into(),
                    owner: "O".into(),
                    lamports: 20,
                    executable: false,
                    data_hex: String::new(),
                    data_len: 0,
                },
            ],
            total_lamports: 30,
        };
        assert_eq!(snap.by_address("B").unwrap().lamports, 20);
        assert!(snap.by_address("C").is_none());
    }
}
