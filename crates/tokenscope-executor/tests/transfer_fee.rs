//! Integration tests: real Token-2022 execution in the Mollusk SVM. Expected values come from
//! the documented fee formula `min(ceil(amount * bps / 10_000), maximum_fee)`, computed here
//! independently, and are compared against decoded on-chain account state.

use tokenscope_executor::{ExecError, Harness};
use tokenscope_model::{MintSpec, TransferFeeParams};

fn expected_fee(amount: u64, bps: u16, max: u64) -> u64 {
    let raw = (u128::from(amount) * u128::from(bps)).div_ceil(10_000);
    u64::try_from(raw).unwrap().min(max)
}

struct Run {
    source_after: u64,
    dest_after: u64,
    dest_withheld: Option<u64>,
}

fn transfer(spec: MintSpec, balance: u64, amount: u64) -> Result<Run, ExecError> {
    let h = Harness::new();
    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let mint = h.create_mint("m", &spec)?;
    let src = h.create_token_account("src", &mint, &alice)?;
    let dst = h.create_token_account("dst", &mint, &bob)?;
    h.mint_to(&mint, &src, balance)?;
    let ix = h.transfer_checked_ix(&src, &mint, &dst, &alice, amount, spec.decimals)?;
    h.execute(&ix)?;
    let s = h.token_account(&src)?;
    let d = h.token_account(&dst)?;
    Ok(Run {
        source_after: s.amount,
        dest_after: d.amount,
        dest_withheld: d.withheld,
    })
}

fn fee_spec(bps: u16, max: u64) -> MintSpec {
    MintSpec {
        decimals: 6,
        transfer_fee: Some(TransferFeeParams {
            basis_points: bps,
            maximum_fee: max,
        }),
    }
}

#[test]
fn two_percent_fee_is_applied_by_real_token_2022() {
    let r = transfer(fee_spec(200, u64::MAX), 10_000, 1_000).unwrap();
    let fee = expected_fee(1_000, 200, u64::MAX);
    assert_eq!(r.source_after, 9_000, "source debited the nominal amount");
    assert_eq!(r.dest_after, 1_000 - fee, "destination credited net of fee");
    assert_eq!(
        r.dest_withheld,
        Some(fee),
        "fee tracked as withheld on destination"
    );
}

#[test]
fn fee_rounds_up_and_can_consume_entire_tiny_transfer() {
    // 1 unit at 1 bps: ceil(0.0001) = 1 -> destination receives 0.
    let r = transfer(fee_spec(1, u64::MAX), 100, 1).unwrap();
    assert_eq!(expected_fee(1, 1, u64::MAX), 1);
    assert_eq!(r.dest_after, 0);
    assert_eq!(r.dest_withheld, Some(1));
}

#[test]
fn maximum_fee_caps_the_fee() {
    let r = transfer(fee_spec(1_000, 10), 100_000, 50_000).unwrap();
    assert_eq!(expected_fee(50_000, 1_000, 10), 10);
    assert_eq!(r.dest_after, 50_000 - 10);
    assert_eq!(r.dest_withheld, Some(10));
}

#[test]
fn zero_amount_transfer_has_zero_fee() {
    let r = transfer(fee_spec(500, u64::MAX), 100, 0).unwrap();
    assert_eq!(r.dest_after, 0);
    assert_eq!(r.dest_withheld, Some(0));
    assert_eq!(r.source_after, 100);
}

#[test]
fn mint_without_fee_extension_delivers_full_amount() {
    let spec = MintSpec {
        decimals: 6,
        transfer_fee: None,
    };
    let r = transfer(spec, 10_000, 1_000).unwrap();
    assert_eq!(r.dest_after, 1_000);
    assert_eq!(r.dest_withheld, None);
}

#[test]
fn insufficient_funds_is_an_instruction_failure_not_a_setup_error() {
    let err = transfer(fee_spec(200, u64::MAX), 10, 1_000).err().unwrap();
    assert!(matches!(err, ExecError::Instruction(_)), "got {err:?}");
}

#[test]
fn mint_fee_params_are_read_from_real_mint_state() {
    let h = Harness::new();
    let mint = h.create_mint("m", &fee_spec(321, 777)).unwrap();
    assert_eq!(h.mint_fee_params(&mint, 0).unwrap(), Some((321, 777)));
}

#[test]
fn self_check_runs_real_execution() {
    let msg = tokenscope_executor::self_check().unwrap();
    assert!(msg.contains("980") && msg.contains("20"), "{msg}");
    assert_eq!(tokenscope_executor::token_2022_elf_sha256().len(), 64);
}

#[test]
fn freeze_authority_can_freeze_and_thaw_and_frozen_accounts_cannot_transfer() {
    let h = Harness::new();
    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let spec = MintSpec {
        decimals: 6,
        transfer_fee: None,
    };
    let mint = h.create_mint("m", &spec).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 100).unwrap();
    assert!(!h.token_account(&src).unwrap().frozen);

    h.execute(&h.freeze_ix(&src, &mint, true).unwrap()).unwrap();
    assert!(
        h.token_account(&src).unwrap().frozen,
        "state decoded from real account data"
    );
    let ix = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 10, 6)
        .unwrap();
    let rep = h.execute_logged(&ix);
    assert!(
        rep.error.is_some(),
        "transfer from a frozen account must fail"
    );
    assert!(
        rep.logs.iter().any(|l| l.to_lowercase().contains("frozen")),
        "{:?}",
        rep.logs
    );
    assert_eq!(
        h.token_account(&src).unwrap().amount,
        100,
        "failed transfer persisted nothing"
    );

    h.execute(&h.freeze_ix(&src, &mint, false).unwrap())
        .unwrap();
    assert!(!h.token_account(&src).unwrap().frozen);
    h.execute(&ix).unwrap();
    assert_eq!(h.token_account(&dst).unwrap().amount, 10);
}

#[test]
fn logs_of_late_instructions_are_still_captured() {
    // Regression: solana's default LogCollector drops everything after 10 KB, which silently
    // blanked the logs (and hid errors) of later steps in a long-lived harness.
    let h = Harness::new();
    let (alice, bob) = (
        tokenscope_executor::derive_address("alice"),
        tokenscope_executor::derive_address("bob"),
    );
    let spec = MintSpec {
        decimals: 6,
        transfer_fee: None,
    };
    let mint = h.create_mint("m", &spec).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 1_000_000).unwrap();
    let ix = h
        .transfer_checked_ix(&src, &mint, &dst, &alice, 1, 6)
        .unwrap();
    let first = h.execute_logged(&ix);
    assert!(!first.logs.is_empty());
    let mut total_log_bytes = 0;
    for _ in 0..200 {
        total_log_bytes += h
            .execute_logged(&ix)
            .logs
            .iter()
            .map(String::len)
            .sum::<usize>();
    }
    assert!(
        total_log_bytes > 10_000,
        "must have crossed the old 10 KB limit, got {total_log_bytes}"
    );
    let late = h.execute_logged(&ix);
    assert!(
        late.logs.iter().any(|l| l.contains("TransferChecked")),
        "late logs missing: {:?}",
        late.logs
    );
}

#[test]
fn transfer_checked_with_fee_verifies_exact_fee() {
    let h = Harness::new();
    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    let spec = fee_spec(200, u64::MAX); // 2%
    let mint = h.create_mint("m", &spec).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    let amount = 1_000;
    let expected = expected_fee(amount, 200, u64::MAX); // 20

    // Transfer with understated fee (expected - 1 = 19) must fail with FeeMismatch (32).
    let ix_under = h
        .transfer_checked_with_fee_ix(&src, &mint, &dst, &alice, amount, 6, expected - 1)
        .unwrap();
    let rep_under = h.execute_logged(&ix_under);
    assert!(
        rep_under.error.is_some(),
        "understated fee must be rejected"
    );
    assert!(
        rep_under
            .logs
            .iter()
            .any(|l| l.contains("Calculated fee does not match expected fee") || l.contains("0x20")),
        "log must indicate fee mismatch: {:?}",
        rep_under.logs
    );
    assert!(
        rep_under
            .error
            .as_deref()
            .unwrap_or("")
            .contains("Custom(32)"),
        "error must be Custom(32) (FeeMismatch 0x20): {:?}",
        rep_under.error
    );
    assert_eq!(h.token_account(&src).unwrap().amount, 10_000);

    // Transfer with overstated fee (expected + 1 = 21) must also fail.
    let ix_over = h
        .transfer_checked_with_fee_ix(&src, &mint, &dst, &alice, amount, 6, expected + 1)
        .unwrap();
    let rep_over = h.execute_logged(&ix_over);
    assert!(rep_over.error.is_some(), "overstated fee must be rejected");
    assert_eq!(h.token_account(&src).unwrap().amount, 10_000);

    // Transfer with the exact fee succeeds.
    let ix_exact = h
        .transfer_checked_with_fee_ix(&src, &mint, &dst, &alice, amount, 6, expected)
        .unwrap();
    h.execute(&ix_exact).unwrap();
    assert_eq!(h.token_account(&src).unwrap().amount, 9_000);
    assert_eq!(h.token_account(&dst).unwrap().amount, 1_000 - expected);
    assert_eq!(h.token_account(&dst).unwrap().withheld, Some(expected));
}

#[test]
fn harvest_withheld_tokens_to_mint_is_permissionless_and_works_on_frozen_accounts() {
    let h = Harness::new();
    let (alice, bob, charlie, _eve) = (
        tokenscope_executor::derive_address("alice"),
        tokenscope_executor::derive_address("bob"),
        tokenscope_executor::derive_address("charlie"),
        tokenscope_executor::derive_address("eve"),
    );
    let spec = fee_spec(200, u64::MAX); // 2%
    let mint = h.create_mint("m", &spec).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst_bob = h.create_token_account("dst_bob", &mint, &bob).unwrap();
    let dst_charlie = h
        .create_token_account("dst_charlie", &mint, &charlie)
        .unwrap();
    h.mint_to(&mint, &src, 10_000).unwrap();

    // Two transfers creating withheld fees on Bob and Charlie.
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst_bob, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst_charlie, &alice, 2_000, 6)
            .unwrap(),
    )
    .unwrap();

    assert_eq!(h.token_account(&dst_bob).unwrap().withheld, Some(20));
    assert_eq!(h.token_account(&dst_charlie).unwrap().withheld, Some(40));
    assert_eq!(h.mint_withheld(&mint).unwrap(), Some(0));

    // Freeze Charlie's account.
    h.execute(&h.freeze_ix(&dst_charlie, &mint, true).unwrap())
        .unwrap();
    assert!(h.token_account(&dst_charlie).unwrap().frozen);

    // Permissionless harvest: executes without requiring account authority signatures.
    let harvest_ix = h
        .harvest_withheld_to_mint_ix(&mint, &[&dst_bob, &dst_charlie])
        .unwrap();
    h.execute(&harvest_ix).unwrap();

    // Both accounts' withheld balances drained to the mint.
    assert_eq!(h.token_account(&dst_bob).unwrap().withheld, Some(0));
    assert_eq!(h.token_account(&dst_charlie).unwrap().withheld, Some(0));
    // Charlie is still frozen.
    assert!(h.token_account(&dst_charlie).unwrap().frozen);
    // Mint accumulated the sum of both withheld fees.
    assert_eq!(h.mint_withheld(&mint).unwrap(), Some(60));
}

#[test]
fn withdraw_withheld_tokens_enforces_withdraw_authority() {
    let h = Harness::new();
    let (alice, bob, attacker) = (
        tokenscope_executor::derive_address("alice"),
        tokenscope_executor::derive_address("bob"),
        tokenscope_executor::derive_address("attacker"),
    );
    let spec = fee_spec(200, u64::MAX); // 2%
    let mint = h.create_mint("m", &spec).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    // Fee recipient account owned by the authorized withdraw authority (the harness payer).
    let fee_recipient = h
        .create_token_account("fee_recipient", &mint, &h.payer())
        .unwrap();
    let attacker_recipient = h
        .create_token_account("attacker_recipient", &mint, &attacker)
        .unwrap();

    h.mint_to(&mint, &src, 10_000).unwrap();
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 2_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(h.token_account(&dst).unwrap().withheld, Some(40));

    // Harvest to mint so the mint holds 40 withheld tokens.
    h.execute(&h.harvest_withheld_to_mint_ix(&mint, &[&dst]).unwrap())
        .unwrap();
    assert_eq!(h.mint_withheld(&mint).unwrap(), Some(40));

    // 1. Unauthorized WithdrawWithheldTokensFromMint must fail.
    let unauth_mint_ix = h
        .withdraw_withheld_from_mint_ix(&mint, &attacker_recipient, &attacker)
        .unwrap();
    let rep = h.execute_logged(&unauth_mint_ix);
    assert!(
        rep.error.is_some(),
        "unauthorized withdraw from mint must fail"
    );
    assert_eq!(h.mint_withheld(&mint).unwrap(), Some(40));
    assert_eq!(h.token_account(&attacker_recipient).unwrap().amount, 0);

    // 2. Authorized WithdrawWithheldTokensFromMint succeeds.
    let auth_mint_ix = h
        .withdraw_withheld_from_mint_ix(&mint, &fee_recipient, &h.payer())
        .unwrap();
    h.execute(&auth_mint_ix).unwrap();
    assert_eq!(h.mint_withheld(&mint).unwrap(), Some(0));
    assert_eq!(h.token_account(&fee_recipient).unwrap().amount, 40);

    // 3. Create fresh withheld tokens directly on dst.
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(h.token_account(&dst).unwrap().withheld, Some(20));

    // 4. Unauthorized WithdrawWithheldTokensFromAccounts must fail.
    let unauth_accts_ix = h
        .withdraw_withheld_from_accounts_ix(&mint, &attacker_recipient, &attacker, &[&dst])
        .unwrap();
    let rep2 = h.execute_logged(&unauth_accts_ix);
    assert!(
        rep2.error.is_some(),
        "unauthorized direct withdraw from accounts must fail"
    );
    assert_eq!(h.token_account(&dst).unwrap().withheld, Some(20));

    // 5. Authorized WithdrawWithheldTokensFromAccounts succeeds.
    let auth_accts_ix = h
        .withdraw_withheld_from_accounts_ix(&mint, &fee_recipient, &h.payer(), &[&dst])
        .unwrap();
    h.execute(&auth_accts_ix).unwrap();
    assert_eq!(h.token_account(&dst).unwrap().withheld, Some(0));
    assert_eq!(h.token_account(&fee_recipient).unwrap().amount, 60);
}

#[test]
fn token_conservation_across_transfers_harvests_and_withdrawals() {
    let h = Harness::new();
    let (alice, bob, charlie) = (
        tokenscope_executor::derive_address("alice"),
        tokenscope_executor::derive_address("bob"),
        tokenscope_executor::derive_address("charlie"),
    );
    let spec = fee_spec(300, 100); // 3% fee, capped at 100
    let mint = h.create_mint("m", &spec).unwrap();
    let acct_a = h.create_token_account("a", &mint, &alice).unwrap();
    let acct_b = h.create_token_account("b", &mint, &bob).unwrap();
    let acct_c = h.create_token_account("c", &mint, &charlie).unwrap();
    let treasury = h
        .create_token_account("treasury", &mint, &h.payer())
        .unwrap();

    let initial_mint = 50_000;
    h.mint_to(&mint, &acct_a, initial_mint).unwrap();

    let check_conservation = |step: &str| {
        let supply = h.mint_supply(&mint).unwrap();
        let a = h.token_account(&acct_a).unwrap();
        let b = h.token_account(&acct_b).unwrap();
        let c = h.token_account(&acct_c).unwrap();
        let t = h.token_account(&treasury).unwrap();
        let m_withheld = h.mint_withheld(&mint).unwrap().unwrap_or(0);

        let total_balances = a.amount + b.amount + c.amount + t.amount;
        let total_withheld = a.withheld.unwrap_or(0)
            + b.withheld.unwrap_or(0)
            + c.withheld.unwrap_or(0)
            + t.withheld.unwrap_or(0)
            + m_withheld;

        assert_eq!(
            supply,
            total_balances + total_withheld,
            "token conservation violated at step: {step}"
        );
        assert_eq!(
            supply, initial_mint,
            "supply must remain constant at: {step}"
        );
    };

    check_conservation("initial");

    // Transfer A -> B
    h.execute(
        &h.transfer_checked_ix(&acct_a, &mint, &acct_b, &alice, 10_000, 6)
            .unwrap(),
    )
    .unwrap();
    check_conservation("after transfer A->B");

    // Transfer B -> C
    h.execute(
        &h.transfer_checked_ix(&acct_b, &mint, &acct_c, &bob, 4_000, 6)
            .unwrap(),
    )
    .unwrap();
    check_conservation("after transfer B->C");

    // Harvest B & C to mint
    h.execute(
        &h.harvest_withheld_to_mint_ix(&mint, &[&acct_b, &acct_c])
            .unwrap(),
    )
    .unwrap();
    check_conservation("after harvest to mint");

    // Withdraw from mint to treasury
    h.execute(
        &h.withdraw_withheld_from_mint_ix(&mint, &treasury, &h.payer())
            .unwrap(),
    )
    .unwrap();
    check_conservation("after withdraw from mint to treasury");

    // Transfer C -> A (generates withheld on A)
    h.execute(
        &h.transfer_checked_ix(&acct_c, &mint, &acct_a, &charlie, 2_000, 6)
            .unwrap(),
    )
    .unwrap();
    check_conservation("after transfer C->A");

    // Direct withdraw from accounts to treasury
    h.execute(
        &h.withdraw_withheld_from_accounts_ix(&mint, &treasury, &h.payer(), &[&acct_a])
            .unwrap(),
    )
    .unwrap();
    check_conservation("after direct withdraw from accounts");
}

#[test]
fn epoch_delayed_fee_changes_apply_two_epochs_later() {
    let mut h = Harness::new();
    let alice = tokenscope_executor::derive_address("alice");
    let bob = tokenscope_executor::derive_address("bob");
    // Initial: 200 bps (2%), max 1,000
    let spec = fee_spec(200, 1_000);
    let mint = h.create_mint("m", &spec).unwrap();
    let src = h.create_token_account("src", &mint, &alice).unwrap();
    let dst = h.create_token_account("dst", &mint, &bob).unwrap();
    h.mint_to(&mint, &src, 100_000).unwrap();

    assert_eq!(h.epoch(), 0);
    assert_eq!(h.mint_fee_params(&mint, 0).unwrap(), Some((200, 1_000)));

    // Base transfer at epoch 0: 2% of 1,000 = 20
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(h.token_account(&dst).unwrap().amount, 980);
    assert_eq!(h.token_account(&dst).unwrap().withheld, Some(20));

    // Phase 1: RAISE fee from 200 bps to 500 bps (5%), max 2,000
    h.execute(
        &h.set_transfer_fee_ix(&mint, &h.payer(), 500, 2_000)
            .unwrap(),
    )
    .unwrap();

    // At epoch 0: old fee (200 bps) still applies to transfers
    let d_before = h.token_account(&dst).unwrap().amount;
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        h.token_account(&dst).unwrap().amount - d_before,
        980,
        "epoch 0 still 200 bps"
    );

    // Warp to epoch 1: old fee (200 bps) STILL applies (only 1 epoch passed)
    h.warp_to_epoch(1);
    assert_eq!(h.epoch(), 1);
    let d_before_1 = h.token_account(&dst).unwrap().amount;
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        h.token_account(&dst).unwrap().amount - d_before_1,
        980,
        "epoch 1 still 200 bps"
    );

    // Warp to epoch 2: new fee (500 bps = 50 tokens) takes effect!
    h.warp_to_epoch(2);
    assert_eq!(h.epoch(), 2);
    let d_before_2 = h.token_account(&dst).unwrap().amount;
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        h.token_account(&dst).unwrap().amount - d_before_2,
        950,
        "epoch 2 must be 500 bps"
    );

    // Phase 2: LOWER fee from 500 bps to 100 bps (1%), max 500 at epoch 2
    h.execute(&h.set_transfer_fee_ix(&mint, &h.payer(), 100, 500).unwrap())
        .unwrap();

    // At epoch 2: 500 bps still active
    let d_before_low_0 = h.token_account(&dst).unwrap().amount;
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        h.token_account(&dst).unwrap().amount - d_before_low_0,
        950,
        "epoch 2 still 500 bps"
    );

    // Warp to epoch 3: 500 bps still active
    h.warp_to_epoch(3);
    assert_eq!(h.epoch(), 3);
    let d_before_low_1 = h.token_account(&dst).unwrap().amount;
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        h.token_account(&dst).unwrap().amount - d_before_low_1,
        950,
        "epoch 3 still 500 bps"
    );

    // Warp to epoch 4: new lower fee (100 bps = 10 tokens) takes effect!
    h.warp_to_epoch(4);
    assert_eq!(h.epoch(), 4);
    let d_before_low_2 = h.token_account(&dst).unwrap().amount;
    h.execute(
        &h.transfer_checked_ix(&src, &mint, &dst, &alice, 1_000, 6)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        h.token_account(&dst).unwrap().amount - d_before_low_2,
        990,
        "epoch 4 must be 100 bps"
    );
}

#[test]
fn mint_withheld_and_epoch_helpers_behave_consistently() {
    let mut h = Harness::new();
    assert_eq!(h.epoch(), 0);
    h.warp_to_epoch(42);
    assert_eq!(h.epoch(), 42);

    let fee_mint = h.create_mint("fee_mint", &fee_spec(100, 500)).unwrap();
    assert_eq!(h.mint_withheld(&fee_mint).unwrap(), Some(0));

    let no_fee_spec = MintSpec {
        decimals: 6,
        transfer_fee: None,
    };
    let plain_mint = h.create_mint("plain_mint", &no_fee_spec).unwrap();
    assert_eq!(h.mint_withheld(&plain_mint).unwrap(), None);
}
