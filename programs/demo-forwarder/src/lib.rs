//! Demo CPI forwarder program for TokenScope.
use pinocchio::{
    AccountView, Address, ProgramResult,
    cpi::invoke_with_slice,
    entrypoint,
    error::ProgramError,
    instruction::{InstructionAccount, InstructionView},
};

entrypoint!(process_instruction);

const MAX_FORWARD_ACCOUNTS: usize = 32;
const ZERO: Address = Address::new_from_array([0u8; 32]);

fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [target_program, remaining @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if remaining.len() > MAX_FORWARD_ACCOUNTS {
        return Err(ProgramError::InvalidArgument);
    }

    let mut instruction_accounts = [const { InstructionAccount::readonly(&ZERO) }; MAX_FORWARD_ACCOUNTS];
    for (i, account) in remaining.iter().enumerate() {
        instruction_accounts[i] = InstructionAccount::new(
            account.address(),
            account.is_writable(),
            account.is_signer(),
        );
    }

    let instruction = InstructionView {
        program_id: target_program.address(),
        accounts: &instruction_accounts[..remaining.len()],
        data,
    };

    invoke_with_slice(&instruction, remaining)
}
