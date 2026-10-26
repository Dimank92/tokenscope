//! Helpers shared by the extension integration tests. Every expectation in those tests was
//! observed against the real Token-2022 program in the Mollusk SVM, not assumed.
#![allow(dead_code)] // each test binary uses a different subset

use solana_pubkey::Pubkey;
use spl_token_2022_interface::error::TokenError;
use tokenscope_executor::{ExecReport, Harness, MintPlan, derive_address};

pub fn tok() -> Pubkey {
    spl_token_2022_interface::id()
}

/// Custom program error code of a failed report (`Failure(Custom(N))` -> `N`).
pub fn code(rep: &ExecReport) -> Option<u32> {
    let e = rep.error.as_ref()?;
    let n = e.strip_prefix("Failure(Custom(")?.strip_suffix("))")?;
    n.parse().ok()
}

pub fn log_contains(rep: &ExecReport, needle: &str) -> bool {
    rep.logs
        .iter()
        .any(|l| l.to_lowercase().contains(&needle.to_lowercase()))
}

pub const FROZEN: u32 = TokenError::AccountFrozen as u32;
pub const PAUSED: u32 = TokenError::MintPaused as u32;
pub const OWNER_MISMATCH: u32 = TokenError::OwnerMismatch as u32;
pub const INSUFFICIENT: u32 = TokenError::InsufficientFunds as u32;
pub const NON_TRANSFERABLE: u32 = TokenError::NonTransferable as u32;
pub const INVALID_COMBO: u32 = TokenError::InvalidExtensionCombination as u32;
pub const CANNOT_FREEZE: u32 = TokenError::MintCannotFreeze as u32;

/// A mint plus two funded-by-the-test token accounts (alice's `a`, bob's `b`), no balances yet.
pub struct Env {
    pub h: Harness,
    pub mint: Pubkey,
    pub alice: Pubkey,
    pub bob: Pubkey,
    pub a: Pubkey,
    pub b: Pubkey,
}

pub fn env(plan: &MintPlan) -> Env {
    let h = Harness::new();
    let (alice, bob) = (derive_address("alice"), derive_address("bob"));
    let mint = h.create_mint_ext("m", plan).expect("mint");
    let a = h
        .create_token_account("a", &mint, &alice)
        .expect("alice account");
    let b = h
        .create_token_account("b", &mint, &bob)
        .expect("bob account");
    Env {
        h,
        mint,
        alice,
        bob,
        a,
        b,
    }
}

impl Env {
    pub fn transfer(
        &self,
        from: &Pubkey,
        to: &Pubkey,
        authority: &Pubkey,
        amount: u64,
    ) -> ExecReport {
        let ix = self
            .h
            .transfer_checked_ix(from, &self.mint, to, authority, amount, 6)
            .unwrap();
        self.h.execute_logged(&ix)
    }

    pub fn amount(&self, account: &Pubkey) -> u64 {
        self.h.token_account(account).unwrap().amount
    }
}
