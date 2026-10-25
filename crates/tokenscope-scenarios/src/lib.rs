//! Scenario generation.
//!
//! * [`builtin_suite`] is a fixed, hand-written set covering transfer fees, rounding, boundaries
//!   and failure cases.
//! * [`generate`] derives scenarios from a seed with a small, documented PRNG (SplitMix64), so
//!   the same seed always yields the same scenarios on every platform.

use tokenscope_model::{
    Action, MAX_ACTIONS, MintSpec, SCHEMA_VERSION, Scenario, TransferFeeParams,
};

/// SplitMix64 pseudo-random generator (Steele, Lea, Flood 2014). Deterministic and portable.
#[derive(Debug, Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// Create a generator from a seed.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish value in `0..bound` (`bound > 0`; modulo bias is irrelevant for test generation).
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

fn base(
    name: &str,
    target: &str,
    fee: Option<(u16, u64)>,
    balance: u64,
    actions: Vec<Action>,
) -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: name.into(),
        seed: 0,
        target: target.into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: fee.map(|(basis_points, maximum_fee)| TransferFeeParams {
                basis_points,
                maximum_fee,
            }),
        },
        initial_balance: balance,
        actions,
    }
}

fn dep(amount: u64) -> Action {
    Action::Deposit { amount }
}

/// The fixed scenario suite for `target`.
pub fn builtin_suite(target: &str) -> Vec<Scenario> {
    let t = target;
    let max = u64::MAX;
    vec![
        base("control-no-fee", t, None, 10_000, vec![dep(1_000)]),
        base(
            "fee-2pct-deposit",
            t,
            Some((200, max)),
            10_000,
            vec![dep(1_000)],
        ),
        base(
            "fee-2pct-deposit-withdraw",
            t,
            Some((200, max)),
            10_000,
            vec![dep(1_000), Action::Withdraw],
        ),
        base(
            "fee-rounds-up-tiny-deposit",
            t,
            Some((1, max)),
            100,
            vec![dep(1)],
        ),
        base(
            "fee-capped-by-maximum",
            t,
            Some((1_000, 10)),
            100_000,
            vec![dep(50_000)],
        ),
        base(
            "fee-100pct-consumes-deposit",
            t,
            Some((10_000, max)),
            1_000,
            vec![dep(500)],
        ),
        base(
            "zero-amount-deposit",
            t,
            Some((500, max)),
            100,
            vec![dep(0)],
        ),
        base(
            "two-deposits-accumulate",
            t,
            Some((300, max)),
            10_000,
            vec![dep(1_000), dep(2_000)],
        ),
        base(
            "large-amount",
            t,
            Some((50, max)),
            2_000_000_000_000_000,
            vec![dep(1_000_000_000_000_000)],
        ),
        base(
            "failure-insufficient-funds",
            t,
            Some((200, max)),
            10,
            vec![dep(1_000)],
        ),
        base(
            "withdraw-with-nothing-recorded",
            t,
            Some((200, max)),
            10,
            vec![Action::Withdraw],
        ),
    ]
}

/// Generate `count` scenarios for `target` from `seed`. Scenario `i` records the seed it was
/// derived from (`seed` mixed with `i`), so it can be regenerated independently.
pub fn generate(seed: u64, count: usize, target: &str) -> Vec<Scenario> {
    (0..count as u64)
        .map(|i| {
            generate_one(
                seed.wrapping_add(i.wrapping_mul(0xA24B_AED4_963E_E407)),
                i,
                target,
            )
        })
        .collect()
}

fn generate_one(seed: u64, index: u64, target: &str) -> Scenario {
    let mut rng = SplitMix64::new(seed);
    let fee = match rng.below(4) {
        0 => None,
        1 => Some((rng.below(10_001) as u16, u64::MAX)),
        2 => Some((rng.below(10_001) as u16, 1 + rng.below(1_000))),
        _ => Some((1 + rng.below(50) as u16, u64::MAX)),
    };
    let balance = 1 + rng.below(1_000_000);
    let n_actions = 1 + rng.below(4) as usize;
    let mut actions = Vec::with_capacity(n_actions);
    for _ in 0..n_actions.min(MAX_ACTIONS) {
        if rng.below(5) == 0 {
            actions.push(Action::Withdraw);
        } else {
            let amount = match rng.below(4) {
                0 => rng.below(4),
                1 => 1 + rng.below(100),
                _ => rng.below(balance / 2 + 1),
            };
            actions.push(dep(amount));
        }
    }
    let mut s = base(&format!("generated-{index}"), target, fee, balance, actions);
    s.seed = seed;
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_matches_reference_vector() {
        // First outputs of SplitMix64 seeded with 0 (reference implementation values).
        let mut r = SplitMix64::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }

    #[test]
    fn builtin_suite_is_valid_and_uniquely_identified() {
        let suite = builtin_suite("demo-escrow-vulnerable");
        assert!(suite.len() >= 10);
        let mut ids: Vec<_> = suite.iter().map(Scenario::id).collect();
        for s in &suite {
            s.validate().unwrap();
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), suite.len());
    }

    #[test]
    fn same_seed_same_scenarios_different_seed_differs() {
        let a = generate(42, 20, "t");
        assert_eq!(a, generate(42, 20, "t"));
        assert_ne!(a, generate(43, 20, "t"));
    }

    #[test]
    fn generated_scenarios_are_valid_and_regenerable_from_recorded_seed() {
        for (i, s) in generate(7, 50, "t").iter().enumerate() {
            s.validate().unwrap();
            assert_eq!(&generate_one(s.seed, i as u64, "t"), s);
        }
    }
}
