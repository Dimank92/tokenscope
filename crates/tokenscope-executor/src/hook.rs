//! Fixture support for the demo transfer-hook program (`programs/demo-hook`).
//!
//! The validation account (`ExtraAccountMetaList`) holds a single fixed extra account, the counter.
//! Its bytes follow the documented TLV layout: `8-byte Execute discriminator`, `u32 LE length`,
//! then a list of `u32 LE count` followed by 35-byte `ExtraAccountMeta` entries (discriminator `0`
//! for a fixed address, the 32-byte address, `is_signer`, `is_writable`). If the layout were wrong
//! the real Token-2022 program would reject transfers, which the integration tests would catch.

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{ExecError, Harness, derive_address};

/// `sha256("spl-transfer-hook-interface:execute")[..8]`.
pub const EXECUTE_DISCRIMINATOR: [u8; 8] = [105, 37, 101, 197, 75, 251, 102, 26];
/// `sha256("spl-transfer-hook-interface:initialize-extra-account-metas")[..8]`.
pub const INITIALIZE_DISCRIMINATOR: [u8; 8] = [43, 34, 13, 49, 167, 88, 235, 235];
const COUNTER_LEN: usize = 17;
const VALIDATION_LEN: usize = 8 + 4 + 4 + 35;

/// Address the demo hook program is loaded at (also what mints must point their hook at).
pub fn demo_hook_program_id() -> Pubkey {
    derive_address("program/demo-hook")
}

/// Addresses of an installed demo hook for one mint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookFixture {
    /// The hook program.
    pub program_id: Pubkey,
    /// The `ExtraAccountMetaList` PDA for the mint.
    pub validation: Pubkey,
    /// The counter account the hook updates.
    pub counter: Pubkey,
}

/// Decoded counter account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookCounter {
    /// Number of successful hook invocations.
    pub invocations: u64,
    /// Whether the hook is set to reject.
    pub reject: bool,
    /// Amount of the last successful invocation.
    pub last_amount: u64,
}

fn validation_bytes(counter: &Pubkey) -> Vec<u8> {
    let mut v = Vec::with_capacity(VALIDATION_LEN);
    v.extend_from_slice(&EXECUTE_DISCRIMINATOR);
    v.extend_from_slice(&(4u32 + 35).to_le_bytes());
    v.extend_from_slice(&1u32.to_le_bytes());
    v.push(0); // fixed address
    v.extend_from_slice(counter.as_ref());
    v.push(0); // not a signer
    v.push(1); // writable
    v
}

impl Harness {
    /// Load the demo hook ELF and initialize its validation and counter accounts for `mint`.
    pub fn install_demo_hook(
        &mut self,
        elf: &[u8],
        mint: &Pubkey,
    ) -> Result<HookFixture, ExecError> {
        let program_id = demo_hook_program_id();
        self.load_program(&program_id, elf);
        let (validation, _) =
            Pubkey::find_program_address(&[b"extra-account-metas", mint.as_ref()], &program_id);
        self.create_owned_account_at(&validation, VALIDATION_LEN, &program_id)?;
        let counter = self.create_owned_account("hook-counter", COUNTER_LEN, &program_id)?;
        let mut data = INITIALIZE_DISCRIMINATOR.to_vec();
        data.extend_from_slice(&validation_bytes(&counter));
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(validation, false),
                AccountMeta::new_readonly(*mint, false),
            ],
            data,
        };
        let rep = self.execute_logged(&ix);
        if let Some(e) = rep.error {
            return Err(ExecError::Setup {
                step: "init-hook-validation".into(),
                detail: format!("{e}; logs: {:?}", rep.logs),
            });
        }
        Ok(HookFixture {
            program_id,
            validation,
            counter,
        })
    }

    /// The extra accounts a hook-enabled transfer must carry: counter, validation PDA and the
    /// hook program.
    pub fn hook_extra_accounts(&self, hook: &HookFixture) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(hook.counter, false),
            AccountMeta::new_readonly(hook.validation, false),
            AccountMeta::new_readonly(hook.program_id, false),
        ]
    }

    /// Decode the hook's counter account.
    pub fn hook_counter(&self, hook: &HookFixture) -> Result<HookCounter, ExecError> {
        let raw = self.raw_account(&hook.counter)?;
        let d = &raw.data;
        if d.len() < 17 {
            return Err(ExecError::Decode {
                account: hook.counter.to_string(),
                detail: format!("hook counter data length {} is less than 17", d.len()),
            });
        }
        let le = |r: std::ops::Range<usize>| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&d[r]);
            u64::from_le_bytes(b)
        };
        Ok(HookCounter {
            invocations: le(0..8),
            reject: d[8] != 0,
            last_amount: le(9..17),
        })
    }

    /// Set or clear the hook's reject flag (fixture-only instruction).
    pub fn set_hook_reject(&self, hook: &HookFixture, reject: bool) -> Result<(), ExecError> {
        let ix = Instruction {
            program_id: hook.program_id,
            accounts: vec![AccountMeta::new(hook.counter, false)],
            data: vec![0xF0, u8::from(reject)],
        };
        self.execute(&ix)
    }

    /// Build a direct `Execute` call to the hook (not through a token transfer).
    pub fn direct_execute_ix(
        &self,
        hook: &HookFixture,
        source: &Pubkey,
        mint: &Pubkey,
        destination: &Pubkey,
        authority: &Pubkey,
        amount: u64,
    ) -> Instruction {
        let mut data = EXECUTE_DISCRIMINATOR.to_vec();
        data.extend_from_slice(&amount.to_le_bytes());
        Instruction {
            program_id: hook.program_id,
            accounts: vec![
                AccountMeta::new_readonly(*source, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(*destination, false),
                AccountMeta::new_readonly(*authority, false),
                AccountMeta::new_readonly(hook.validation, false),
                AccountMeta::new(hook.counter, false),
            ],
            data,
        }
    }
}
