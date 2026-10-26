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
