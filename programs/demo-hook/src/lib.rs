//! Demo transfer-hook program for TokenScope.
//!
//! Implements the `spl-transfer-hook-interface` instruction layout:
//!
//! * `Execute` (discriminator `sha256("spl-transfer-hook-interface:execute")[..8]` followed by the
//!   amount as u64 LE) with accounts `[source, mint, destination, authority, validation, counter]`.
//! * `InitializeExtraAccountMetaList` (its discriminator) in a *fixture form*: the payload is the
//!   raw TLV bytes of the validation account, which the harness produces with the documented
//!   layout; the account must already exist, be owned by this program and have exactly that size.
//! * A fixture-only `[0xF0, flag]` instruction with accounts `[counter]` sets the reject flag. It
//!   is intentionally unauthenticated and must never be used outside tests.
//!
//! The counter account is `[0..8]` invocation count (u64 LE), `[8]` reject flag, `[9..17]` the last
//! transferred amount. `Execute` refuses to run unless the source token account's
//! `TransferHookAccount.transferring` flag is set, i.e. only as part of a real token transfer.

use pinocchio::{AccountView, Address, ProgramResult, entrypoint, error::ProgramError};

entrypoint!(process_instruction);

const EXECUTE: [u8; 8] = [105, 37, 101, 197, 75, 251, 102, 26];
const INITIALIZE: [u8; 8] = [43, 34, 13, 49, 167, 88, 235, 235];
const SEED: &[u8] = b"extra-account-metas";
const TRANSFER_HOOK_ACCOUNT: u16 = 15;
const COUNTER_LEN: usize = 17;

/// Custom errors (`ProgramError::Custom`).
#[repr(u32)]
pub enum HookError {
    /// The reject flag is set on the counter account.
    Rejected = 100,
    /// `Execute` was not invoked from within a token transfer.
    NotTransferring = 101,
    /// The validation account is not the expected PDA or not owned by this program.
    BadValidationAccount = 102,
    /// The counter account is not owned by this program or has the wrong size.
    BadCounter = 103,
}

impl From<HookError> for ProgramError {
    fn from(e: HookError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

fn process_instruction(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    if data.len() == 2 && data[0] == 0xF0 {
        return set_reject(program_id, accounts, data[1]);
    }
    match data.get(..8) {
        Some(d) if d == EXECUTE => {
            let amount: [u8; 8] = data[8..].try_into().map_err(|_| ProgramError::InvalidInstructionData)?;
            execute(program_id, accounts, u64::from_le_bytes(amount))
        }
        Some(d) if d == INITIALIZE => initialize(program_id, accounts, &data[8..]),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn expected_validation(program_id: &Address, mint: &AccountView) -> Address {
    Address::find_program_address(&[SEED, mint.address().as_ref()], program_id).0
}

/// Whether the token account's `TransferHookAccount.transferring` flag is set.
fn is_transferring(token_account: &[u8]) -> bool {
    // Base account is 165 bytes, then the account-type byte, then TLV entries (type u16, len u16).
    let mut offset = 166;
    while offset + 4 <= token_account.len() {
        let ty = u16::from_le_bytes([token_account[offset], token_account[offset + 1]]);
        let len = u16::from_le_bytes([token_account[offset + 2], token_account[offset + 3]]) as usize;
        let value = offset + 4;
        if ty == TRANSFER_HOOK_ACCOUNT {
            return token_account.get(value).is_some_and(|b| *b != 0);
        }
        offset = value + len;
    }
    false
}

fn initialize(program_id: &Address, accounts: &mut [AccountView], payload: &[u8]) -> ProgramResult {
    let [validation, mint, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if validation.address() != &expected_validation(program_id, mint)
        || !validation.owned_by(program_id)
        || !validation.is_writable()
    {
        return Err(HookError::BadValidationAccount.into());
    }
    let mut data = validation.try_borrow_mut()?;
    if data.len() != payload.len() {
        return Err(ProgramError::InvalidAccountData);
    }
    data.copy_from_slice(payload);
    Ok(())
}

fn set_reject(program_id: &Address, accounts: &mut [AccountView], flag: u8) -> ProgramResult {
    let [counter, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !counter.owned_by(program_id) || counter.data_len() != COUNTER_LEN || !counter.is_writable() {
        return Err(HookError::BadCounter.into());
    }
    counter.try_borrow_mut()?[8] = flag;
    Ok(())
}

fn execute(program_id: &Address, accounts: &mut [AccountView], amount: u64) -> ProgramResult {
    let [source, mint, _destination, _authority, validation, counter, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if validation.address() != &expected_validation(program_id, mint) || !validation.owned_by(program_id) {
        return Err(HookError::BadValidationAccount.into());
    }
    if !is_transferring(&source.try_borrow()?) {
        return Err(HookError::NotTransferring.into());
    }
    if !counter.owned_by(program_id) || counter.data_len() != COUNTER_LEN || !counter.is_writable() {
        return Err(HookError::BadCounter.into());
    }
    let mut data = counter.try_borrow_mut()?;
    if data[8] != 0 {
        return Err(HookError::Rejected.into());
    }
    let mut n = [0u8; 8];
    n.copy_from_slice(&data[..8]);
    let next = u64::from_le_bytes(n).checked_add(1).ok_or(ProgramError::ArithmeticOverflow)?;
    data[..8].copy_from_slice(&next.to_le_bytes());
    data[9..17].copy_from_slice(&amount.to_le_bytes());
    Ok(())
}
