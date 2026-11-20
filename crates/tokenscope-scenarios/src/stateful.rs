//! Model-based stateful scenario generation using [`TokenReferenceModel`].
//!
//! Generates action sequences where candidate actions are chosen based on the simulated
//! state machine (balance, vault contents, pause, freeze, allowance).

use tokenscope_model::{
    Action, MintExtension, MintSpec, SCHEMA_VERSION, Scenario, TokenReferenceModel,
    TransferFeeParams,
};

use crate::{
    SplitMix64,
    guided::{BOUNDARY_BPS, BOUNDARY_MAX_FEE, rounding_boundaries},
};

fn scenario_seed(seed: u64, index: u64) -> u64 {
    SplitMix64::new(seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15)).next_u64()
}

fn pick<T: Copy>(rng: &mut SplitMix64, items: &[T]) -> T {
    items[rng.below(items.len() as u64) as usize]
}

/// Generate stateful scenarios where action sequences depend on simulated state.
pub fn generate_stateful(seed: u64, count: usize, target: &str) -> Vec<Scenario> {
    (0..count as u64)
        .map(|i| stateful_one(scenario_seed(seed, i), i, target))
        .collect()
}

fn stateful_one(seed: u64, index: u64, target: &str) -> Scenario {
    let mut rng = SplitMix64::new(seed);
    let initial_balance = 1_000 + rng.below(1_000_000);

    let fee = if rng.below(3) != 0 {
        Some(TransferFeeParams {
            basis_points: pick(&mut rng, &BOUNDARY_BPS),
            maximum_fee: pick(&mut rng, &BOUNDARY_MAX_FEE),
        })
    } else {
        None
    };

    let mut extensions = Vec::new();
    let candidates = [
        MintExtension::Pausable,
        MintExtension::DefaultAccountState { frozen: true },
        MintExtension::PermanentDelegate,
        MintExtension::TransferHook,
    ];
    for ext in candidates {
        if rng.below(4) == 0 {
            extensions.push(ext);
        }
    }

    let mut scenario = Scenario {
        schema_version: SCHEMA_VERSION,
        name: format!("stateful-{index}"),
        seed,
        target: target.into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: fee,
        },
        initial_balance,
        extensions,
        actions: Vec::new(),
    };

    let is_corrected = target.contains("corrected");
    let mut model = TokenReferenceModel::from_scenario(&scenario);
    let steps = 2 + rng.below(6) as usize;

    for _ in 0..steps {
        let snap = model.snapshot;
        let user_bal = snap.depositor_balance;
        let vault_amt = snap.vault_amount;
        let is_frozen = snap.vault_frozen;
        let is_paused = snap.mint_paused.unwrap_or(false);
        let has_perm_delegate = scenario
            .extensions
            .contains(&MintExtension::PermanentDelegate);
        let allowance = snap.delegate_allowance;

        let action = if is_frozen && rng.below(10) < 7 {
            Action::ThawVault
        } else if is_paused && rng.below(10) < 6 {
            Action::Resume
        } else {
            match rng.below(10) {
                0..=3 if user_bal > 0 => {
                    let amt = match rng.below(4) {
                        0 => user_bal,
                        1 => (user_bal / 2).max(1),
                        2 => {
                            if let Some(f) = fee {
                                let bounds = rounding_boundaries(f.basis_points);
                                pick(&mut rng, &bounds).min(user_bal).max(1)
                            } else {
                                1 + rng.below(user_bal)
                            }
                        }
                        _ => 1 + rng.below(user_bal),
                    };
                    Action::Deposit { amount: amt }
                }
                4..=5 if vault_amt > 0 => {
                    if has_perm_delegate && rng.below(2) == 0 {
                        Action::PermanentDelegateDrain {
                            amount: (vault_amt / 2).max(1),
                        }
                    } else {
                        Action::Withdraw
                    }
                }
                6 if user_bal > 0 && allowance == 0 => Action::ApproveDelegate {
                    amount: (user_bal / 2).max(1),
                },
                7 if allowance > 0 => Action::DelegateTransfer {
                    amount: (allowance / 2).max(1),
                },
                8 if !is_paused && scenario.extensions.contains(&MintExtension::Pausable) => {
                    Action::Pause
                }
                _ => {
                    if user_bal > 0 {
                        Action::Deposit {
                            amount: (user_bal / 3).max(1),
                        }
                    } else if vault_amt > 0 {
                        Action::Withdraw
                    } else {
                        Action::Deposit { amount: 100 }
                    }
                }
            }
        };

        let _ = model.step(action, is_corrected);
        scenario.actions.push(action);
    }

    scenario
}
