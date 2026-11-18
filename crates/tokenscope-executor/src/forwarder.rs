//! Fixture support for the demo CPI forwarder program (`programs/demo-forwarder`).

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{Harness, derive_address};

/// Address the demo forwarder program is loaded at.
pub fn demo_forwarder_program_id() -> Pubkey {
    derive_address("program/demo-forwarder")
}

/// Wrap an instruction destined for a target program (e.g. Token-2022) into an instruction
/// to the demo forwarder program, which executes it via cross-program invocation (CPI).
pub fn forward_instruction(forwarder_id: &Pubkey, target_ix: &Instruction) -> Instruction {
    let mut accounts = Vec::with_capacity(1 + target_ix.accounts.len());
    // Account 0 is the target program to invoke.
    accounts.push(AccountMeta::new_readonly(target_ix.program_id, false));
    accounts.extend(target_ix.accounts.iter().cloned());
    Instruction {
        program_id: *forwarder_id,
        accounts,
        data: target_ix.data.clone(),
    }
}

impl Harness {
    /// Load the demo forwarder program ELF into the harness and return its program ID.
    pub fn install_demo_forwarder(&mut self, elf: &[u8]) -> Pubkey {
        let program_id = demo_forwarder_program_id();
        self.load_program(&program_id, elf);
        program_id
    }
}
