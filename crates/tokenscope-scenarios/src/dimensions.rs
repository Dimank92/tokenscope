//! Scenario-dimension tracking.
//!
//! A *cell* is a small, named combination of scenario features (fee class, amount class, fee x
//! amount, action kind, extension, extension x action kind, a few structural flags). Coverage here
//! means coverage of this **input space**; it says nothing about which lines of the target program
//! ran. It exists to pick varied scenarios and to show which regions have not been tried.

use std::collections::{BTreeMap, BTreeSet};

use tokenscope_model::{Action, MintExtension, Scenario};

/// Fee class of a scenario's transfer-fee configuration.
pub const FEE_CLASSES: [&str; 6] = [
    "none",
    "zero_bp",
    "low_1_99",
    "mid_100_999",
    "high_1000_9999",
    "full_10000",
];
/// Amount classes of action amounts.
pub const AMOUNT_CLASSES: [&str; 6] = [
    "zero",
    "one",
    "small_2_999",
    "medium_1e3_1e9",
    "large",
    "u64_max",
];

/// Class of a basis-points value (`None` = no fee extension).
pub fn fee_class(bps: Option<u16>) -> &'static str {
    match bps {
        None => "none",
        Some(0) => "zero_bp",
        Some(1..=99) => "low_1_99",
        Some(100..=999) => "mid_100_999",
        Some(1000..=9999) => "high_1000_9999",
        Some(_) => "full_10000",
    }
}

/// Class of an amount.
pub fn amount_class(a: u64) -> &'static str {
    match a {
        0 => "zero",
        1 => "one",
        2..=999 => "small_2_999",
        1_000..=1_000_000_000 => "medium_1e3_1e9",
        u64::MAX => "u64_max",
        _ => "large",
    }
}

/// The cells a scenario occupies.
pub fn cells(s: &Scenario) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let fee = fee_class(s.mint.transfer_fee.map(|f| f.basis_points));
    out.insert(format!("fee:{fee}"));
    if let Some(f) = s.mint.transfer_fee {
        out.insert(format!(
            "fee_cap:{}",
            if f.maximum_fee == u64::MAX {
                "uncapped"
            } else {
                "capped"
            }
        ));
    }
    let exts: Vec<&str> = s.extensions.iter().map(|e| e.name()).collect();
    for e in &exts {
        out.insert(format!("ext:{e}"));
    }
    if let Some(MintExtension::DefaultAccountState { frozen }) = s
        .extensions
        .iter()
        .find(|e| matches!(e, MintExtension::DefaultAccountState { .. }))
    {
        out.insert(format!("default_state_frozen:{frozen}"));
    }
    for a in &s.actions {
        let kind = a.kind_name();
        out.insert(format!("kind:{kind}"));
        for e in &exts {
            out.insert(format!("ext_x_kind:{e}:{kind}"));
        }
        if let Some(amount) = a.amount() {
            let ac = amount_class(amount);
            out.insert(format!("amount:{ac}"));
            out.insert(format!("fee_x_amount:{fee}:{ac}"));
        }
    }
    out.insert(format!(
        "len:{}",
        match s.actions.len() {
            1 => "1",
            2..=3 => "2_3",
            _ => "4_plus",
        }
    ));
    out.insert(format!("balance:{}", amount_class(s.initial_balance)));
    out
}

/// Size of the universe of *reachable-by-design* cells (used only to report a percentage).
pub fn universe() -> BTreeSet<String> {
    let mut u = BTreeSet::new();
    for f in FEE_CLASSES {
        u.insert(format!("fee:{f}"));
        for a in AMOUNT_CLASSES {
            u.insert(format!("fee_x_amount:{f}:{a}"));
        }
    }
    u.insert("fee_cap:capped".into());
    u.insert("fee_cap:uncapped".into());
    for a in AMOUNT_CLASSES {
        u.insert(format!("amount:{a}"));
        u.insert(format!("balance:{a}"));
    }
    for k in Action::KIND_NAMES {
        u.insert(format!("kind:{k}"));
        for e in tokenscope_model::MintExtension::NAMES {
            u.insert(format!("ext_x_kind:{e}:{k}"));
        }
    }
    for e in tokenscope_model::MintExtension::NAMES {
        u.insert(format!("ext:{e}"));
    }
    u.insert("default_state_frozen:true".into());
    u.insert("default_state_frozen:false".into());
    for l in ["1", "2_3", "4_plus"] {
        u.insert(format!("len:{l}"));
    }
    u
}

/// Running count of how often each cell has been exercised.
#[derive(Debug, Clone, Default)]
pub struct Coverage {
    seen: BTreeMap<String, usize>,
}

impl Coverage {
    /// Empty coverage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of not-yet-seen cells `s` would add.
    pub fn gain(&self, s: &Scenario) -> usize {
        cells(s)
            .iter()
            .filter(|c| !self.seen.contains_key(*c))
            .count()
    }

    /// Record `s`; returns the number of new cells.
    pub fn add(&mut self, s: &Scenario) -> usize {
        let mut new = 0;
        for c in cells(s) {
            let n = self.seen.entry(c).or_insert(0);
            if *n == 0 {
                new += 1;
            }
            *n += 1;
        }
        new
    }

    /// Distinct cells seen.
    pub fn distinct(&self) -> usize {
        self.seen.len()
    }

    /// Distinct seen cells that are in [`universe`].
    pub fn universe_hit(&self) -> usize {
        let u = universe();
        self.seen.keys().filter(|k| u.contains(*k)).count()
    }

    /// Cells of the universe not yet seen, sorted.
    pub fn missing(&self) -> Vec<String> {
        universe()
            .into_iter()
            .filter(|c| !self.seen.contains_key(c))
            .collect()
    }

    /// Times a cell has been seen.
    pub fn count(&self, cell: &str) -> usize {
        self.seen.get(cell).copied().unwrap_or(0)
    }
}
