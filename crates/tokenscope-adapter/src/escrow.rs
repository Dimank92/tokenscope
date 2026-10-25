//! Adapter for the demo escrow (vulnerable and corrected flavors).

use std::path::{Path, PathBuf};

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use tokenscope_executor::{ExecError, Harness, derive_address};
use tokenscope_model::{
    AccountRef, Action, ExecutionTrace, Scenario, Snapshot, StepRecord, StepStatus,
};

/// Escrow state account size; must match `demo_escrow::STATE_LEN`.
const STATE_LEN: usize = 105;
const OFF_RECORDED: usize = 97;
const ESCROW_SEED: &[u8] = b"escrow";

/// Which build of the demo escrow to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowFlavor {
    /// Credits the nominal deposit amount (the bug).
    Vulnerable,
    /// Credits the amount that actually arrived in the vault.
    Corrected,
}

impl EscrowFlavor {
    /// Target identifier used in scenarios.
    pub fn target_id(self) -> &'static str {
        match self {
            Self::Vulnerable => "demo-escrow-vulnerable",
            Self::Corrected => "demo-escrow-corrected",
        }
    }

    /// Parse a target identifier.
    pub fn from_target_id(id: &str) -> Option<Self> {
        match id {
            "demo-escrow-vulnerable" => Some(Self::Vulnerable),
            "demo-escrow-corrected" => Some(Self::Corrected),
            _ => None,
        }
    }

    fn elf_name(self) -> &'static str {
        match self {
            Self::Vulnerable => "demo_escrow_vulnerable.so",
            Self::Corrected => "demo_escrow_corrected.so",
        }
    }
}

/// Locate the directory holding built programs: `$TOKENSCOPE_PROGRAM_DIR`, else
/// `target/sbf-programs` in the current directory or one of its ancestors, else the workspace
/// the crate was compiled from.
pub fn default_program_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TOKENSCOPE_PROGRAM_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(mut dir) = std::env::current_dir() {
        loop {
            let candidate = dir.join("target/sbf-programs");
            if candidate.is_dir() {
                return candidate;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sbf-programs")
}

/// Runs scenarios against one flavor of the demo escrow.
pub struct EscrowAdapter {
    flavor: EscrowFlavor,
    program_dir: PathBuf,
}

impl EscrowAdapter {
    /// Adapter using [`default_program_dir`].
    pub fn new(flavor: EscrowFlavor) -> Self {
        Self {
            flavor,
            program_dir: default_program_dir(),
        }
    }

    /// Adapter reading built programs from `dir`.
    pub fn with_program_dir(flavor: EscrowFlavor, dir: impl Into<PathBuf>) -> Self {
        Self {
            flavor,
            program_dir: dir.into(),
        }
    }

    /// The flavor this adapter runs.
    pub fn flavor(&self) -> EscrowFlavor {
        self.flavor
    }

    fn setup_err(step: &str, detail: impl ToString) -> ExecError {
        ExecError::Setup {
            step: step.into(),
            detail: detail.to_string(),
        }
    }

    /// Execute `scenario` and return the observed trace. `Err` always means a *setup* problem
    /// (never a finding): the scenario was invalid, the program was missing, or the harness could
    /// not establish the initial state.
    pub fn run(&self, scenario: &Scenario) -> Result<ExecutionTrace, ExecError> {
        scenario
            .validate()
            .map_err(|e| Self::setup_err("validate-scenario", e))?;
        if scenario.target != self.flavor.target_id() {
            return Err(Self::setup_err(
                "select-target",
                format!(
                    "scenario targets `{}` but adapter runs `{}`",
                    scenario.target,
                    self.flavor.target_id()
                ),
            ));
        }
        let elf_path = self.program_dir.join(self.flavor.elf_name());
        let elf = std::fs::read(&elf_path).map_err(|e| {
            Self::setup_err(
                "load-program",
                format!(
                    "{}: {e} (build with scripts/build-programs.sh)",
                    elf_path.display()
                ),
            )
        })?;

        let mut h = Harness::new();
        let program_id = derive_address(&format!("program/{}", self.flavor.target_id()));
        h.load_program(&program_id, &elf);

        let token_program = spl_token_2022_interface::id();
        let mint = h.create_mint("mint", &scenario.mint)?;
        let state = h.create_owned_account("escrow-state", STATE_LEN, &program_id)?;
        let (authority, _bump) =
            Pubkey::find_program_address(&[ESCROW_SEED, state.as_ref()], &program_id);
        let depositor = derive_address("depositor");
        let source = h.create_token_account("depositor-src", &mint, &depositor)?;
        let destination = h.create_token_account("withdraw-dst", &mint, &depositor)?;
        let vault = h.create_token_account("vault", &mint, &authority)?;
        h.mint_to(&mint, &source, scenario.initial_balance)?;

        let init = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(state, false),
                AccountMeta::new_readonly(depositor, true),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new_readonly(vault, false),
            ],
            data: vec![0],
        };
        let rep = h.execute_logged(&init);
        if let Some(e) = rep.error {
            return Err(Self::setup_err(
                "init-escrow",
                format!("{e}; logs: {:?}", rep.logs),
            ));
        }

        let snapshot = |h: &Harness| -> Result<Snapshot, ExecError> {
            let v = h.token_account(&vault)?;
            let raw = h.raw_account(&state)?;
            let mut b = [0u8; 8];
            b.copy_from_slice(&raw.data[OFF_RECORDED..OFF_RECORDED + 8]);
            Ok(Snapshot {
                vault_amount: v.amount,
                vault_withheld: v.withheld,
                escrow_recorded: u64::from_le_bytes(b),
                depositor_balance: h.token_account(&source)?.amount,
                destination_balance: h.token_account(&destination)?.amount,
            })
        };

        let accounts = [
            ("program", program_id),
            ("mint", mint),
            ("escrow_state", state),
            ("escrow_authority", authority),
            ("depositor", depositor),
            ("depositor_source", source),
            ("withdraw_destination", destination),
            ("vault", vault),
        ]
        .into_iter()
        .map(|(l, a)| AccountRef {
            label: l.into(),
            address: a.to_string(),
        })
        .collect();

        let mut trace = ExecutionTrace::new(
            scenario.id(),
            scenario.target.clone(),
            snapshot(&h)?,
            accounts,
        );

        for (index, action) in scenario.actions.iter().enumerate() {
            let ix = match action {
                Action::Deposit { amount } => {
                    let mut data = vec![1];
                    data.extend_from_slice(&amount.to_le_bytes());
                    Instruction {
                        program_id,
                        accounts: vec![
                            AccountMeta::new(state, false),
                            AccountMeta::new_readonly(depositor, true),
                            AccountMeta::new(source, false),
                            AccountMeta::new(vault, false),
                            AccountMeta::new_readonly(mint, false),
                            AccountMeta::new_readonly(token_program, false),
                        ],
                        data,
                    }
                }
                Action::Withdraw => Instruction {
                    program_id,
                    accounts: vec![
                        AccountMeta::new(state, false),
                        AccountMeta::new_readonly(depositor, true),
                        AccountMeta::new(vault, false),
                        AccountMeta::new(destination, false),
                        AccountMeta::new_readonly(mint, false),
                        AccountMeta::new_readonly(token_program, false),
                        AccountMeta::new_readonly(authority, false),
                    ],
                    data: vec![2],
                },
            };
            let before = snapshot(&h)?;
            let rep = h.execute_logged(&ix);
            let after = snapshot(&h)?;
            trace.steps.push(StepRecord {
                index,
                action: *action,
                before,
                after,
                status: match rep.error {
                    None => StepStatus::Ok,
                    Some(error) => StepStatus::Failed { error },
                },
                logs: rep.logs,
                compute_units: rep.compute_units,
            });
        }
        Ok(trace)
    }
}
