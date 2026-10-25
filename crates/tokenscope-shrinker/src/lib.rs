//! Failure minimization.
//!
//! [`shrink`] reduces a failing [`Scenario`] while a caller-supplied *failure predicate* keeps
//! holding. The predicate normally re-executes the scenario on the real SVM and checks that the
//! same invariant is still violated, so every accepted reduction is validated by real execution.
//!
//! Properties:
//! * deterministic (no randomness);
//! * bounded by [`ShrinkConfig::max_attempts`] predicate evaluations;
//! * never returns an invalid scenario (candidates failing [`Scenario::validate`] are skipped);
//! * the returned scenario satisfies the predicate (it is re-checked at the end).
//!
//! Strategies, applied to a fixpoint: keep a single action, delete actions, shrink numeric inputs
//! (deposit amounts, initial balance, fee basis points, maximum fee, decimals) and drop the
//! transfer-fee extension.

use thiserror::Error;
use tokenscope_model::{Action, ModelError, Scenario, TransferFeeParams};

/// Shrinking limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShrinkConfig {
    /// Maximum number of predicate evaluations (excluding the two bookend checks).
    pub max_attempts: usize,
}

impl Default for ShrinkConfig {
    fn default() -> Self {
        Self { max_attempts: 300 }
    }
}

/// Outcome of a shrink run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShrinkReport {
    /// The minimized scenario (satisfies the predicate).
    pub minimized: Scenario,
    /// Predicate evaluations spent on candidates.
    pub attempts: usize,
    /// Candidates accepted.
    pub accepted: usize,
    /// Whether the attempt budget ran out before a fixpoint was reached.
    pub budget_exhausted: bool,
}

/// Shrinking failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShrinkError {
    /// The original scenario is invalid.
    #[error("original scenario is invalid: {0}")]
    InvalidInput(#[from] ModelError),
    /// The failure does not reproduce on the original scenario, so there is nothing to preserve.
    #[error("the failure does not reproduce on the original scenario")]
    NotReproducible,
    /// Internal consistency failure: the final scenario no longer satisfies the predicate
    /// (indicates a non-deterministic predicate).
    #[error(
        "the minimized scenario no longer reproduces the failure (non-deterministic predicate?)"
    )]
    LostFailure,
}

struct Ctx<F: FnMut(&Scenario) -> bool> {
    predicate: F,
    attempts: usize,
    accepted: usize,
    max: usize,
}

impl<F: FnMut(&Scenario) -> bool> Ctx<F> {
    fn exhausted(&self) -> bool {
        self.attempts >= self.max
    }

    /// Evaluate a candidate; invalid or over-budget candidates are rejected without evaluation.
    fn accepts(&mut self, candidate: &Scenario) -> bool {
        if self.exhausted() || candidate.validate().is_err() {
            return false;
        }
        self.attempts += 1;
        let ok = (self.predicate)(candidate);
        if ok {
            self.accepted += 1;
        }
        ok
    }

    /// Smallest value reachable from `start` by repeatedly accepting smaller candidates.
    fn shrink_u64(&mut self, start: u64, mut build: impl FnMut(u64) -> Scenario) -> u64 {
        let mut cur = start;
        loop {
            let mut next = None;
            for cand in candidates(cur) {
                if self.accepts(&build(cand)) {
                    next = Some(cand);
                    break;
                }
            }
            match next {
                Some(c) => cur = c,
                None => return cur,
            }
        }
    }
}

/// Ascending candidate values strictly below `cur`: `0`, `1`, then `cur - cur/2^k` for
/// k = 1, 2, ... (gap halving, which converges on a failure boundary in O(log^2 n) evaluations)
/// and finally `cur - 1`.
fn candidates(cur: u64) -> Vec<u64> {
    let mut c = vec![0, 1, cur.saturating_sub(1)];
    for k in 1..64 {
        let step = cur >> k;
        if step == 0 {
            break;
        }
        c.push(cur - step);
    }
    c.retain(|&v| v < cur);
    c.sort_unstable();
    c.dedup();
    c
}

/// Minimize `original` while `predicate` holds.
pub fn shrink<F>(
    original: &Scenario,
    config: ShrinkConfig,
    predicate: F,
) -> Result<ShrinkReport, ShrinkError>
where
    F: FnMut(&Scenario) -> bool,
{
    original.validate()?;
    let mut ctx = Ctx {
        predicate,
        attempts: 0,
        accepted: 0,
        max: config.max_attempts,
    };
    if !(ctx.predicate)(original) {
        return Err(ShrinkError::NotReproducible);
    }
    let mut cur = original.clone();
    loop {
        let before = cur.clone();
        cur = keep_single_action(&mut ctx, cur);
        cur = delete_actions(&mut ctx, cur);
        cur = shrink_amounts(&mut ctx, cur);
        cur = shrink_config(&mut ctx, cur);
        if cur == before || ctx.exhausted() {
            break;
        }
    }
    let budget_exhausted = ctx.exhausted();
    let (attempts, accepted) = (ctx.attempts, ctx.accepted);
    cur.name = truncate_name(&format!("{}-min", original.name));
    if !(ctx.predicate)(&cur) {
        return Err(ShrinkError::LostFailure);
    }
    Ok(ShrinkReport {
        minimized: cur,
        attempts,
        accepted,
        budget_exhausted,
    })
}

fn truncate_name(name: &str) -> String {
    name.chars().take(tokenscope_model::MAX_NAME_LEN).collect()
}

fn keep_single_action<F: FnMut(&Scenario) -> bool>(ctx: &mut Ctx<F>, cur: Scenario) -> Scenario {
    if cur.actions.len() <= 1 {
        return cur;
    }
    for i in 0..cur.actions.len() {
        let mut cand = cur.clone();
        cand.actions = vec![cur.actions[i]];
        if ctx.accepts(&cand) {
            return cand;
        }
    }
    cur
}

fn delete_actions<F: FnMut(&Scenario) -> bool>(ctx: &mut Ctx<F>, mut cur: Scenario) -> Scenario {
    let mut i = cur.actions.len();
    while i > 0 {
        i -= 1;
        if cur.actions.len() <= 1 {
            break;
        }
        let mut cand = cur.clone();
        cand.actions.remove(i);
        if ctx.accepts(&cand) {
            cur = cand;
        }
    }
    cur
}

fn shrink_amounts<F: FnMut(&Scenario) -> bool>(ctx: &mut Ctx<F>, mut cur: Scenario) -> Scenario {
    for i in 0..cur.actions.len() {
        if let Action::Deposit { amount } = cur.actions[i] {
            let base = cur.clone();
            let best = ctx.shrink_u64(amount, |v| {
                let mut s = base.clone();
                s.actions[i] = Action::Deposit { amount: v };
                s
            });
            cur.actions[i] = Action::Deposit { amount: best };
        }
    }
    let base = cur.clone();
    cur.initial_balance = ctx.shrink_u64(base.initial_balance, |v| {
        let mut s = base.clone();
        s.initial_balance = v;
        s
    });
    cur
}

fn shrink_config<F: FnMut(&Scenario) -> bool>(ctx: &mut Ctx<F>, mut cur: Scenario) -> Scenario {
    if cur.mint.decimals != 0 {
        let mut cand = cur.clone();
        cand.mint.decimals = 0;
        if ctx.accepts(&cand) {
            cur = cand;
        }
    }
    if let Some(fee) = cur.mint.transfer_fee {
        let mut none = cur.clone();
        none.mint.transfer_fee = None;
        if ctx.accepts(&none) {
            return none;
        }
        let base = cur.clone();
        let bps = ctx.shrink_u64(u64::from(fee.basis_points), |v| {
            let mut s = base.clone();
            s.mint.transfer_fee = Some(TransferFeeParams {
                basis_points: v as u16,
                maximum_fee: fee.maximum_fee,
            });
            s
        });
        let base = Scenario {
            mint: tokenscope_model::MintSpec {
                transfer_fee: Some(TransferFeeParams {
                    basis_points: bps as u16,
                    maximum_fee: fee.maximum_fee,
                }),
                ..cur.mint
            },
            ..cur.clone()
        };
        let max = ctx.shrink_u64(fee.maximum_fee, |v| {
            let mut s = base.clone();
            s.mint.transfer_fee = Some(TransferFeeParams {
                basis_points: bps as u16,
                maximum_fee: v,
            });
            s
        });
        cur.mint.transfer_fee = Some(TransferFeeParams {
            basis_points: bps as u16,
            maximum_fee: max,
        });
    }
    cur
}

#[cfg(test)]
mod tests;
