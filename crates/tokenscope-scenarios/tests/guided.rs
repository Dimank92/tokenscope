//! Boundary exploration, mutation, dimension tracking and guided selection (property tests).

use proptest::prelude::*;
use tokenscope_model::{Action, MintExtension, Scenario};
use tokenscope_scenarios::{
    Coverage, SplitMix64, boundary_scenarios, builtin_suite, cells, extension_suite, generate,
    generate_extended, generate_guided, mutate, rounding_boundaries, universe,
};

const T: &str = "demo-escrow-vulnerable";

#[test]
fn rounding_boundaries_surround_the_first_unit_of_fee() {
    // 200 bp: fee becomes 1 at amount ceil(10000/200) = 50.
    let b = rounding_boundaries(200);
    for expected in [0, 1, 2, 49, 50, 51, 100] {
        assert!(b.contains(&expected), "{b:?}");
    }
    assert_eq!(rounding_boundaries(0), vec![0, 1, 2]);
    assert!(rounding_boundaries(1).contains(&10_000));
    assert!(rounding_boundaries(10_000).contains(&1));
}

#[test]
fn boundary_scenarios_are_valid_unique_and_cover_the_fee_extremes() {
    let all = boundary_scenarios(T, 0);
    let mut ids: Vec<_> = all.iter().map(Scenario::id).collect();
    for s in &all {
        s.validate().unwrap();
    }
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
    let cov = all.iter().fold(Coverage::new(), |mut c, s| {
        c.add(s);
        c
    });
    for cell in [
        "fee:zero_bp",
        "fee:low_1_99",
        "fee:full_10000",
        "fee_cap:capped",
        "fee_cap:uncapped",
        "amount:zero",
        "amount:one",
    ] {
        assert!(cov.count(cell) > 0, "missing {cell}");
    }
}

#[test]
fn cells_describe_a_scenario() {
    let s = &builtin_suite(T)[1]; // fee-2pct-deposit: 1000 at 200 bp
    let c = cells(s);
    for expected in [
        "fee:mid_100_999",
        "fee_cap:uncapped",
        "kind:deposit",
        "amount:medium_1e3_1e9",
        "fee_x_amount:mid_100_999:medium_1e3_1e9",
        "len:1",
    ] {
        assert!(c.contains(expected), "{expected} not in {c:?}");
    }
    assert!(universe().contains("fee_x_amount:mid_100_999:medium_1e3_1e9"));
}

#[test]
fn coverage_tracks_gain_and_missing_cells() {
    let mut cov = Coverage::new();
    let s = &builtin_suite(T)[1];
    let first_gain = cov.gain(s);
    assert!(first_gain > 0);
    assert_eq!(cov.add(s), first_gain);
    assert_eq!(cov.gain(s), 0, "a repeat adds nothing");
    assert_eq!(cov.add(s), 0);
    assert!(cov.count("kind:deposit") == 2);
    let missing = cov.missing();
    assert!(!missing.is_empty() && !missing.contains(&"kind:deposit".to_string()));
    assert!(cov.universe_hit() <= cov.distinct());
}

#[test]
fn extended_generation_is_valid_deterministic_and_reaches_every_kind() {
    let a = generate_extended(11, 300, T);
    assert_eq!(a, generate_extended(11, 300, T));
    for s in &a {
        s.validate().unwrap();
    }
    for kind in Action::KIND_NAMES {
        assert!(
            a.iter()
                .any(|s| s.actions.iter().any(|x| x.kind_name() == kind)),
            "kind {kind} never generated"
        );
    }
    for ext in MintExtension::NAMES {
        assert!(
            a.iter()
                .any(|s| s.extensions.iter().any(|e| e.name() == ext)),
            "extension {ext} never generated"
        );
    }
}

proptest! {
    #[test]
    fn mutation_is_deterministic_and_always_valid(seed in any::<u64>(), steps in 1usize..12) {
        let base = &extension_suite(T)[(seed % 10) as usize];
        let run = |seed| {
            let mut rng = SplitMix64::new(seed);
            let mut s = base.clone();
            for _ in 0..steps { s = mutate(&s, &mut rng); }
            s
        };
        let a = run(seed);
        prop_assert!(a.validate().is_ok());
        prop_assert_eq!(&a, &run(seed));
    }

    #[test]
    fn guided_batches_are_deterministic_valid_unique_and_each_adds_coverage(seed in any::<u64>(), count in 1usize..15) {
        let seeds = builtin_suite(T);
        let g = generate_guided(seed, count, T, &seeds);
        prop_assert!(g.scenarios.len() <= count);
        let mut ids = std::collections::BTreeSet::new();
        let mut cov = Coverage::new();
        for s in &seeds { cov.add(s); }
        for s in &g.scenarios {
            prop_assert!(s.validate().is_ok());
            prop_assert!(ids.insert(s.id()), "duplicate scenario");
            prop_assert!(cov.add(s) > 0, "a selected scenario added no new cell");
        }
        prop_assert_eq!(&g.scenarios, &generate_guided(seed, count, T, &seeds).scenarios);
    }
}

#[test]
fn guided_selection_covers_at_least_as_much_as_plain_random_generation() {
    let universe_hit = |scs: &[Scenario]| {
        let mut c = Coverage::new();
        scs.iter().for_each(|s| {
            c.add(s);
        });
        c.universe_hit()
    };
    for seed in 1..=6u64 {
        let n = 12;
        let random = universe_hit(&generate(seed, n, T));
        let guided = universe_hit(&generate_guided(seed, n, T, &[]).scenarios);
        assert!(
            guided >= random,
            "seed {seed}: guided {guided} < random {random}"
        );
    }
    // With a larger budget guided selection reaches a meaningful share of the universe.
    let g = generate_guided(3, 60, T, &builtin_suite(T));
    assert!(
        g.coverage.universe_hit() * 2 > universe().len(),
        "{}/{}",
        g.coverage.universe_hit(),
        universe().len()
    );
}

#[test]
fn guided_generation_complements_its_seeds_and_reports_redundancy() {
    let seeds: Vec<_> = builtin_suite(T)
        .into_iter()
        .chain(extension_suite(T))
        .collect();
    let before = seeds.iter().fold(Coverage::new(), |mut c, s| {
        c.add(s);
        c
    });
    let g = generate_guided(5, 20, T, &seeds);
    assert!(g.candidates > g.scenarios.len());
    assert!(g.redundant > 0, "some candidates must have been redundant");
    assert!(
        g.coverage.distinct() > before.distinct(),
        "the batch adds cells the seeds lack"
    );
    // Seeds' cells are not reselected as "new": every selected scenario adds something beyond the seeds.
    let mut cov = before;
    for s in &g.scenarios {
        assert!(cov.add(s) > 0);
    }
}

#[test]
fn generated_scenarios_use_independent_random_streams() {
    // Regression: with seeds spaced by SplitMix64's own increment, scenario i+1's stream was
    // scenario i's stream shifted by one draw, which badly skewed the action-kind distribution.
    let kinds: std::collections::BTreeMap<&str, usize> = generate_extended(11, 300, T)
        .iter()
        .flat_map(|s| s.actions.iter().map(|a| a.kind_name()))
        .fold(Default::default(), |mut m, k| {
            *m.entry(k).or_default() += 1;
            m
        });
    let total: usize = kinds.values().sum();
    // The rarest kind must still get a reasonable share (uniform would be ~1/17 of 1/14 branch mass).
    let rarest = kinds.values().copied().min().unwrap();
    assert!(
        rarest * 400 >= total,
        "rarest kind has only {rarest} of {total}: {kinds:?}"
    );
    assert_eq!(kinds.len(), Action::KIND_NAMES.len());
}
