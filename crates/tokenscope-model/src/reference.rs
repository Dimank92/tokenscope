//! Independent reference model for Token-2022 accounting and state transitions.
//!
//! This model provides a pure, non-SVM reference implementation for expected account
//! and fee transitions. It is completely independent of Mollusk, Solana SDK, and the
//! program under test.

use thiserror::Error;

use crate::{Action, MintExtension, MintSpec, Scenario, Snapshot};

/// Errors representing expected rejection in the reference model.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReferenceError {
    /// The source or destination account is frozen.
    #[error("account is frozen")]
    AccountFrozen,
    /// The mint is paused.
    #[error("mint is paused")]
    MintPaused,
    /// Insufficient token balance to complete the operation.
    #[error("insufficient funds: required {required}, available {available}")]
    InsufficientFunds {
        /// Tokens needed.
        required: u64,
        /// Current account balance.
        available: u64,
    },
    /// Insufficient delegate allowance.
    #[error("insufficient allowance: required {required}, allowance {allowance}")]
    InsufficientAllowance {
        /// Allowance needed.
        required: u64,
        /// Current delegate allowance.
        allowance: u64,
    },
    /// Action expected to be rejected by authorization policy.
    #[error("operation rejected by authorization policy")]
    Unauthorized,
    /// Unsupported action or mismatch precondition.
    #[error("precondition or mismatch rejected: {0}")]
    PreconditionFailed(String),
}

/// Independent state machine modeling expected Token-2022 and target behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenReferenceModel {
    /// Mint specification (decimals, transfer fee configuration).
    pub mint: MintSpec,
    /// Active mint extensions.
    pub extensions: Vec<MintExtension>,
    /// Simulated account snapshots.
    pub snapshot: Snapshot,
}

impl TokenReferenceModel {
    /// Initialize a reference model from a scenario's initial specification.
    pub fn from_scenario(scenario: &Scenario) -> Self {
        let has_transfer_fee = scenario.mint.transfer_fee.is_some();
        let has_pausable = scenario.extensions.contains(&MintExtension::Pausable);
        let has_default_frozen = scenario
            .extensions
            .iter()
            .any(|e| matches!(e, MintExtension::DefaultAccountState { frozen: true }));
        let has_hook = scenario.extensions.contains(&MintExtension::TransferHook);

        let snapshot = Snapshot {
            vault_amount: 0,
            vault_withheld: if has_transfer_fee { Some(0) } else { None },
            escrow_recorded: 0,
            depositor_balance: scenario.initial_balance,
            destination_balance: 0,
            // Depositor account is frozen initially only if default frozen extension is set
            depositor_frozen: has_default_frozen,
            attacker_balance: 0,
            // Vault account is frozen initially if default frozen is active
            vault_frozen: has_default_frozen,
            mint_paused: if has_pausable { Some(false) } else { None },
            source_cpi_guard: false,
            delegate_allowance: 0,
            hook_invocations: if has_hook { Some(0) } else { None },
            issuer_balance: 0,
        };

        Self {
            mint: scenario.mint,
            extensions: scenario.extensions.clone(),
            snapshot,
        }
    }

    /// Calculate the transfer fee according to Token-2022 ceiling division and maximum fee cap.
    pub fn calculate_fee(&self, amount: u64) -> u64 {
        match self.mint.transfer_fee {
            Some(f) => {
                if f.basis_points == 0 || amount == 0 {
                    0
                } else {
                    let raw = (amount as u128 * f.basis_points as u128).div_ceil(10_000);
                    std::cmp::min(raw, f.maximum_fee as u128) as u64
                }
            }
            None => 0,
        }
    }

    /// Access the current simulated snapshot.
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
    }

    /// Apply an action against the reference state.
    ///
    /// If the action fails, the internal snapshot is NOT updated and the error is returned
    /// (preserving atomic rollback).
    pub fn step(
        &mut self,
        action: Action,
        is_corrected_escrow: bool,
    ) -> Result<Snapshot, ReferenceError> {
        let mut next = self.snapshot;

        match action {
            Action::Deposit { amount } => {
                if next.mint_paused == Some(true) {
                    return Err(ReferenceError::MintPaused);
                }
                if next.depositor_frozen || next.vault_frozen {
                    return Err(ReferenceError::AccountFrozen);
                }
                if next.depositor_balance < amount {
                    return Err(ReferenceError::InsufficientFunds {
                        required: amount,
                        available: next.depositor_balance,
                    });
                }

                let fee = self.calculate_fee(amount);
                let net = amount - fee;

                next.depositor_balance -= amount;
                next.vault_amount += net;
                if let Some(w) = next.vault_withheld.as_mut() {
                    *w += fee;
                }
                if let Some(h) = next.hook_invocations.as_mut() {
                    *h += 1;
                }

                if is_corrected_escrow {
                    next.escrow_recorded += net;
                } else {
                    next.escrow_recorded += amount;
                }
            }

            Action::Withdraw => {
                if next.mint_paused == Some(true) {
                    return Err(ReferenceError::MintPaused);
                }
                if next.vault_frozen {
                    return Err(ReferenceError::AccountFrozen);
                }

                let withdraw_amount = next.escrow_recorded;
                if next.vault_amount < withdraw_amount {
                    return Err(ReferenceError::InsufficientFunds {
                        required: withdraw_amount,
                        available: next.vault_amount,
                    });
                }

                let fee = self.calculate_fee(withdraw_amount);
                let net = withdraw_amount - fee;

                next.vault_amount -= withdraw_amount;
                next.destination_balance += net;
                // On withdraw (Vault -> Destination), the fee is withheld on the recipient (Destination),
                // so vault_withheld is unchanged.
                if let Some(h) = next.hook_invocations.as_mut() {
                    *h += 1;
                }
                next.escrow_recorded = 0;
            }

            Action::FreezeDepositor => {
                next.depositor_frozen = true;
            }

            Action::ThawDepositor => {
                next.depositor_frozen = false;
            }

            Action::ThawVault => {
                next.vault_frozen = false;
            }

            Action::Pause => {
                if next.mint_paused.is_some() {
                    next.mint_paused = Some(true);
                }
            }

            Action::Resume => {
                if next.mint_paused.is_some() {
                    next.mint_paused = Some(false);
                }
            }

            Action::EnableCpiGuard => {
                next.source_cpi_guard = true;
            }

            Action::DisableCpiGuard => {
                next.source_cpi_guard = false;
            }

            Action::ApproveDelegate { amount } => {
                next.delegate_allowance = amount;
            }

            Action::RevokeDelegate => {
                next.delegate_allowance = 0;
            }

            Action::DelegateTransfer { amount } => {
                if next.mint_paused == Some(true) {
                    return Err(ReferenceError::MintPaused);
                }
                if next.depositor_frozen || next.vault_frozen {
                    return Err(ReferenceError::AccountFrozen);
                }
                if next.delegate_allowance < amount {
                    return Err(ReferenceError::InsufficientAllowance {
                        required: amount,
                        allowance: next.delegate_allowance,
                    });
                }
                if next.depositor_balance < amount {
                    return Err(ReferenceError::InsufficientFunds {
                        required: amount,
                        available: next.depositor_balance,
                    });
                }

                let fee = self.calculate_fee(amount);
                let net = amount - fee;

                next.delegate_allowance -= amount;
                next.depositor_balance -= amount;
                next.vault_amount += net;
                if let Some(w) = next.vault_withheld.as_mut() {
                    *w += fee;
                }
                if let Some(h) = next.hook_invocations.as_mut() {
                    *h += 1;
                }
            }

            Action::PermanentDelegateDrain { amount } => {
                let drain = std::cmp::min(amount, next.vault_amount);
                let fee = self.calculate_fee(drain);
                let net = drain - fee;

                next.vault_amount -= drain;
                next.issuer_balance += net;
                if let Some(w) = next.vault_withheld.as_mut() {
                    *w += fee;
                }
            }

            Action::UnauthorizedWithdraw => {
                return Err(ReferenceError::Unauthorized);
            }

            Action::MismatchedDeposit { mismatch, .. } => {
                return Err(ReferenceError::PreconditionFailed(format!("{mismatch:?}")));
            }

            Action::MismatchedWithdraw { mismatch } => {
                return Err(ReferenceError::PreconditionFailed(format!("{mismatch:?}")));
            }

            Action::ReinitEscrow => {
                return Err(ReferenceError::Unauthorized);
            }
        }

        self.snapshot = next;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransferFeeParams;

    #[test]
    fn fee_ceiling_division_and_cap() {
        let scenario = Scenario {
            schema_version: 1,
            name: "ref-test".into(),
            seed: 1,
            target: "demo-escrow-corrected".into(),
            mint: MintSpec {
                decimals: 2,
                transfer_fee: Some(TransferFeeParams {
                    basis_points: 250, // 2.5%
                    maximum_fee: 50,
                }),
            },
            initial_balance: 10_000,
            extensions: vec![],
            actions: vec![],
        };

        let model = TokenReferenceModel::from_scenario(&scenario);

        // 1 unit at 250 bp = 1 * 250 / 10000 = 0.025 -> rounds up to 1
        assert_eq!(model.calculate_fee(1), 1);
        // 1000 units at 250 bp = 25
        assert_eq!(model.calculate_fee(1000), 25);
        // 4000 units at 250 bp = 100 -> capped at 50
        assert_eq!(model.calculate_fee(4000), 50);
    }

    #[test]
    fn atomic_rollback_on_insufficient_funds() {
        let scenario = Scenario {
            schema_version: 1,
            name: "atomic-ref-test".into(),
            seed: 1,
            target: "demo-escrow-corrected".into(),
            mint: MintSpec {
                decimals: 0,
                transfer_fee: None,
            },
            initial_balance: 100,
            extensions: vec![],
            actions: vec![],
        };

        let mut model = TokenReferenceModel::from_scenario(&scenario);
        let before = model.snapshot();

        let res = model.step(Action::Deposit { amount: 500 }, true);
        assert!(matches!(res, Err(ReferenceError::InsufficientFunds { .. })));
        assert_eq!(model.snapshot(), before);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_fee_never_exceeds_cap_or_amount(
            amount in 0u64..=1_000_000_000_000,
            bps in 0u16..=10_000,
            max_fee in 0u64..=1_000_000_000,
        ) {
            let scenario = Scenario {
                schema_version: 1,
                name: "prop-fee".into(),
                seed: 1,
                target: "demo-escrow-corrected".into(),
                mint: MintSpec {
                    decimals: 2,
                    transfer_fee: Some(TransferFeeParams {
                        basis_points: bps,
                        maximum_fee: max_fee,
                    }),
                },
                initial_balance: amount,
                extensions: vec![],
                actions: vec![],
            };
            let model = TokenReferenceModel::from_scenario(&scenario);
            let fee = model.calculate_fee(amount);
            prop_assert!(fee <= max_fee);
            if bps == 0 || amount == 0 {
                prop_assert_eq!(fee, 0);
            }
        }

        #[test]
        fn prop_state_transitions_preserve_atomicity(
            initial_balance in 100u64..=10_000,
            excess_amount in 10_001u64..=100_000,
        ) {
            let scenario = Scenario {
                schema_version: 1,
                name: "prop-atomicity".into(),
                seed: 1,
                target: "demo-escrow-corrected".into(),
                mint: MintSpec {
                    decimals: 0,
                    transfer_fee: None,
                },
                initial_balance,
                extensions: vec![],
                actions: vec![],
            };
            let mut model = TokenReferenceModel::from_scenario(&scenario);
            let before = model.snapshot();
            let res = model.step(Action::Deposit { amount: excess_amount }, true);
            prop_assert!(res.is_err());
            prop_assert_eq!(model.snapshot(), before);
        }
    }
}
