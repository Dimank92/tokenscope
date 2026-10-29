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

/// Token-2022 mint extensions a scenario can enable (besides the transfer fee, which lives in
/// [`MintSpec`]). Parameters are fixed by the harness (authorities are the harness payer; the
/// permanent delegate is the harness "issuer"; the hook is the demo hook program).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MintExtension {
    /// `DefaultAccountState`: new token accounts start frozen (the harness thaws every account
    /// except the escrow vault during setup, so deposits are blocked until `ThawVault`).
    DefaultAccountState {
        /// `true` to freeze new accounts by default.
        frozen: bool,
    },
    /// `Pausable`; the harness payer is the pause authority.
    Pausable,
    /// `PermanentDelegate`; the delegate is the harness "issuer".
    PermanentDelegate,
    /// `TransferHook` pointing at the demo hook program.
    TransferHook,
}

impl MintExtension {
    /// Stable snake_case name of the extension kind.
    pub fn name(self) -> &'static str {
        match self {
            MintExtension::DefaultAccountState { .. } => "default_account_state",
            MintExtension::Pausable => "pausable",
            MintExtension::PermanentDelegate => "permanent_delegate",
            MintExtension::TransferHook => "transfer_hook",
        }
    }

    /// Every extension kind name.
    pub const NAMES: [&'static str; 4] = [
        "default_account_state",
        "pausable",
        "permanent_delegate",
        "transfer_hook",
    ];
}

/// Which account an adversarial instruction substitutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mismatch {
    /// A different mint than the one the escrow was created for.
    WrongMint,
    /// A token account that is not the escrow's vault.
    WrongVault,
    /// A program other than Token-2022 in the token-program position.
    WrongTokenProgram,
    /// An address other than the escrow PDA as the vault authority (withdraw only).
    WrongAuthority,
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
    /// The pause authority pauses the mint (`Pausable`).
    Pause,
    /// The pause authority resumes the mint (`Pausable`).
    Resume,
    /// The depositor's owner enables CPI Guard on the source account (a top-level instruction).
    EnableCpiGuard,
    /// The depositor's owner disables CPI Guard on the source account (a top-level instruction).
    DisableCpiGuard,
    /// The freeze authority thaws the escrow vault (for `DefaultAccountState`).
    ThawVault,
    /// The depositor approves the harness delegate for `amount` on the source account.
    ApproveDelegate {
        /// Allowance in token units.
        amount: u64,
    },
    /// The depositor revokes the delegate.
    RevokeDelegate,
    /// The delegate moves `amount` from the depositor's source account (a direct token transfer,
    /// not through the escrow).
    DelegateTransfer {
        /// Amount to move.
        amount: u64,
    },
    /// The permanent delegate moves `amount` out of the escrow vault with a direct token transfer.
    PermanentDelegateDrain {
        /// Amount to move.
        amount: u64,
    },
    /// A deposit with one account substituted. The escrow must reject it.
    MismatchedDeposit {
        /// Which account is substituted.
        mismatch: Mismatch,
        /// Nominal amount.
        amount: u64,
    },
    /// A withdrawal with one account substituted. The escrow must reject it.
    MismatchedWithdraw {
        /// Which account is substituted.
        mismatch: Mismatch,
    },
    /// Initializing the already-initialized escrow again. The escrow must reject it.
    ReinitEscrow,
}

impl Action {
    /// Stable snake_case name of the action kind (the `kind` tag in JSON).
    pub fn kind_name(self) -> &'static str {
        match self {
            Action::Deposit { .. } => "deposit",
            Action::Withdraw => "withdraw",
            Action::FreezeDepositor => "freeze_depositor",
            Action::ThawDepositor => "thaw_depositor",
            Action::UnauthorizedWithdraw => "unauthorized_withdraw",
            Action::Pause => "pause",
            Action::Resume => "resume",
            Action::EnableCpiGuard => "enable_cpi_guard",
            Action::DisableCpiGuard => "disable_cpi_guard",
            Action::ThawVault => "thaw_vault",
            Action::ApproveDelegate { .. } => "approve_delegate",
            Action::RevokeDelegate => "revoke_delegate",
            Action::DelegateTransfer { .. } => "delegate_transfer",
            Action::PermanentDelegateDrain { .. } => "permanent_delegate_drain",
            Action::MismatchedDeposit { .. } => "mismatched_deposit",
            Action::MismatchedWithdraw { .. } => "mismatched_withdraw",
            Action::ReinitEscrow => "reinit_escrow",
        }
    }

    /// Every action kind name, in declaration order.
    pub const KIND_NAMES: [&'static str; 17] = [
        "deposit",
        "withdraw",
        "freeze_depositor",
        "thaw_depositor",
        "unauthorized_withdraw",
        "pause",
        "resume",
        "enable_cpi_guard",
        "disable_cpi_guard",
        "thaw_vault",
        "approve_delegate",
        "revoke_delegate",
        "delegate_transfer",
        "permanent_delegate_drain",
        "mismatched_deposit",
        "mismatched_withdraw",
        "reinit_escrow",
    ];

    /// The token amount this action carries, if any.
    pub fn amount(self) -> Option<u64> {
        match self {
            Action::Deposit { amount }
            | Action::ApproveDelegate { amount }
            | Action::DelegateTransfer { amount }
            | Action::PermanentDelegateDrain { amount }
            | Action::MismatchedDeposit { amount, .. } => Some(amount),
            _ => None,
        }
    }

    /// The same action with its amount replaced (unchanged for actions without an amount).
    pub fn with_amount(self, value: u64) -> Action {
        match self {
            Action::Deposit { .. } => Action::Deposit { amount: value },
            Action::ApproveDelegate { .. } => Action::ApproveDelegate { amount: value },
            Action::DelegateTransfer { .. } => Action::DelegateTransfer { amount: value },
            Action::PermanentDelegateDrain { .. } => {
                Action::PermanentDelegateDrain { amount: value }
            }
            Action::MismatchedDeposit { mismatch, .. } => Action::MismatchedDeposit {
                mismatch,
                amount: value,
            },
            other => other,
        }
    }

    /// Whether a rejected instruction is the *desired* outcome for this adversarial action, so a
    /// failed step must not be classified as an execution failure.
    pub fn expects_rejection(self) -> bool {
        matches!(
            self,
            Action::UnauthorizedWithdraw
                | Action::MismatchedDeposit { .. }
                | Action::MismatchedWithdraw { .. }
                | Action::ReinitEscrow
        )
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
    /// Additional mint extensions (omitted from JSON when empty, so older documents and IDs are unchanged).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<MintExtension>,
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
    #[test]
    fn kind_names_match_the_json_tags_and_the_constant_list() {
        use super::*;
        let all = [
            Action::Deposit { amount: 1 },
            Action::Withdraw,
            Action::FreezeDepositor,
            Action::ThawDepositor,
            Action::UnauthorizedWithdraw,
            Action::Pause,
            Action::Resume,
            Action::EnableCpiGuard,
            Action::DisableCpiGuard,
            Action::ThawVault,
            Action::ApproveDelegate { amount: 1 },
            Action::RevokeDelegate,
            Action::DelegateTransfer { amount: 1 },
            Action::PermanentDelegateDrain { amount: 1 },
            Action::MismatchedDeposit {
                mismatch: Mismatch::WrongMint,
                amount: 1,
            },
            Action::MismatchedWithdraw {
                mismatch: Mismatch::WrongMint,
            },
            Action::ReinitEscrow,
        ];
        assert_eq!(all.len(), Action::KIND_NAMES.len());
        for (a, name) in all.iter().zip(Action::KIND_NAMES) {
            assert_eq!(a.kind_name(), name);
            let json = serde_json::to_value(a).unwrap();
            assert_eq!(json["kind"], name, "kind_name must equal the serde tag");
        }
        for e in [
            MintExtension::DefaultAccountState { frozen: true },
            MintExtension::Pausable,
            MintExtension::PermanentDelegate,
            MintExtension::TransferHook,
        ] {
            assert!(MintExtension::NAMES.contains(&e.name()));
            assert_eq!(serde_json::to_value(e).unwrap()["kind"], e.name());
        }
    }

    #[test]
    fn amount_accessors_cover_exactly_the_amount_carrying_actions() {
        use super::*;
        for a in [
            Action::Deposit { amount: 5 },
            Action::ApproveDelegate { amount: 5 },
            Action::DelegateTransfer { amount: 5 },
            Action::PermanentDelegateDrain { amount: 5 },
            Action::MismatchedDeposit {
                mismatch: Mismatch::WrongVault,
                amount: 5,
            },
        ] {
            assert_eq!(a.amount(), Some(5));
            assert_eq!(a.with_amount(9).amount(), Some(9));
        }
        for a in [
            Action::Withdraw,
            Action::Pause,
            Action::ReinitEscrow,
            Action::ThawVault,
        ] {
            assert_eq!(a.amount(), None);
            assert_eq!(a.with_amount(9), a);
        }
    }

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
            extensions: vec![],
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
                    extensions: vec![],
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
