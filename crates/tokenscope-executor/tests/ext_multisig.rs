//! Integration tests: Token-2022 multisig authorities on the real Mollusk SVM.
//!
//! Verified behaviors:
//! 1. Threshold signing: an M-of-N multisig authorizes token operations only when at least
//!    M distinct member signatures are provided.
//! 2. Duplicate and unknown signers do not count towards the threshold.
//! 3. Re-initialization of an already initialized multisig account is rejected with `AlreadyInUse`.
//! 4. Client-side vs on-chain discrepancy: client builders reject invalid thresholds before
//!    transmission; raw instructions reveal on-chain runtime behavior:
//!    - m = 0 is rejected by the runtime (custom error 8, InvalidNumberOfRequiredSigners).
//!    - n = 12 (> MAX_SIGNERS 11) is rejected by the runtime (custom error 7, InvalidNumberOfProvidedSigners).
//!    - m > n is accepted by the runtime, creating an unusable multisig that permanently locks funds.
//! 5. Fund locking: accounts owned by an m > n multisig can never satisfy the threshold.

use solana_instruction::{AccountMeta, Instruction};
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use spl_token_2022_interface::{
    instruction::{self as token_ix, TokenInstruction},
    state::Multisig,
};
use tokenscope_executor::Harness;
use tokenscope_model::MintSpec;

fn plain_spec() -> MintSpec {
    MintSpec {
        decimals: 6,
        transfer_fee: None,
    }
}

#[test]
fn threshold_signing_requires_at_least_m_distinct_signers() {
    let h = Harness::new();
    let s1 = tokenscope_executor::derive_address("signer_1");
    let s2 = tokenscope_executor::derive_address("signer_2");
    let s3 = tokenscope_executor::derive_address("signer_3");
    let bob = tokenscope_executor::derive_address("bob");

    let mint = h.create_mint("m", &plain_spec()).unwrap();
    let multisig = h.create_multisig("2of3", &[&s1, &s2, &s3], 2).unwrap();
    let src = h.create_token_account("src", &mint, &multisig).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    // 0 signers: rejected
    let ix_0 = h
        .transfer_checked_multisig_ix(&src, &mint, &dst, &multisig, &[], 100, 6)
        .unwrap();
    let rep_0 = h.execute_logged(&ix_0);
    assert!(rep_0.error.is_some(), "0 signers must fail");
    assert_eq!(h.token_account(&src).unwrap().amount, 10_000);

    // 1 signer (< 2 threshold): rejected
    let ix_1 = h
        .transfer_checked_multisig_ix(&src, &mint, &dst, &multisig, &[&s1], 100, 6)
        .unwrap();
    let rep_1 = h.execute_logged(&ix_1);
    assert!(
        rep_1.error.is_some(),
        "1 of 2 threshold must fail: {:?}",
        rep_1.error
    );
    assert_eq!(h.token_account(&src).unwrap().amount, 10_000);

    // 2 signers (>= 2 threshold): succeeds
    let ix_2 = h
        .transfer_checked_multisig_ix(&src, &mint, &dst, &multisig, &[&s1, &s2], 100, 6)
        .unwrap();
    h.execute(&ix_2).unwrap();
    assert_eq!(h.token_account(&src).unwrap().amount, 9_900);
    assert_eq!(h.token_account(&dst).unwrap().amount, 100);

    // 3 signers (all members): succeeds
    let ix_3 = h
        .transfer_checked_multisig_ix(&src, &mint, &dst, &multisig, &[&s1, &s2, &s3], 200, 6)
        .unwrap();
    h.execute(&ix_3).unwrap();
    assert_eq!(h.token_account(&src).unwrap().amount, 9_700);
    assert_eq!(h.token_account(&dst).unwrap().amount, 300);
}

#[test]
fn duplicate_and_unknown_signers_do_not_satisfy_threshold() {
    let h = Harness::new();
    let s1 = tokenscope_executor::derive_address("signer_1");
    let s2 = tokenscope_executor::derive_address("signer_2");
    let s3 = tokenscope_executor::derive_address("signer_3");
    let unknown1 = tokenscope_executor::derive_address("unknown_1");
    let unknown2 = tokenscope_executor::derive_address("unknown_2");
    let bob = tokenscope_executor::derive_address("bob");

    let mint = h.create_mint("m", &plain_spec()).unwrap();
    let multisig = h.create_multisig("2of3", &[&s1, &s2, &s3], 2).unwrap();
    let src = h.create_token_account("src", &mint, &multisig).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    // Duplicate signers: [s1, s1] provides 2 signer slots, but only 1 unique member
    let ix_dup = h
        .transfer_checked_multisig_ix(&src, &mint, &dst, &multisig, &[&s1, &s1], 100, 6)
        .unwrap();
    let rep_dup = h.execute_logged(&ix_dup);
    assert!(
        rep_dup.error.is_some(),
        "duplicate signer must not satisfy 2-of-3 threshold: {:?}",
        rep_dup.error
    );
    assert_eq!(h.token_account(&src).unwrap().amount, 10_000);

    // Partial valid + unknown signer: [s1, unknown1]
    let ix_part_unk = h
        .transfer_checked_multisig_ix(&src, &mint, &dst, &multisig, &[&s1, &unknown1], 100, 6)
        .unwrap();
    let rep_part_unk = h.execute_logged(&ix_part_unk);
    assert!(
        rep_part_unk.error.is_some(),
        "unknown signer must not count towards threshold: {:?}",
        rep_part_unk.error
    );
    assert_eq!(h.token_account(&src).unwrap().amount, 10_000);

    // Exclusively unknown signers: [unknown1, unknown2]
    let ix_all_unk = h
        .transfer_checked_multisig_ix(
            &src,
            &mint,
            &dst,
            &multisig,
            &[&unknown1, &unknown2],
            100,
            6,
        )
        .unwrap();
    let rep_all_unk = h.execute_logged(&ix_all_unk);
    assert!(
        rep_all_unk.error.is_some(),
        "all unknown signers must fail: {:?}",
        rep_all_unk.error
    );
    assert_eq!(h.token_account(&src).unwrap().amount, 10_000);
}

#[test]
fn reinitialization_of_multisig_is_rejected() {
    let h = Harness::new();
    let s1 = tokenscope_executor::derive_address("s1");
    let s2 = tokenscope_executor::derive_address("s2");
    let multisig = h.create_multisig("multi", &[&s1, &s2], 2).unwrap();

    // Attempting to re-initialize with InitializeMultisig2
    let token = spl_token_2022_interface::id();
    let ix_reinit = token_ix::initialize_multisig2(&token, &multisig, &[&s1, &s2], 1).unwrap();
    let rep = h.execute_logged(&ix_reinit);
    assert!(rep.error.is_some(), "re-initializing multisig must fail");
    assert!(
        rep.logs
            .iter()
            .any(|l| l.contains("already in use") || l.contains("0x6")),
        "log must indicate account already in use: {:?}",
        rep.logs
    );

    // State remains intact
    let ms = h.multisig(&multisig).unwrap();
    assert_eq!(ms.m, 2, "threshold must remain unchanged");
    assert_eq!(ms.n, 2, "member count must remain unchanged");
    assert!(ms.is_initialized);
}

#[test]
fn raw_instruction_threshold_checks_and_builder_discrepancy() {
    let h = Harness::new();
    let token = spl_token_2022_interface::id();
    let s1 = tokenscope_executor::derive_address("s1");
    let s2 = tokenscope_executor::derive_address("s2");

    // 1. Client-side builder validation: initialize_multisig2 rejects m = 0 before execution
    let builder_err_m0 = token_ix::initialize_multisig2(&token, &s1, &[&s1, &s2], 0).err();
    assert!(
        builder_err_m0.is_some(),
        "instruction builder must reject m=0 client-side"
    );

    // 2. Client-side builder validation: initialize_multisig2 rejects m > n client-side
    let builder_err_mn = token_ix::initialize_multisig2(&token, &s1, &[&s1, &s2], 3).err();
    assert!(
        builder_err_mn.is_some(),
        "instruction builder must reject m>n client-side"
    );

    // 3. Raw instruction: m = 0 sent directly to the on-chain runtime
    let multi_m0 = h
        .create_owned_account("multi_m0", Multisig::LEN, &token)
        .unwrap();
    let ix_raw_m0 = Instruction {
        program_id: token,
        accounts: vec![
            AccountMeta::new(multi_m0, false),
            AccountMeta::new_readonly(s1, false),
            AccountMeta::new_readonly(s2, false),
        ],
        data: TokenInstruction::InitializeMultisig2 { m: 0 }.pack(),
    };
    let rep_m0 = h.execute_logged(&ix_raw_m0);
    assert!(
        rep_m0.error.is_some(),
        "runtime must reject m=0 on-chain: {:?}",
        rep_m0.error
    );
    assert!(
        rep_m0
            .logs
            .iter()
            .any(|l| l.contains("Invalid number of required signers") || l.contains("0x8")),
        "log must indicate invalid required signers (code 8): {:?}",
        rep_m0.logs
    );

    // 4. Raw instruction: n = 12 (> MAX_SIGNERS 11) sent directly to runtime
    let multi_n12 = h
        .create_owned_account("multi_n12", Multisig::LEN, &token)
        .unwrap();
    let mut accounts_n12 = vec![AccountMeta::new(multi_n12, false)];
    for i in 0..12 {
        accounts_n12.push(AccountMeta::new_readonly(
            tokenscope_executor::derive_address(&format!("signer_{i}")),
            false,
        ));
    }
    let ix_raw_n12 = Instruction {
        program_id: token,
        accounts: accounts_n12,
        data: TokenInstruction::InitializeMultisig2 { m: 2 }.pack(),
    };
    let rep_n12 = h.execute_logged(&ix_raw_n12);
    assert!(
        rep_n12.error.is_some(),
        "runtime must reject n=12 on-chain: {:?}",
        rep_n12.error
    );
    assert!(
        rep_n12
            .logs
            .iter()
            .any(|l| l.contains("Invalid number of provided signers") || l.contains("0x7")),
        "log must indicate invalid provided signers (code 7): {:?}",
        rep_n12.logs
    );

    // 5. Raw instruction: m > n (m=3, n=2) IS ACCEPTED by the runtime!
    let multi_mn = h
        .create_owned_account("multi_mn", Multisig::LEN, &token)
        .unwrap();
    let ix_raw_mn = Instruction {
        program_id: token,
        accounts: vec![
            AccountMeta::new(multi_mn, false),
            AccountMeta::new_readonly(s1, false),
            AccountMeta::new_readonly(s2, false),
        ],
        data: TokenInstruction::InitializeMultisig2 { m: 3 }.pack(),
    };
    let rep_mn = h.execute_logged(&ix_raw_mn);
    assert!(
        rep_mn.error.is_none(),
        "runtime unexpectedly rejected m>n: {:?}",
        rep_mn.error
    );
    let ms = h.multisig(&multi_mn).unwrap();
    assert_eq!(ms.m, 3);
    assert_eq!(ms.n, 2);
    assert!(ms.is_initialized);
}

#[test]
fn unusable_multisig_with_m_greater_than_n_locks_funds() {
    let h = Harness::new();
    let token = spl_token_2022_interface::id();
    let s1 = tokenscope_executor::derive_address("signer_1");
    let s2 = tokenscope_executor::derive_address("signer_2");
    let bob = tokenscope_executor::derive_address("bob");

    let mint = h.create_mint("m", &plain_spec()).unwrap();

    // Create an unusable multisig with m=3, n=2 via raw instruction (runtime accepts this)
    let multi_locked = h
        .create_owned_account("multi_locked", Multisig::LEN, &token)
        .unwrap();
    let ix_init = Instruction {
        program_id: token,
        accounts: vec![
            AccountMeta::new(multi_locked, false),
            AccountMeta::new_readonly(s1, false),
            AccountMeta::new_readonly(s2, false),
        ],
        data: TokenInstruction::InitializeMultisig2 { m: 3 }.pack(),
    };
    h.execute(&ix_init).unwrap();

    // Create a token account owned by this unusable multisig and fund it
    let vault = h
        .create_token_account("vault", &mint, &multi_locked)
        .unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &vault, 50_000).unwrap();
    assert_eq!(h.token_account(&vault).unwrap().amount, 50_000);

    // Attempting to transfer with ALL member signers (s1 and s2) must fail:
    // the multisig requires 3 signers, but only 2 members exist!
    let ix_transfer = h
        .transfer_checked_multisig_ix(&vault, &mint, &dst, &multi_locked, &[&s1, &s2], 1_000, 6)
        .unwrap();
    let rep = h.execute_logged(&ix_transfer);
    assert!(
        rep.error.is_some(),
        "transfer from m>n multisig must fail even with all signers"
    );
    assert!(
        rep.logs
            .iter()
            .any(|l| l.contains("MissingRequiredSignature")),
        "log must indicate missing signature: {:?}",
        rep.logs
    );

    // The funds remain locked in the vault
    assert_eq!(
        h.token_account(&vault).unwrap().amount,
        50_000,
        "vault balance must remain unchanged"
    );
    assert_eq!(
        h.token_account(&dst).unwrap().amount,
        0,
        "destination balance must remain 0"
    );
}

#[test]
fn multisig_account_decoding_and_state_verification() {
    let h = Harness::new();
    let s1 = tokenscope_executor::derive_address("alice");
    let s2 = tokenscope_executor::derive_address("bob");
    let s3 = tokenscope_executor::derive_address("charlie");

    let multisig = h.create_multisig("3of3", &[&s1, &s2, &s3], 3).unwrap();
    let ms = h.multisig(&multisig).unwrap();

    assert_eq!(ms.m, 3);
    assert_eq!(ms.n, 3);
    assert!(ms.is_initialized);

    let decoded_signers: Vec<Pubkey> = ms.signers[..ms.n as usize]
        .iter()
        .map(|addr| Pubkey::new_from_array(addr.to_bytes()))
        .collect();
    assert_eq!(decoded_signers, vec![s1, s2, s3]);
}
