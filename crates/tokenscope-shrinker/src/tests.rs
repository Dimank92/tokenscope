use tokenscope_model::{Action, MintSpec, SCHEMA_VERSION, Scenario, TransferFeeParams};

use crate::*;

fn scenario(actions: Vec<Action>) -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: "big".into(),
        seed: 3,
        target: "t".into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: Some(TransferFeeParams {
                basis_points: 777,
                maximum_fee: 123_456,
            }),
        },
        initial_balance: 9_999_999,
        extensions: vec![],
        actions,
    }
}

fn many() -> Scenario {
    scenario(vec![
        Action::Withdraw,
        Action::Deposit { amount: 50_000 },
        Action::Deposit { amount: 3 },
        Action::Withdraw,
        Action::Deposit { amount: 4_000 },
    ])
}

/// Failure: some deposit of at least 1000 exists and a fee is configured.
fn big_deposit_with_fee(s: &Scenario) -> bool {
    s.mint.transfer_fee.is_some()
        && s.actions
            .iter()
            .any(|a| matches!(a, Action::Deposit { amount } if *amount >= 1_000))
}

#[test]
fn shrinks_to_the_boundary_and_removes_everything_irrelevant() {
    let r = shrink(&many(), ShrinkConfig::default(), big_deposit_with_fee).unwrap();
    let m = &r.minimized;
    assert_eq!(m.actions, vec![Action::Deposit { amount: 1_000 }]);
    assert_eq!(m.initial_balance, 0);
    assert_eq!(m.mint.decimals, 0);
    assert_eq!(
        m.mint.transfer_fee,
        Some(TransferFeeParams {
            basis_points: 0,
            maximum_fee: 0
        })
    );
    assert!(big_deposit_with_fee(m));
    assert!(m.validate().is_ok());
    assert!(!r.budget_exhausted && r.accepted > 0);
    assert_eq!(m.name, "big-min");
}

#[test]
fn preserves_the_failure_predicate() {
    // Failure needs the fee extension AND a withdraw after a deposit.
    let pred = |s: &Scenario| {
        s.mint.transfer_fee.is_some()
            && s.actions
                .windows(2)
                .any(|w| matches!(w, [Action::Deposit { .. }, Action::Withdraw]))
    };
    let r = shrink(&many(), ShrinkConfig::default(), pred).unwrap();
    assert!(pred(&r.minimized));
    assert_eq!(r.minimized.actions.len(), 2);
}

#[test]
fn not_reproducible_failure_is_reported() {
    assert_eq!(
        shrink(&many(), ShrinkConfig::default(), |_| false),
        Err(ShrinkError::NotReproducible)
    );
}

#[test]
fn invalid_input_is_rejected() {
    let mut s = many();
    s.actions.clear();
    assert!(matches!(
        shrink(&s, ShrinkConfig::default(), |_| true),
        Err(ShrinkError::InvalidInput(_))
    ));
}

#[test]
fn budget_bounds_work_and_result_is_still_valid_and_failing() {
    let mut evals = 0;
    let r = shrink(&many(), ShrinkConfig { max_attempts: 5 }, |s| {
        evals += 1;
        big_deposit_with_fee(s)
    })
    .unwrap();
    assert!(r.budget_exhausted);
    assert!(r.attempts <= 5);
    assert!(evals <= 5 + 2, "bookend checks only");
    assert!(big_deposit_with_fee(&r.minimized) && r.minimized.validate().is_ok());
}

#[test]
fn shrinking_is_deterministic() {
    let a = shrink(&many(), ShrinkConfig::default(), big_deposit_with_fee).unwrap();
    let b = shrink(&many(), ShrinkConfig::default(), big_deposit_with_fee).unwrap();
    assert_eq!(a, b);
}

#[test]
fn never_produces_an_empty_action_list() {
    let r = shrink(&many(), ShrinkConfig::default(), |s| {
        s.mint.transfer_fee.is_some()
    })
    .unwrap();
    assert_eq!(r.minimized.actions.len(), 1);
}

#[test]
fn nondeterministic_predicate_is_detected() {
    let mut calls = 0;
    let r = shrink(&many(), ShrinkConfig::default(), |_| {
        calls += 1;
        calls == 1 // true only on the initial check
    });
    assert_eq!(r, Err(ShrinkError::LostFailure));
}

#[test]
fn candidate_generation_is_strictly_decreasing() {
    assert_eq!(candidates(0), Vec::<u64>::new());
    assert_eq!(candidates(1), vec![0]);
    assert_eq!(candidates(100), vec![0, 1, 50, 75, 88, 94, 97, 99]);
}

mod prop {
    use proptest::prelude::*;
    use tokenscope_model::{Action, MintSpec, SCHEMA_VERSION, Scenario, TransferFeeParams};

    use crate::*;

    fn arb() -> impl Strategy<Value = Scenario> {
        (
            proptest::collection::vec(
                prop_oneof![
                    (0u64..1_000_000).prop_map(|amount| Action::Deposit { amount }),
                    Just(Action::Withdraw)
                ],
                1..12,
            ),
            0u64..1_000_000,
            proptest::option::of((0u16..=10_000, 0u64..1_000_000)),
        )
            .prop_map(|(actions, initial_balance, fee)| Scenario {
                schema_version: SCHEMA_VERSION,
                name: "p".into(),
                seed: 0,
                target: "t".into(),
                mint: MintSpec {
                    decimals: 6,
                    transfer_fee: fee.map(|(basis_points, maximum_fee)| TransferFeeParams {
                        basis_points,
                        maximum_fee,
                    }),
                },
                initial_balance,
                extensions: vec![],
                actions,
            })
    }

    proptest! {
        /// For a threshold failure ("some deposit >= t"), shrinking keeps the failure, stays valid,
        /// never grows the input, and lands exactly on the threshold.
        #[test]
        fn shrinking_preserves_failure_and_never_grows(s in arb(), t in 1u64..500_000) {
            let pred = |x: &Scenario| x.actions.iter().any(|a| matches!(a, Action::Deposit { amount } if *amount >= t));
            if pred(&s) {
                let r = shrink(&s, ShrinkConfig { max_attempts: 5_000 }, pred).unwrap();
                prop_assert!(pred(&r.minimized));
                prop_assert!(r.minimized.validate().is_ok());
                prop_assert!(r.minimized.actions.len() <= s.actions.len());
                prop_assert!(r.minimized.initial_balance <= s.initial_balance);
                prop_assert_eq!(r.minimized.actions.len(), 1);
                prop_assert_eq!(r.minimized.actions[0], Action::Deposit { amount: t });
            } else {
                prop_assert_eq!(shrink(&s, ShrinkConfig::default(), pred), Err(ShrinkError::NotReproducible));
            }
        }
    }
}

mod extensions {
    use tokenscope_model::{Action, MintExtension, MintSpec, SCHEMA_VERSION, Scenario};

    use crate::*;

    fn noisy() -> Scenario {
        Scenario {
            schema_version: SCHEMA_VERSION,
            name: "e".into(),
            seed: 1,
            target: "t".into(),
            mint: MintSpec {
                decimals: 6,
                transfer_fee: None,
            },
            initial_balance: 500,
            extensions: vec![
                MintExtension::Pausable,
                MintExtension::PermanentDelegate,
                MintExtension::TransferHook,
            ],
            actions: vec![
                Action::Pause,
                Action::Deposit { amount: 900 },
                Action::PermanentDelegateDrain { amount: 700 },
                Action::Resume,
            ],
        }
    }

    // Failure: a permanent delegate exists and a drain of at least 10 follows a deposit.
    fn pred(s: &Scenario) -> bool {
        let drained = s
            .actions
            .iter()
            .position(|a| matches!(a, Action::PermanentDelegateDrain { amount } if *amount >= 10));
        let deposited = s
            .actions
            .iter()
            .position(|a| matches!(a, Action::Deposit { .. }));
        s.extensions.contains(&MintExtension::PermanentDelegate)
            && matches!((deposited, drained), (Some(d), Some(x)) if d < x)
    }

    #[test]
    fn unneeded_extensions_actions_and_amounts_are_removed() {
        let r = shrink(&noisy(), ShrinkConfig::default(), pred).unwrap();
        let m = &r.minimized;
        assert_eq!(m.extensions, vec![MintExtension::PermanentDelegate]);
        assert_eq!(
            m.actions,
            vec![
                Action::Deposit { amount: 0 },
                Action::PermanentDelegateDrain { amount: 10 }
            ]
        );
        assert_eq!(m.initial_balance, 0);
        assert!(pred(m) && m.validate().is_ok());
    }
}
