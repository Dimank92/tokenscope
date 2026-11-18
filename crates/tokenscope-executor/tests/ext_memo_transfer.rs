//! Integration tests: Token-2022 Required Transfer Memos on the real Mollusk SVM.
//!
//! Verified behaviors:
//! 1. A token account with `MemoTransfer` enabled rejects transfers that are not preceded
//!    by an SPL Memo program instruction (custom error 38, NoTransferMemos).
//! 2. Multi-instruction transaction execution: a transaction containing `[memo, transfer]` succeeds.
//! 3. Finding: an **empty memo** (`""` / `&[]`) satisfies the requirement.
//! 4. Instruction ordering matters: putting the memo after the transfer fails.
//! 5. Atomic rollback: a failure in any instruction in a transaction rolls back all state
//!    changes made by earlier instructions in that transaction.
//! 6. Disabling `MemoTransfer` allows transfers without memos once again.

use solana_instruction::Instruction;
use tokenscope_executor::Harness;
use tokenscope_model::MintSpec;

fn plain_spec() -> MintSpec {
    MintSpec {
        decimals: 6,
        transfer_fee: None,
    }
}

fn memo_ix(memo: &str) -> Instruction {
    Instruction {
        program_id: mollusk_svm_programs_memo::memo::ID,
        accounts: vec![],
        data: memo.as_bytes().to_vec(),
    }
}

#[test]
fn transfer_without_memo_to_memo_required_account_fails() {
    let mut h = Harness::new();
    mollusk_svm_programs_memo::memo::add_program(h.mollusk_mut());

    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let mint = h.create_mint("m", &plain_spec()).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    // Enable MemoTransfer on destination
    let enable_ixs = h.memo_transfer_instructions(&dst, &bob, true).unwrap();
    for ix in &enable_ixs {
        h.execute(ix).unwrap();
    }
    assert_eq!(h.token_memo_transfer(&dst).unwrap(), Some(true));

    // Plain transfer without memo must fail
    let transfer_ix = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
        .unwrap();
    let rep = h.execute_logged(&transfer_ix);
    assert!(rep.error.is_some(), "transfer without memo must fail");
    assert!(
        rep.logs
            .iter()
            .any(|l| l.contains("No memo in previous instruction") || l.contains("0x24")),
        "log must indicate NoTransferMemos (0x24): {:?}",
        rep.logs
    );
    assert_eq!(h.token_account(&src).unwrap().amount, 10_000);
    assert_eq!(h.token_account(&dst).unwrap().amount, 0);
}

#[test]
fn transaction_with_preceding_memo_succeeds() {
    let mut h = Harness::new();
    mollusk_svm_programs_memo::memo::add_program(h.mollusk_mut());

    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let mint = h.create_mint("m", &plain_spec()).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    let enable_ixs = h.memo_transfer_instructions(&dst, &bob, true).unwrap();
    for ix in &enable_ixs {
        h.execute(ix).unwrap();
    }

    let transfer_ix = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
        .unwrap();
    let txn = [memo_ix("payment for invoice 42"), transfer_ix];

    let rep = h.execute_transaction_logged(&txn);
    assert!(
        rep.error.is_none(),
        "transaction with memo must succeed: {:?}",
        rep.error
    );
    assert_eq!(h.token_account(&src).unwrap().amount, 9_000);
    assert_eq!(h.token_account(&dst).unwrap().amount, 1_000);
}

#[test]
fn empty_memo_satisfies_requirement() {
    let mut h = Harness::new();
    mollusk_svm_programs_memo::memo::add_program(h.mollusk_mut());

    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let mint = h.create_mint("m", &plain_spec()).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    let enable_ixs = h.memo_transfer_instructions(&dst, &bob, true).unwrap();
    for ix in &enable_ixs {
        h.execute(ix).unwrap();
    }

    // An empty memo (zero bytes of data) is accepted by the on-chain runtime
    let transfer_ix = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 500, 6)
        .unwrap();
    let txn = [memo_ix(""), transfer_ix];

    let rep = h.execute_transaction_logged(&txn);
    assert!(
        rep.error.is_none(),
        "empty memo must satisfy MemoTransfer requirement: {:?}",
        rep.error
    );
    assert_eq!(h.token_account(&src).unwrap().amount, 9_500);
    assert_eq!(h.token_account(&dst).unwrap().amount, 500);
}

#[test]
fn instruction_ordering_matters() {
    let mut h = Harness::new();
    mollusk_svm_programs_memo::memo::add_program(h.mollusk_mut());

    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let mint = h.create_mint("m", &plain_spec()).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    let enable_ixs = h.memo_transfer_instructions(&dst, &bob, true).unwrap();
    for ix in &enable_ixs {
        h.execute(ix).unwrap();
    }

    // Putting the memo AFTER the transfer: the transfer executes first, finds no preceding memo,
    // and fails.
    let transfer_ix = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
        .unwrap();
    let txn = [transfer_ix, memo_ix("too late")];

    let rep = h.execute_transaction_logged(&txn);
    assert!(rep.error.is_some(), "transfer placed before memo must fail");
    assert_eq!(
        h.token_account(&src).unwrap().amount,
        10_000,
        "failed transaction must leave source balance intact"
    );
    assert_eq!(
        h.token_account(&dst).unwrap().amount,
        0,
        "failed transaction must leave destination balance intact"
    );
}

#[test]
fn transaction_failure_rolls_back_entire_state_atomically() {
    let mut h = Harness::new();
    mollusk_svm_programs_memo::memo::add_program(h.mollusk_mut());

    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let mint = h.create_mint("m", &plain_spec()).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    let enable_ixs = h.memo_transfer_instructions(&dst, &bob, true).unwrap();
    for ix in &enable_ixs {
        h.execute(ix).unwrap();
    }

    // Step 1: Valid memo.
    // Step 2: Valid transfer (1,000 tokens).
    // Step 3: Failing transfer (1,000,000 tokens, exceeds Alice's balance).
    let valid_transfer = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
        .unwrap();
    let failing_transfer = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 1_000_000, 6)
        .unwrap();

    let txn = [memo_ix("valid memo"), valid_transfer, failing_transfer];

    let rep = h.execute_transaction_logged(&txn);
    assert!(
        rep.error.is_some(),
        "transaction containing failing instruction must fail"
    );

    // Atomic rollback verification: despite Step 2 being valid in isolation,
    // the failure of Step 3 rolls back the ENTIRE transaction.
    assert_eq!(
        h.token_account(&src).unwrap().amount,
        10_000,
        "source balance must remain untouched due to atomic rollback"
    );
    assert_eq!(
        h.token_account(&dst).unwrap().amount,
        0,
        "destination balance must remain 0 due to atomic rollback"
    );
}

#[test]
fn disabling_memo_transfers_allows_plain_transfers() {
    let mut h = Harness::new();
    mollusk_svm_programs_memo::memo::add_program(h.mollusk_mut());

    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let mint = h.create_mint("m", &plain_spec()).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    // 1. Enable MemoTransfer
    for ix in &h.memo_transfer_instructions(&dst, &bob, true).unwrap() {
        h.execute(ix).unwrap();
    }
    assert_eq!(h.token_memo_transfer(&dst).unwrap(), Some(true));

    // Plain transfer rejected
    let transfer_ix = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
        .unwrap();
    assert!(h.execute(&transfer_ix).is_err());

    // 2. Disable MemoTransfer
    for ix in &h.memo_transfer_instructions(&dst, &bob, false).unwrap() {
        h.execute(ix).unwrap();
    }
    assert_eq!(h.token_memo_transfer(&dst).unwrap(), Some(false));

    // Plain transfer now succeeds
    h.execute(&transfer_ix).unwrap();
    assert_eq!(h.token_account(&src).unwrap().amount, 9_000);
    assert_eq!(h.token_account(&dst).unwrap().amount, 1_000);
}
