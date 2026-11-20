//! Observations captured while executing a scenario against a target.

use serde::{Deserialize, Serialize};

use crate::{Action, RawSnapshot, SCHEMA_VERSION};

/// Real account quantities observed after setup and after each step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Snapshot {
    /// Token balance of the escrow vault (decoded from the Token-2022 account).
    pub vault_amount: u64,
    /// Fee withheld on the vault by the transfer-fee extension, if the extension is present.
    pub vault_withheld: Option<u64>,
    /// Amount the escrow program has recorded as owed to the depositor.
    pub escrow_recorded: u64,
    /// Depositor's source token balance.
    pub depositor_balance: u64,
    /// Withdrawal destination token balance.
    pub destination_balance: u64,
    /// Whether the depositor's source token account is frozen.
    pub depositor_frozen: bool,
    /// Token balance of the attacker's account (used by authorization checks).
    pub attacker_balance: u64,
    /// Whether the escrow vault is frozen.
    #[serde(default)]
    pub vault_frozen: bool,
    /// Whether the mint is paused (`None` without the `Pausable` extension).
    #[serde(default)]
    pub mint_paused: Option<bool>,
    /// Whether CPI Guard is enabled on the depositor's source account.
    #[serde(default)]
    pub source_cpi_guard: bool,
    /// Remaining allowance of the harness delegate on the source account.
    #[serde(default)]
    pub delegate_allowance: u64,
    /// Hook invocations so far (`None` without the `TransferHook` extension).
    #[serde(default)]
    pub hook_invocations: Option<u64>,
    /// Balance of the permanent delegate's own token account.
    #[serde(default)]
    pub issuer_balance: u64,
}

/// Whether an instruction executed successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StepStatus {
    /// The instruction succeeded and its state changes were persisted.
    Ok,
    /// The program or runtime rejected the instruction; state was not persisted.
    Failed {
        /// Error as reported by the runtime.
        error: String,
    },
}

/// Record of one executed action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRecord {
    /// Zero-based action index.
    pub index: usize,
    /// The action performed.
    pub action: Action,
    /// State before the action.
    pub before: Snapshot,
    /// State after the action (equal to `before` when the step failed).
    pub after: Snapshot,
    /// Success or failure.
    pub status: StepStatus,
    /// Program logs produced by the step.
    pub logs: Vec<String>,
    /// Compute units consumed.
    pub compute_units: u64,
    /// Raw account state before the action (generic layer, omitted when not captured).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_before: Option<RawSnapshot>,
    /// Raw account state after the action (generic layer, omitted when not captured).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_after: Option<RawSnapshot>,
}

/// A named address involved in the run, for evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRef {
    /// Role label, e.g. `vault`.
    pub label: String,
    /// Base58 address.
    pub address: String,
}

/// Complete observation of one scenario run (setup succeeded).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Persisted-format version.
    pub schema_version: u32,
    /// Scenario identifier.
    pub scenario_id: String,
    /// Target identifier.
    pub target: String,
    /// Snapshot after setup, before the first action.
    pub initial: Snapshot,
    /// Accounts involved.
    pub accounts: Vec<AccountRef>,
    /// Executed steps in order.
    pub steps: Vec<StepRecord>,
    /// Raw account state after setup (generic layer, omitted when not captured).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_initial: Option<RawSnapshot>,
}

impl ExecutionTrace {
    /// Create an empty trace.
    pub fn new(
        scenario_id: String,
        target: String,
        initial: Snapshot,
        accounts: Vec<AccountRef>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            scenario_id,
            target,
            initial,
            accounts,
            steps: Vec::new(),
            raw_initial: None,
        }
    }

    /// Parse a trace from JSON, rejecting unknown schema versions.
    pub fn from_json(input: &str) -> Result<Self, crate::ModelError> {
        let t: ExecutionTrace =
            serde_json::from_str(input).map_err(|e| crate::ModelError::Malformed(e.to_string()))?;
        if t.schema_version != SCHEMA_VERSION {
            return Err(crate::ModelError::UnsupportedSchema {
                found: t.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(t)
    }

    /// Serialize to pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("trace serialization is infallible")
    }

    /// Look up an account address by label.
    pub fn address_of(&self, label: &str) -> Option<&str> {
        self.accounts
            .iter()
            .find(|a| a.label == label)
            .map(|a| a.address.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(v: u64) -> Snapshot {
        Snapshot {
            vault_amount: v,
            vault_withheld: None,
            escrow_recorded: 0,
            depositor_balance: 0,
            destination_balance: 0,
            depositor_frozen: false,
            attacker_balance: 0,
            vault_frozen: false,
            mint_paused: None,
            source_cpi_guard: false,
            delegate_allowance: 0,
            hook_invocations: None,
            issuer_balance: 0,
        }
    }

    #[test]
    fn trace_round_trips_through_json() {
        let mut t = ExecutionTrace::new(
            "sc_1".into(),
            "t".into(),
            snap(0),
            vec![AccountRef {
                label: "vault".into(),
                address: "A".into(),
            }],
        );
        t.steps.push(StepRecord {
            index: 0,
            action: Action::Deposit { amount: 5 },
            before: snap(0),
            after: snap(4),
            status: StepStatus::Failed { error: "x".into() },
            logs: vec!["l".into()],
            compute_units: 9,
            raw_before: None,
            raw_after: None,
        });
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<ExecutionTrace>(&json).unwrap(), t);
        assert_eq!(t.address_of("vault"), Some("A"));
        assert_eq!(t.address_of("nope"), None);
        assert!(json.contains("\"status\":\"failed\""));
    }
}
