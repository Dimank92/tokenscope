//! Demo escrow for TokenScope.
//!
//! One escrow holds tokens for one depositor in a Token-2022 vault. The program exists in two
//! flavors built from this single source:
//!
//! * **default (vulnerable):** `Deposit` credits the *nominal* amount passed to the transfer.
//!   When the mint has a transfer fee the vault receives less than was credited.
//! * **`corrected` feature:** `Deposit` credits the amount that *actually arrived*, measured from
//!   the vault's real token balance before and after the transfer.
//!
//! Instruction data: `[0]` Init, `[1, amount: u64 LE]` Deposit, `[2]` Withdraw.

use pinocchio::{
    AccountView, Address, ProgramResult,
    cpi::{Seed, Signer},
    entrypoint,
    error::ProgramError,
};
use pinocchio_token_2022::{
    instructions::TransferChecked,
    state::{Account as TokenAccount, Mint},
};

entrypoint!(process_instruction);

/// Size of the escrow state account.
pub const STATE_LEN: usize = 105;
const SEED: &[u8] = b"escrow";

// State layout: [0] initialized | [1..33] depositor | [33..65] mint | [65..97] vault | [97..105] recorded
const OFF_DEPOSITOR: usize = 1;
const OFF_MINT: usize = 33;
const OFF_VAULT: usize = 65;
const OFF_RECORDED: usize = 97;

/// Custom program errors (`ProgramError::Custom`).
#[repr(u32)]
pub enum EscrowError {
    /// State account already initialized.
    AlreadyInitialized = 1,
    /// State account not initialized.
    NotInitialized = 2,
    /// An account has the wrong owner or size.
    BadAccount = 3,
    /// The vault does not match the escrow or is not controlled by the escrow PDA.
    BadVault = 4,
    /// The depositor did not sign or is not the escrow's depositor.
    Unauthorized = 5,
    /// The token program is not the one that owns the vault.
    BadTokenProgram = 6,
    /// The mint does not match the escrow.
    BadMint = 7,
}

impl From<EscrowError> for ProgramError {
    fn from(e: EscrowError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    match data.split_first() {
        Some((0, [])) => init(program_id, accounts),
        Some((1, rest)) => {
            let amount = u64::from_le_bytes(
                rest.try_into()
                    .map_err(|_| ProgramError::InvalidInstructionData)?,
            );
            deposit(program_id, accounts, amount)
        }
        Some((2, [])) => withdraw(program_id, accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn read_recorded(state: &[u8]) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&state[OFF_RECORDED..OFF_RECORDED + 8]);
    u64::from_le_bytes(b)
}

fn check_state(
    program_id: &Address,
    state: &AccountView,
    depositor: &AccountView,
) -> Result<(), ProgramError> {
    if !state.owned_by(program_id) || state.data_len() != STATE_LEN || !state.is_writable() {
        return Err(EscrowError::BadAccount.into());
    }
    let data = state.try_borrow()?;
    if data[0] != 1 {
        return Err(EscrowError::NotInitialized.into());
    }
    if !depositor.is_signer()
        || depositor.address().as_ref() != &data[OFF_DEPOSITOR..OFF_DEPOSITOR + 32]
    {
        return Err(EscrowError::Unauthorized.into());
    }
    Ok(())
}

fn init(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [state, depositor, mint, vault] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !state.owned_by(program_id) || state.data_len() != STATE_LEN || !state.is_writable() {
        return Err(EscrowError::BadAccount.into());
    }
    if !depositor.is_signer() {
        return Err(EscrowError::Unauthorized.into());
    }
    let (pda, _) = Address::find_program_address(&[SEED, state.address().as_ref()], program_id);
    {
        let v = TokenAccount::from_account_view(vault)?;
        if v.mint() != mint.address() {
            return Err(EscrowError::BadMint.into());
        }
        if v.owner() != &pda {
            return Err(EscrowError::BadVault.into());
        }
    }
    let state_key = *state.address();
    let mut data = state.try_borrow_mut()?;
    if data[0] != 0 {
        return Err(EscrowError::AlreadyInitialized.into());
    }
    let _ = state_key;
    data[0] = 1;
    data[OFF_DEPOSITOR..OFF_DEPOSITOR + 32].copy_from_slice(depositor.address().as_ref());
    data[OFF_MINT..OFF_MINT + 32].copy_from_slice(mint.address().as_ref());
    data[OFF_VAULT..OFF_VAULT + 32].copy_from_slice(vault.address().as_ref());
    data[OFF_RECORDED..OFF_RECORDED + 8].copy_from_slice(&0u64.to_le_bytes());
    Ok(())
}

fn vault_amount(vault: &AccountView) -> Result<u64, ProgramError> {
    Ok(TokenAccount::from_account_view(vault)?.amount())
}

fn deposit(program_id: &Address, accounts: &mut [AccountView], amount: u64) -> ProgramResult {
    let [state, depositor, source, vault, mint, token_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    check_state(program_id, state, depositor)?;
    {
        let data = state.try_borrow()?;
        if vault.address().as_ref() != &data[OFF_VAULT..OFF_VAULT + 32] {
            return Err(EscrowError::BadVault.into());
        }
        if mint.address().as_ref() != &data[OFF_MINT..OFF_MINT + 32] {
            return Err(EscrowError::BadMint.into());
        }
    }
    if !vault.owned_by(token_program.address()) {
        return Err(EscrowError::BadTokenProgram.into());
    }
    let decimals = Mint::from_account_view(mint)?.decimals();
    let before = vault_amount(vault)?;

    TransferChecked {
        from: source,
        mint,
        to: vault,
        authority: depositor,
        amount,
        decimals,
        token_program: token_program.address(),
    }
    .invoke()?;

    // THE ONLY DIFFERENCE between the two flavors.
    #[cfg(feature = "corrected")]
    let credit = {
        let after = vault_amount(vault)?;
        after
            .checked_sub(before)
            .ok_or(ProgramError::ArithmeticOverflow)?
    };
    #[cfg(not(feature = "corrected"))]
    let credit = {
        let _ = before;
        amount
    };

    let mut data = state.try_borrow_mut()?;
    let new_recorded = read_recorded(&data)
        .checked_add(credit)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    data[OFF_RECORDED..OFF_RECORDED + 8].copy_from_slice(&new_recorded.to_le_bytes());
    Ok(())
}

fn withdraw(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [
        state,
        depositor,
        vault,
        destination,
        mint,
        token_program,
        authority,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    check_state(program_id, state, depositor)?;
    let recorded = {
        let data = state.try_borrow()?;
        if vault.address().as_ref() != &data[OFF_VAULT..OFF_VAULT + 32] {
            return Err(EscrowError::BadVault.into());
        }
        if mint.address().as_ref() != &data[OFF_MINT..OFF_MINT + 32] {
            return Err(EscrowError::BadMint.into());
        }
        read_recorded(&data)
    };
    if !vault.owned_by(token_program.address()) {
        return Err(EscrowError::BadTokenProgram.into());
    }
    let (pda, bump) = Address::find_program_address(&[SEED, state.address().as_ref()], program_id);
    if authority.address() != &pda {
        return Err(EscrowError::BadVault.into());
    }
    let decimals = Mint::from_account_view(mint)?.decimals();
    let bump_seed = [bump];
    let seeds = [
        Seed::from(SEED),
        Seed::from(state.address().as_ref()),
        Seed::from(&bump_seed),
    ];
    TransferChecked {
        from: vault,
        mint,
        to: destination,
        authority,
        amount: recorded,
        decimals,
        token_program: token_program.address(),
    }
    .invoke_signed(&[Signer::from(&seeds)])?;

    let mut data = state.try_borrow_mut()?;
    data[OFF_RECORDED..OFF_RECORDED + 8].copy_from_slice(&0u64.to_le_bytes());
    Ok(())
}
