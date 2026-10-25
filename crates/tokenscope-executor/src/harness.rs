//! Stateful Token-2022 execution environment.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use mollusk_svm::{
    Mollusk, MolluskContext, program::loader_keys::LOADER_V3, result::ProgramResult,
};
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use solana_svm_log_collector::LogCollector;
use spl_token_2022_interface::{
    extension::{
        BaseStateWithExtensions, ExtensionType, StateWithExtensions, account_len,
        transfer_fee::{TransferFeeAmount, TransferFeeConfig, instruction as fee_ix},
    },
    instruction as token_ix,
    state::{Account as TokenAccount, Mint},
};
use tokenscope_model::MintSpec;

use crate::ExecError;

/// Deterministically derive an address from a label (reproducible runs need stable addresses).
pub fn derive_address(label: &str) -> Pubkey {
    let digest = Sha256::digest(format!("tokenscope/address/{label}"));
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest);
    Pubkey::new_from_array(bytes)
}

/// Decoded view of a Token-2022 token account (read from real account data).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenAccountView {
    /// Spendable token balance.
    pub amount: u64,
    /// Fee amount withheld on this account by the transfer-fee extension, if present.
    pub withheld: Option<u64>,
}

/// Execution environment holding a Mollusk SVM and its persistent account store.
pub struct Harness {
    ctx: MolluskContext<HashMap<Pubkey, Account>>,
    payer: Pubkey,
    logs: Rc<RefCell<LogCollector>>,
}

/// Outcome of executing one instruction, including captured logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecReport {
    /// `None` on success, otherwise the runtime error text.
    pub error: Option<String>,
    /// Log lines emitted by this instruction.
    pub logs: Vec<String>,
    /// Compute units consumed.
    pub compute_units: u64,
}

impl Harness {
    /// Create an environment with the system program and Token-2022 loaded and a funded payer.
    pub fn new() -> Self {
        let mut mollusk = Mollusk::default();
        let logs = LogCollector::new_ref();
        mollusk.logger = Some(Rc::clone(&logs));
        mollusk_svm_programs_token_2022::add_program(&mut mollusk);
        let payer = derive_address("payer");
        let mut store = HashMap::new();
        store.insert(
            payer,
            Account::new(10_000_000_000, 0, &solana_system_interface::program::ID),
        );
        Self {
            ctx: mollusk.with_context(store),
            payer,
            logs,
        }
    }

    /// The funded fee payer / default authority.
    pub fn payer(&self) -> Pubkey {
        self.payer
    }

    /// Access to the underlying Mollusk context (for target program registration).
    pub fn context(&self) -> &MolluskContext<HashMap<Pubkey, Account>> {
        &self.ctx
    }

    /// Mutable access to the underlying Mollusk (for target program registration).
    pub fn mollusk_mut(&mut self) -> &mut Mollusk {
        &mut self.ctx.mollusk
    }

    /// Register a program ELF under the upgradeable loader (as deployed programs are).
    pub fn load_program(&mut self, program_id: &Pubkey, elf: &[u8]) {
        self.ctx
            .mollusk
            .add_program_with_loader_and_elf(program_id, &LOADER_V3, elf);
    }

    /// Execute one instruction and report errors, logs and compute usage. State persists only
    /// when the instruction succeeds.
    pub fn execute_logged(&self, ix: &Instruction) -> ExecReport {
        let start = self.logs.borrow().get_recorded_content().len();
        let result = self.ctx.process_instruction(ix);
        let logs = self.logs.borrow().get_recorded_content()[start..].to_vec();
        let error = match result.program_result {
            ProgramResult::Success => None,
            other => Some(format!("{other:?}")),
        };
        ExecReport {
            error,
            logs,
            compute_units: result.compute_units_consumed,
        }
    }

    /// Execute one instruction, persisting state on success (failed instructions leave state intact).
    pub fn execute(&self, ix: &Instruction) -> Result<(), ExecError> {
        match self.execute_logged(ix).error {
            None => Ok(()),
            Some(e) => Err(ExecError::Instruction(e)),
        }
    }

    /// Create an account owned by `owner` with `space` bytes, rent-exempt, funded by the payer.
    pub fn create_owned_account(
        &self,
        label: &str,
        space: usize,
        owner: &Pubkey,
    ) -> Result<Pubkey, ExecError> {
        let address = derive_address(&format!("account/{label}"));
        let ix = solana_system_interface::instruction::create_account(
            &self.payer,
            &address,
            self.rent_for(space),
            space as u64,
            owner,
        );
        self.setup("create-account", &[ix])?;
        Ok(address)
    }

    fn setup(&self, step: &str, ixs: &[Instruction]) -> Result<(), ExecError> {
        for ix in ixs {
            self.execute(ix).map_err(|e| ExecError::Setup {
                step: step.into(),
                detail: e.to_string(),
            })?;
        }
        Ok(())
    }

    fn rent_for(&self, space: usize) -> u64 {
        self.ctx.mollusk.sysvars.rent.minimum_balance(space)
    }

    /// Create and initialize a Token-2022 mint via real instructions, with an optional
    /// `TransferFeeConfig`. Returns the mint address.
    pub fn create_mint(&self, label: &str, spec: &MintSpec) -> Result<Pubkey, ExecError> {
        let mint = derive_address(&format!("mint/{label}"));
        let token = spl_token_2022_interface::id();
        let mut exts = Vec::new();
        if spec.transfer_fee.is_some() {
            exts.push(ExtensionType::TransferFeeConfig);
        }
        let space = ExtensionType::try_calculate_account_len::<Mint>(&exts).map_err(|e| {
            ExecError::Setup {
                step: "mint-size".into(),
                detail: e.to_string(),
            }
        })?;
        let mut ixs = vec![solana_system_interface::instruction::create_account(
            &self.payer,
            &mint,
            self.rent_for(space),
            space as u64,
            &token,
        )];
        if let Some(fee) = spec.transfer_fee {
            ixs.push(
                fee_ix::initialize_transfer_fee_config(
                    &token,
                    &mint,
                    Some(&self.payer),
                    Some(&self.payer),
                    fee.basis_points,
                    fee.maximum_fee,
                )
                .map_err(|e| ExecError::Setup {
                    step: "fee-config-ix".into(),
                    detail: e.to_string(),
                })?,
            );
        }
        ixs.push(
            token_ix::initialize_mint2(&token, &mint, &self.payer, None, spec.decimals).map_err(
                |e| ExecError::Setup {
                    step: "init-mint-ix".into(),
                    detail: e.to_string(),
                },
            )?,
        );
        self.setup("create-mint", &ixs)?;
        Ok(mint)
    }

    /// Create and initialize a Token-2022 token account for `mint` owned by `owner`.
    pub fn create_token_account(
        &self,
        label: &str,
        mint: &Pubkey,
        owner: &Pubkey,
    ) -> Result<Pubkey, ExecError> {
        let account = derive_address(&format!("token-account/{label}"));
        let token = spl_token_2022_interface::id();
        let mint_data = self.account(mint)?.data;
        let space = account_len::try_calculate_account_len_from_mint_data(&mint_data, &[])
            .map_err(|e| ExecError::Setup {
                step: "account-size".into(),
                detail: e.to_string(),
            })?;
        let ixs = [
            solana_system_interface::instruction::create_account(
                &self.payer,
                &account,
                self.rent_for(space),
                space as u64,
                &token,
            ),
            token_ix::initialize_account3(&token, &account, mint, owner).map_err(|e| {
                ExecError::Setup {
                    step: "init-account-ix".into(),
                    detail: e.to_string(),
                }
            })?,
        ];
        self.setup("create-token-account", &ixs)?;
        Ok(account)
    }

    /// Mint `amount` tokens to `destination` using the mint authority (the payer).
    pub fn mint_to(
        &self,
        mint: &Pubkey,
        destination: &Pubkey,
        amount: u64,
    ) -> Result<(), ExecError> {
        let token = spl_token_2022_interface::id();
        let ix = token_ix::mint_to(&token, mint, destination, &self.payer, &[], amount).map_err(
            |e| ExecError::Setup {
                step: "mint-to-ix".into(),
                detail: e.to_string(),
            },
        )?;
        self.setup("mint-to", &[ix])
    }

    /// Build a `TransferChecked` instruction (does not execute it).
    pub fn transfer_checked_ix(
        &self,
        source: &Pubkey,
        mint: &Pubkey,
        destination: &Pubkey,
        authority: &Pubkey,
        amount: u64,
        decimals: u8,
    ) -> Result<Instruction, ExecError> {
        token_ix::transfer_checked(
            &spl_token_2022_interface::id(),
            source,
            mint,
            destination,
            authority,
            &[],
            amount,
            decimals,
        )
        .map_err(|e| ExecError::Setup {
            step: "transfer-ix".into(),
            detail: e.to_string(),
        })
    }

    fn account(&self, address: &Pubkey) -> Result<Account, ExecError> {
        self.ctx
            .account_store
            .borrow()
            .get(address)
            .cloned()
            .ok_or_else(|| ExecError::MissingAccount(address.to_string()))
    }

    /// Decode a token account from its real on-chain data.
    pub fn token_account(&self, address: &Pubkey) -> Result<TokenAccountView, ExecError> {
        let acct = self.account(address)?;
        let state = StateWithExtensions::<TokenAccount>::unpack(&acct.data).map_err(|e| {
            ExecError::Decode {
                account: address.to_string(),
                detail: e.to_string(),
            }
        })?;
        let withheld = state
            .get_extension::<TransferFeeAmount>()
            .ok()
            .map(|e| u64::from(e.withheld_amount));
        Ok(TokenAccountView {
            amount: state.base.amount,
            withheld,
        })
    }

    /// Read the mint's current transfer-fee parameters, if configured (real mint data).
    pub fn mint_fee_params(
        &self,
        mint: &Pubkey,
        epoch: u64,
    ) -> Result<Option<(u16, u64)>, ExecError> {
        let acct = self.account(mint)?;
        let state =
            StateWithExtensions::<Mint>::unpack(&acct.data).map_err(|e| ExecError::Decode {
                account: mint.to_string(),
                detail: e.to_string(),
            })?;
        Ok(state.get_extension::<TransferFeeConfig>().ok().map(|c| {
            let f = c.get_epoch_fee(epoch);
            (
                u16::from(f.transfer_fee_basis_points),
                u64::from(f.maximum_fee),
            )
        }))
    }

    /// Raw account bytes (for evidence capture and tests).
    pub fn raw_account(&self, address: &Pubkey) -> Result<Account, ExecError> {
        self.account(address)
    }
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}
