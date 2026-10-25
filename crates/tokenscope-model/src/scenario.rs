//! Scenario definitions: what a campaign asks a target program to do.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ModelError, SCHEMA_VERSION};

/// Maximum number of actions in one scenario (bounds work on untrusted input).
pub const MAX_ACTIONS: usize = 64;
/// Maximum length of a scenario name.
pub const MAX_NAME_LEN: usize = 128;
/// Maximum transfer-fee basis points (100%).
pub const MAX_BASIS_POINTS: u16 = 10_000;

/// Token-2022 `TransferFeeConfig` parameters applied at mint creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferFeeParams {
    /// Fee in basis points of the transferred amount.
    pub basis_points: u16,
    /// Upper bound on the fee for a single transfer.
    pub maximum_fee: u64,
}

/// Mint configuration for a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintSpec {
    /// Mint decimals.
    pub decimals: u8,
    /// Optional transfer-fee extension.
    pub transfer_fee: Option<TransferFeeParams>,
}

/// One step a scenario performs against the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    /// Deposit a nominal token amount into the escrow.
    Deposit {
        /// Nominal amount passed to the transfer instruction.
        amount: u64,
    },
    /// Withdraw the depositor's recorded balance from the escrow.
    Withdraw,
    /// The mint's freeze authority freezes the depositor's source token account.
    FreezeDepositor,
    /// The mint's freeze authority thaws the depositor's source token account.
    ThawDepositor,
    /// An attacker (not the depositor) signs a withdrawal. A correct program must reject it.
    UnauthorizedWithdraw,
}

impl Action {
    /// Whether a rejected instruction is the *desired* outcome for this adversarial action, so a
    /// failed step must not be classified as an execution failure.
    pub fn expects_rejection(self) -> bool {
        matches!(self, Action::UnauthorizedWithdraw)
    }
}

/// A complete, self-contained, replayable scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scenario {
    /// Persisted-format version.
    pub schema_version: u32,
    /// Human-readable name.
    pub name: String,
    /// Seed that produced this scenario (0 for hand-written scenarios).
    pub seed: u64,
    /// Target adapter identifier, e.g. `demo-escrow-vulnerable`.
    pub target: String,
    /// Mint configuration.
    pub mint: MintSpec,
    /// Tokens minted to the depositor before the first action.
    pub initial_balance: u64,
    /// Ordered actions.
    pub actions: Vec<Action>,
}

impl Scenario {
    /// Validate invariants of the scenario itself (not of any target program).
    pub fn validate(&self) -> Result<(), ModelError> {
        let bad = |m: &str| Err(ModelError::InvalidScenario(m.to_string()));
        if self.schema_version != SCHEMA_VERSION {
            return Err(ModelError::UnsupportedSchema {
                found: self.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        if self.name.is_empty() || self.name.len() > MAX_NAME_LEN {
            return bad("name must be 1..=128 bytes");
        }
        if self.target.is_empty() {
            return bad("target must not be empty");
        }
        if self.actions.is_empty() {
            return bad("at least one action is required");
        }
        if self.actions.len() > MAX_ACTIONS {
            return bad("too many actions");
        }
        if let Some(fee) = self.mint.transfer_fee
            && fee.basis_points > MAX_BASIS_POINTS
        {
            return bad("basis_points must be <= 10000");
        }
        Ok(())
    }

    /// Parse and validate a scenario from JSON.
    pub fn from_json(input: &str) -> Result<Self, ModelError> {
        let s: Scenario =
            serde_json::from_str(input).map_err(|e| ModelError::Malformed(e.to_string()))?;
        s.validate()?;
        Ok(s)
    }

    /// Serialize to pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("scenario serialization is infallible")
    }

    /// Stable content-derived identifier (`sc_` + 16 hex chars of SHA-256 over compact JSON).
    pub fn id(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("scenario serialization is infallible");
        let digest = Sha256::digest(bytes);
        let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
        format!("sc_{hex}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Scenario {
        Scenario {
            schema_version: SCHEMA_VERSION,
            name: "fee-2pct".into(),
            seed: 7,
            target: "demo-escrow-vulnerable".into(),
            mint: MintSpec {
                decimals: 6,
                transfer_fee: Some(TransferFeeParams {
                    basis_points: 200,
                    maximum_fee: u64::MAX,
                }),
            },
            initial_balance: 10_000,
            actions: vec![Action::Deposit { amount: 1_000 }],
        }
    }

    #[test]
    fn valid_scenario_passes() {
        assert_eq!(sample().validate(), Ok(()));
    }

    #[test]
    fn json_round_trip_preserves_scenario() {
        let s = sample();
        assert_eq!(Scenario::from_json(&s.to_json()).unwrap(), s);
    }

    #[test]
    fn id_is_deterministic_and_content_sensitive() {
        let a = sample();
        let mut b = sample();
        assert_eq!(a.id(), sample().id());
        b.actions = vec![Action::Deposit { amount: 1_001 }];
        assert_ne!(a.id(), b.id());
        assert!(a.id().starts_with("sc_") && a.id().len() == 19);
    }

    #[test]
    fn rejects_bad_inputs() {
        let mut s = sample();
        s.actions.clear();
        assert!(s.validate().is_err());
        let mut s = sample();
        s.name.clear();
        assert!(s.validate().is_err());
        let mut s = sample();
        s.mint.transfer_fee = Some(TransferFeeParams {
            basis_points: 10_001,
            maximum_fee: 1,
        });
        assert!(s.validate().is_err());
        let mut s = sample();
        s.actions = vec![Action::Withdraw; MAX_ACTIONS + 1];
        assert!(s.validate().is_err());
        let mut s = sample();
        s.schema_version = 99;
        assert_eq!(
            s.validate(),
            Err(ModelError::UnsupportedSchema {
                found: 99,
                supported: 1
            })
        );
    }

    #[test]
    fn malformed_json_is_a_structured_error() {
        assert!(matches!(
            Scenario::from_json("{not json"),
            Err(ModelError::Malformed(_))
        ));
        assert!(matches!(
            Scenario::from_json(r#"{"schema_version":1}"#),
            Err(ModelError::Malformed(_))
        ));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn arb_action() -> impl Strategy<Value = Action> {
        prop_oneof![
            any::<u64>().prop_map(|amount| Action::Deposit { amount }),
            Just(Action::Withdraw)
        ]
    }

    pub(crate) fn arb_scenario() -> impl Strategy<Value = Scenario> {
        (
            "[a-z]{1,20}",
            any::<u64>(),
            any::<u8>(),
            proptest::option::of((0u16..=10_000, any::<u64>())),
            any::<u64>(),
            proptest::collection::vec(arb_action(), 1..=MAX_ACTIONS),
        )
            .prop_map(
                |(name, seed, decimals, fee, initial_balance, actions)| Scenario {
                    schema_version: SCHEMA_VERSION,
                    name,
                    seed,
                    target: "demo-escrow-vulnerable".into(),
                    mint: MintSpec {
                        decimals,
                        transfer_fee: fee.map(|(basis_points, maximum_fee)| TransferFeeParams {
                            basis_points,
                            maximum_fee,
                        }),
                    },
                    initial_balance,
                    actions,
                },
            )
    }

    proptest! {
        #[test]
        fn any_valid_scenario_round_trips_and_keeps_its_id(s in arb_scenario()) {
            prop_assert!(s.validate().is_ok());
            let back = Scenario::from_json(&s.to_json()).unwrap();
            prop_assert_eq!(back.id(), s.id());
            prop_assert_eq!(back, s);
        }

        #[test]
        fn arbitrary_text_never_panics_the_parser(input in ".{0,200}") {
            let _ = Scenario::from_json(&input);
        }
    }
}
