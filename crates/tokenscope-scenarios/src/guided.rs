//! Boundary exploration, deterministic mutation and dimension-guided selection.
//!
//! "Guided" means guided by [`Coverage`] of the scenario *input space*, not by code coverage of
//! the target. Nothing here observes program execution; selection and mutation are driven by the
//! seed (SplitMix64) and by scenario features only.

use std::collections::BTreeSet;

use tokenscope_model::{Action, MAX_ACTIONS, MintExtension, Mismatch, Scenario, TransferFeeParams};

use crate::{Coverage, SplitMix64, dimensions::cells};

/// Basis-point values at class edges and common configurations.
pub const BOUNDARY_BPS: [u16; 9] = [0, 1, 2, 99, 100, 200, 999, 9_999, 10_000];
/// Maximum-fee values: zero, one, a small cap and unbounded.
pub const BOUNDARY_MAX_FEE: [u64; 4] = [0, 1, 1_000, u64::MAX];

/// Amounts where the rounded-up fee changes for `bps`: the smallest amount with fee 1 and its
/// neighbours, plus `0`, `1`, `2`.
pub fn rounding_boundaries(bps: u16) -> Vec<u64> {
    let mut v = vec![0, 1, 2];
    if bps > 0 {
        let a0 = 10_000u64.div_ceil(u64::from(bps));
        v.extend([a0.saturating_sub(1), a0, a0 + 1, a0 * 2]);
    }
    v.sort_unstable();
    v.dedup();
    v
}

fn base(name: String, target: &str, seed: u64) -> Scenario {
    Scenario {
        schema_version: tokenscope_model::SCHEMA_VERSION,
        name,
        seed,
        target: target.into(),
        mint: tokenscope_model::MintSpec {
            decimals: 6,
            transfer_fee: None,
        },
        initial_balance: 1_000_000,
        extensions: vec![],
        actions: vec![],
    }
}

/// Deposit scenarios at rounding and cap boundaries for each boundary fee configuration.
pub fn boundary_scenarios(target: &str, seed: u64) -> Vec<Scenario> {
    let mut out = Vec::new();
    for &bps in &BOUNDARY_BPS {
        for &max_fee in &BOUNDARY_MAX_FEE {
            for amount in rounding_boundaries(bps) {
                let mut s = base(
                    format!("boundary-{bps}bp-max{max_fee}-amt{amount}"),
                    target,
                    seed,
                );
                s.mint.transfer_fee = Some(TransferFeeParams {
                    basis_points: bps,
                    maximum_fee: max_fee,
                });
                s.initial_balance = amount.max(1).saturating_mul(2);
                s.actions = vec![Action::Deposit { amount }];
                out.push(s);
            }
        }
    }
    out
}

fn pick<T: Copy>(rng: &mut SplitMix64, items: &[T]) -> T {
    items[rng.below(items.len() as u64) as usize]
}

/// Extension-aware scenario templates: each combines a mint configuration with actions that
/// exercise it (and sometimes adversarial or mismatched variants).
pub fn generate_extended(seed: u64, count: usize, target: &str) -> Vec<Scenario> {
    (0..count as u64)
        .map(|i| extended_one(scenario_seed(seed, i), i, target))
        .collect()
}

/// Per-scenario seed. It is *hashed* (one SplitMix64 output of an offset state) rather than
/// `seed + i * constant`: SplitMix64 advances its state by a fixed odd constant, so seeds spaced by
/// that same constant would give streams that are shifted copies of each other.
fn scenario_seed(seed: u64, index: u64) -> u64 {
    SplitMix64::new(seed ^ index.wrapping_mul(0xA24B_AED4_963E_E407)).next_u64()
}

fn extended_one(seed: u64, index: u64, target: &str) -> Scenario {
    let mut rng = SplitMix64::new(seed);
    let mut s = base(format!("extended-{index}"), target, seed);
    s.initial_balance = 1 + rng.below(2_000_000);
    if rng.below(2) == 0 {
        s.mint.transfer_fee = Some(TransferFeeParams {
            basis_points: pick(&mut rng, &BOUNDARY_BPS),
            maximum_fee: pick(&mut rng, &BOUNDARY_MAX_FEE),
        });
    }
    let all_ext = [
        MintExtension::Pausable,
        MintExtension::DefaultAccountState { frozen: true },
        MintExtension::PermanentDelegate,
        MintExtension::TransferHook,
    ];
    for e in all_ext {
        if rng.below(4) == 0 {
            s.extensions.push(e);
        }
    }
    let amount = |rng: &mut SplitMix64| match rng.below(4) {
        0 => rng.below(3),
        1 => 1 + rng.below(1_000),
        _ => rng.below(s_balance_half(1_000_000)),
    };
    let n = 1 + rng.below(6) as usize;
    for _ in 0..n {
        let a = match rng.below(14) {
            0..=2 => Action::Deposit {
                amount: amount(&mut rng),
            },
            3 => Action::Withdraw,
            4 => Action::Pause,
            5 => Action::Resume,
            6 => Action::EnableCpiGuard,
            7 => Action::DisableCpiGuard,
            8 => Action::ThawVault,
            9 => Action::ApproveDelegate {
                amount: amount(&mut rng),
            },
            10 => Action::DelegateTransfer {
                amount: amount(&mut rng),
            },
            11 => Action::PermanentDelegateDrain {
                amount: amount(&mut rng),
            },
            12 => Action::MismatchedDeposit {
                mismatch: pick(
                    &mut rng,
                    &[
                        Mismatch::WrongMint,
                        Mismatch::WrongVault,
                        Mismatch::WrongTokenProgram,
                    ],
                ),
                amount: amount(&mut rng),
            },
            _ => pick(
                &mut rng,
                &[
                    Action::UnauthorizedWithdraw,
                    Action::ReinitEscrow,
                    Action::RevokeDelegate,
                    Action::MismatchedWithdraw {
                        mismatch: Mismatch::WrongAuthority,
                    },
                    Action::MismatchedWithdraw {
                        mismatch: Mismatch::WrongVault,
                    },
                    Action::MismatchedWithdraw {
                        mismatch: Mismatch::WrongMint,
                    },
                    Action::MismatchedWithdraw {
                        mismatch: Mismatch::WrongTokenProgram,
                    },
                    Action::FreezeDepositor,
                    Action::ThawDepositor,
                ],
            ),
        };
        s.actions.push(a);
    }
    s
}

fn s_balance_half(b: u64) -> u64 {
    b / 2 + 1
}

/// One deterministic mutation of `s`. The result is always a valid scenario (an invalid mutant
/// falls back to the original).
pub fn mutate(s: &Scenario, rng: &mut SplitMix64) -> Scenario {
    let mut m = s.clone();
    match rng.below(8) {
        0 => {
            // Nudge an amount by +/-1 or halve/double it.
            let idxs: Vec<usize> = (0..m.actions.len())
                .filter(|&i| m.actions[i].amount().is_some())
                .collect();
            if !idxs.is_empty() {
                let i = idxs[rng.below(idxs.len() as u64) as usize];
                if let Some(a) = m.actions[i].amount() {
                    let v = match rng.below(4) {
                        0 => a.saturating_add(1),
                        1 => a.saturating_sub(1),
                        2 => a / 2,
                        _ => a.saturating_mul(2),
                    };
                    m.actions[i] = m.actions[i].with_amount(v);
                }
            }
        }
        1 => {
            m.mint.transfer_fee = Some(TransferFeeParams {
                basis_points: pick(rng, &BOUNDARY_BPS),
                maximum_fee: m.mint.transfer_fee.map_or(u64::MAX, |f| f.maximum_fee),
            });
        }
        2 => {
            m.mint.transfer_fee = m.mint.transfer_fee.map(|f| TransferFeeParams {
                basis_points: f.basis_points,
                maximum_fee: pick(rng, &BOUNDARY_MAX_FEE),
            });
        }
        3 => {
            let e = pick(
                rng,
                &[
                    MintExtension::Pausable,
                    MintExtension::PermanentDelegate,
                    MintExtension::TransferHook,
                    MintExtension::DefaultAccountState { frozen: true },
                ],
            );
            if let Some(p) = m.extensions.iter().position(|x| x.name() == e.name()) {
                m.extensions.remove(p);
            } else {
                m.extensions.push(e);
            }
        }
        4 if m.actions.len() < MAX_ACTIONS => {
            let i = rng.below(m.actions.len() as u64) as usize;
            m.actions.insert(i, m.actions[i]); // duplicate an action
        }
        5 if m.actions.len() > 1 => {
            m.actions.remove(rng.below(m.actions.len() as u64) as usize);
        }
        6 if m.actions.len() > 1 => {
            let (i, j) = (
                rng.below(m.actions.len() as u64) as usize,
                rng.below(m.actions.len() as u64) as usize,
            );
            m.actions.swap(i, j);
        }
        _ => m.initial_balance = pick(rng, &[0, 1, 2, 1_000, u64::MAX]),
    }
    if m.validate().is_ok() { m } else { s.clone() }
}

/// Result of guided generation.
#[derive(Debug, Clone)]
pub struct GuidedBatch {
    /// Selected scenarios, in selection order.
    pub scenarios: Vec<Scenario>,
    /// Candidate scenarios considered (after de-duplication).
    pub candidates: usize,
    /// Candidates skipped because they added no new dimension cell.
    pub redundant: usize,
    /// Dimension coverage of the selection (including `seeds`).
    pub coverage: Coverage,
}

/// Build a candidate pool from fresh extended scenarios, mutations of `seeds` (for example the
/// built-in suites and corpus cases) and boundary scenarios, then greedily select up to `count`
/// that each add at least one new dimension cell. Cells already covered by `seeds` count as
/// covered, so the batch complements them. Deterministic for a given `seed`.
pub fn generate_guided(seed: u64, count: usize, target: &str, seeds: &[Scenario]) -> GuidedBatch {
    let mut rng = SplitMix64::new(seed ^ 0xD1B5_4A32_D192_ED03);
    let mut pool: Vec<Scenario> = Vec::new();
    pool.extend(generate_extended(seed, count * 3, target));
    if !seeds.is_empty() {
        for i in 0..count * 3 {
            let from = &seeds[i % seeds.len()];
            let mut m = from.clone();
            m.target = target.into();
            for _ in 0..(1 + rng.below(3)) {
                m = mutate(&m, &mut rng);
            }
            m.name = format!("mutant-{i}");
            m.seed = seed;
            pool.push(m);
        }
    }
    pool.extend(boundary_scenarios(target, seed));

    let mut seen_ids = BTreeSet::new();
    pool.retain(|s| s.validate().is_ok() && seen_ids.insert(s.id()));
    let candidates = pool.len();

    let mut coverage = Coverage::new();
    for s in seeds {
        let mut t = s.clone();
        t.target = target.into();
        coverage.add(&t);
    }
    let mut selected = Vec::new();
    let mut remaining = pool;
    while selected.len() < count {
        let best = remaining
            .iter()
            .enumerate()
            .map(|(i, s)| (coverage.gain(s), i))
            .filter(|(g, _)| *g > 0)
            .min_by_key(|(g, i)| (usize::MAX - g, *i));
        let Some((_, i)) = best else { break };
        let s = remaining.remove(i);
        coverage.add(&s);
        selected.push(s);
    }
    let redundant = remaining
        .iter()
        .filter(|s| cells(s).iter().all(|c| coverage.count(c) > 0))
        .count();
    GuidedBatch {
        scenarios: selected,
        candidates,
        redundant,
        coverage,
    }
}
