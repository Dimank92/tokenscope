//! Adapter for the demo escrow (vulnerable and corrected flavors).

use std::path::{Path, PathBuf};

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use spl_token_2022_interface::{extension::pausable, instruction as token_ix};
use tokenscope_executor::{
    ExecError, Harness, MintExt, MintPlan, demo_hook_program_id, derive_address,
};
use tokenscope_model::{
    AccountRef, Action, ExecutionTrace, MintExtension, Mismatch, Scenario, Snapshot, StepRecord,
    StepStatus,
};

/// Escrow state account size; must match `demo_escrow::STATE_LEN`.
const STATE_LEN: usize = 105;
const OFF_RECORDED: usize = 97;
const ESCROW_SEED: &[u8] = b"escrow";

/// Accounts of a withdraw instruction (overridden one at a time by adversarial variants).
#[derive(Clone, Copy)]
struct WithdrawAccounts {
    signer: Pubkey,
    vault: Pubkey,
    dest: Pubkey,
    mint: Pubkey,
    token: Pubkey,
    auth: Pubkey,
}

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
    /// (never a finding): the scenario was invalid, a program was missing, or the harness could
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
        let read_elf = |name: &str| {
            let path = self.program_dir.join(name);
            std::fs::read(&path).map_err(|e| {
                Self::setup_err(
                    "load-program",
                    format!(
                        "{}: {e} (build with scripts/build-programs.sh)",
                        path.display()
                    ),
                )
            })
        };
        let elf = read_elf(self.flavor.elf_name())?;

        let mut h = Harness::new();
        let program_id = derive_address(&format!("program/{}", self.flavor.target_id()));
        h.load_program(&program_id, &elf);

        // Mint with the requested extensions, initialized by real instructions.
        let token_program = spl_token_2022_interface::id();
        let issuer = derive_address("issuer");
        let delegate = derive_address("delegate");
        let mut ext_plan: Vec<MintExt> = Vec::new();
        if let Some(fee) = scenario.mint.transfer_fee {
            ext_plan.push(MintExt::TransferFee {
                basis_points: fee.basis_points,
                maximum_fee: fee.maximum_fee,
            });
        }
        let (mut default_frozen, mut hooked) = (false, false);
        for e in &scenario.extensions {
            match *e {
                MintExtension::DefaultAccountState { frozen } => {
                    default_frozen = frozen;
                    ext_plan.push(MintExt::DefaultAccountState { frozen });
                }
                MintExtension::Pausable => ext_plan.push(MintExt::Pausable),
                MintExtension::PermanentDelegate => {
                    ext_plan.push(MintExt::PermanentDelegate { delegate: issuer })
                }
                MintExtension::TransferHook => {
                    hooked = true;
                    ext_plan.push(MintExt::TransferHook {
                        program_id: demo_hook_program_id(),
                    });
                }
            }
        }
        let decimals = scenario.mint.decimals;
        let mint = h.create_mint_ext("mint", &MintPlan::new(decimals, ext_plan))?;
        let hook = if hooked {
            Some(h.install_demo_hook(&read_elf("demo_hook.so")?, &mint)?)
        } else {
            None
        };

        let uses = |f: fn(&Action) -> bool| scenario.actions.iter().any(f);
        let needs_issuer = uses(|a| matches!(a, Action::PermanentDelegateDrain { .. }));
        let needs_delegate = uses(|a| matches!(a, Action::DelegateTransfer { .. }));
        let needs_other_mint = uses(|a| {
            matches!(
                a,
                Action::MismatchedDeposit {
                    mismatch: Mismatch::WrongMint,
                    ..
                } | Action::MismatchedWithdraw {
                    mismatch: Mismatch::WrongMint
                }
            )
        });

        let state = h.create_owned_account("escrow-state", STATE_LEN, &program_id)?;
        let (authority, _bump) =
            Pubkey::find_program_address(&[ESCROW_SEED, state.as_ref()], &program_id);
        let depositor = derive_address("depositor");
        let source = h.create_token_account("depositor-src", &mint, &depositor)?;
        let destination = h.create_token_account("withdraw-dst", &mint, &depositor)?;
        let attacker = derive_address("attacker");
        let attacker_dst = h.create_token_account("attacker-dst", &mint, &attacker)?;
        let vault = h.create_token_account("vault", &mint, &authority)?;
        let issuer_dst = if needs_issuer {
            Some(h.create_token_account("issuer-dst", &mint, &issuer)?)
        } else {
            None
        };
        let delegate_dst = if needs_delegate {
            Some(h.create_token_account("delegate-dst", &mint, &delegate)?)
        } else {
            None
        };
        let other_mint = if needs_other_mint {
            Some(h.create_mint_ext("other-mint", &MintPlan::new(decimals, vec![]))?)
        } else {
            None
        };
        if default_frozen {
            // Setup thaws every account except the vault, so only the vault starts frozen.
            for a in [
                Some(source),
                Some(destination),
                Some(attacker_dst),
                issuer_dst,
                delegate_dst,
            ]
            .into_iter()
            .flatten()
            {
                h.execute(&h.freeze_ix(&a, &mint, false)?)?;
            }
        }
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
            let src = h.token_account(&source)?;
            Ok(Snapshot {
                vault_amount: v.amount,
                vault_withheld: v.withheld,
                escrow_recorded: u64::from_le_bytes(b),
                depositor_balance: src.amount,
                destination_balance: h.token_account(&destination)?.amount,
                depositor_frozen: src.frozen,
                attacker_balance: h.token_account(&attacker_dst)?.amount,
                vault_frozen: v.frozen,
                mint_paused: h.mint_paused(&mint)?,
                source_cpi_guard: h.token_cpi_guard(&source)?.unwrap_or(false),
                delegate_allowance: if src.delegate == Some(delegate) {
                    src.delegated_amount
                } else {
                    0
                },
                hook_invocations: match &hook {
                    Some(f) => Some(h.hook_counter(f)?.invocations),
                    None => None,
                },
                issuer_balance: match issuer_dst {
                    Some(a) => h.token_account(&a)?.amount,
                    None => 0,
                },
            })
        };

        let mut labeled = vec![
            ("program", program_id),
            ("mint", mint),
            ("escrow_state", state),
            ("escrow_authority", authority),
            ("depositor", depositor),
            ("depositor_source", source),
            ("withdraw_destination", destination),
            ("attacker", attacker),
            ("attacker_destination", attacker_dst),
            ("vault", vault),
        ];
        if let Some(a) = issuer_dst {
            labeled.push(("permanent_delegate", issuer));
            labeled.push(("permanent_delegate_destination", a));
        }
        if let Some(a) = delegate_dst {
            labeled.push(("delegate", delegate));
            labeled.push(("delegate_destination", a));
        }
        if let Some(f) = &hook {
            labeled.push(("hook_program", f.program_id));
            labeled.push(("hook_validation", f.validation));
            labeled.push(("hook_counter", f.counter));
        }
        let accounts = labeled
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

        let deposit_ix = |amount: u64, mint_key: Pubkey, vault_key: Pubkey, token_key: Pubkey| {
            let mut data = vec![1];
            data.extend_from_slice(&amount.to_le_bytes());
            Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(state, false),
                    AccountMeta::new_readonly(depositor, true),
                    AccountMeta::new(source, false),
                    AccountMeta::new(vault_key, false),
                    AccountMeta::new_readonly(mint_key, false),
                    AccountMeta::new_readonly(token_key, false),
                ],
                data,
            }
        };
        let base_w = WithdrawAccounts {
            signer: depositor,
            vault,
            dest: destination,
            mint,
            token: token_program,
            auth: authority,
        };
        let withdraw_ix = |w: WithdrawAccounts| Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(state, false),
                AccountMeta::new_readonly(w.signer, true),
                AccountMeta::new(w.vault, false),
                AccountMeta::new(w.dest, false),
                AccountMeta::new_readonly(w.mint, false),
                AccountMeta::new_readonly(w.token, false),
                AccountMeta::new_readonly(w.auth, false),
            ],
            data: vec![2],
        };

        for (index, action) in scenario.actions.iter().enumerate() {
            let ixs: Vec<Instruction> = match *action {
                Action::Deposit { amount } => vec![deposit_ix(amount, mint, vault, token_program)],
                Action::Withdraw => vec![withdraw_ix(base_w)],
                Action::FreezeDepositor => vec![h.freeze_ix(&source, &mint, true)?],
                Action::ThawDepositor => vec![h.freeze_ix(&source, &mint, false)?],
                Action::ThawVault => vec![h.freeze_ix(&vault, &mint, false)?],
                // The attacker signs in the depositor's position.
                Action::UnauthorizedWithdraw => {
                    vec![withdraw_ix(WithdrawAccounts {
                        signer: attacker,
                        dest: attacker_dst,
                        ..base_w
                    })]
                }
                Action::Pause => vec![
                    pausable::instruction::pause(&token_program, &mint, &h.payer(), &[])
                        .map_err(|e| Self::setup_err("pause-ix", e))?,
                ],
                Action::Resume => vec![
                    pausable::instruction::resume(&token_program, &mint, &h.payer(), &[])
                        .map_err(|e| Self::setup_err("resume-ix", e))?,
                ],
                Action::EnableCpiGuard => h.cpi_guard_instructions(&source, &depositor, true)?,
                Action::DisableCpiGuard => h.cpi_guard_instructions(&source, &depositor, false)?,
                Action::ApproveDelegate { amount } => vec![
                    token_ix::approve(&token_program, &source, &delegate, &depositor, &[], amount)
                        .map_err(|e| Self::setup_err("approve-ix", e))?,
                ],
                Action::RevokeDelegate => vec![
                    token_ix::revoke(&token_program, &source, &depositor, &[])
                        .map_err(|e| Self::setup_err("revoke-ix", e))?,
                ],
                Action::DelegateTransfer { amount } => {
                    let to = delegate_dst.ok_or_else(|| {
                        Self::setup_err("delegate-transfer", "no delegate account")
                    })?;
                    vec![h.transfer_checked_ix(&source, &mint, &to, &delegate, amount, decimals)?]
                }
                Action::PermanentDelegateDrain { amount } => {
                    let to =
                        issuer_dst.ok_or_else(|| Self::setup_err("drain", "no issuer account"))?;
                    vec![h.transfer_checked_ix(&vault, &mint, &to, &issuer, amount, decimals)?]
                }
                Action::MismatchedDeposit { mismatch, amount } => vec![match mismatch {
                    Mismatch::WrongMint => {
                        deposit_ix(amount, other_mint.unwrap_or(mint), vault, token_program)
                    }
                    Mismatch::WrongVault => deposit_ix(amount, mint, attacker_dst, token_program),
                    Mismatch::WrongTokenProgram => {
                        deposit_ix(amount, mint, vault, solana_system_interface::program::ID)
                    }
                    // Deposits have no separate authority account; substitute the vault instead.
                    Mismatch::WrongAuthority => {
                        deposit_ix(amount, mint, attacker_dst, token_program)
                    }
                }],
                Action::MismatchedWithdraw { mismatch } => vec![match mismatch {
                    Mismatch::WrongMint => withdraw_ix(WithdrawAccounts {
                        mint: other_mint.unwrap_or(mint),
                        ..base_w
                    }),
                    Mismatch::WrongVault => withdraw_ix(WithdrawAccounts {
                        vault: attacker_dst,
                        ..base_w
                    }),
                    Mismatch::WrongTokenProgram => withdraw_ix(WithdrawAccounts {
                        token: solana_system_interface::program::ID,
                        ..base_w
                    }),
                    Mismatch::WrongAuthority => withdraw_ix(WithdrawAccounts {
                        auth: attacker,
                        ..base_w
                    }),
                }],
                Action::ReinitEscrow => vec![init.clone()],
            };
            let before = snapshot(&h)?;
            let (mut error, mut logs, mut compute_units) = (None, Vec::new(), 0);
            for ix in &ixs {
                let rep = h.execute_logged(ix);
                logs.extend(rep.logs);
                compute_units += rep.compute_units;
                if rep.error.is_some() {
                    error = rep.error;
                    break;
                }
            }
            let after = snapshot(&h)?;
            trace.steps.push(StepRecord {
                index,
                action: *action,
                before,
                after,
                status: match error {
                    None => StepStatus::Ok,
                    Some(error) => StepStatus::Failed { error },
                },
                logs,
                compute_units,
            });
        }
        Ok(trace)
    }
}
