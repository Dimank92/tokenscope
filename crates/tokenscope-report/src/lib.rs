//! Reports and reproduction artifacts derived from a [`Campaign`].
//!
//! Reports contain no wall-clock data, so the same campaign always renders identically. Every
//! rendering (JSON, text, CLI, TUI) is a view of the same [`Campaign`] value.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokenscope_model::{Campaign, Finding, OutcomeClass, SCHEMA_VERSION, Totals};

/// Version of the tool that produced a report.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Per-run summary line in a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Scenario identifier.
    pub scenario_id: String,
    /// Scenario name.
    pub name: String,
    /// Classification.
    pub outcome: OutcomeClass,
    /// Setup error detail, if any.
    pub setup_error: Option<String>,
    /// Identifiers of findings from this run.
    pub finding_ids: Vec<String>,
}

/// The versioned JSON report (see `FORMAT.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Report format version.
    pub schema_version: u32,
    /// Tool version that produced it.
    pub tokenscope_version: String,
    /// Campaign identifier.
    pub campaign_id: String,
    /// Target identifier.
    pub target: String,
    /// Campaign seed.
    pub seed: u64,
    /// Aggregate counts.
    pub totals: Totals,
    /// Per-run summaries.
    pub runs: Vec<RunSummary>,
    /// All findings with full evidence and reproduction details.
    pub findings: Vec<Finding>,
}

/// Report/artifact errors.
#[derive(Debug, Error)]
pub enum ReportError {
    /// Filesystem failure.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        source: io::Error,
    },
}

/// Build the structured report for a campaign.
pub fn build_report(c: &Campaign) -> Report {
    Report {
        schema_version: SCHEMA_VERSION,
        tokenscope_version: TOOL_VERSION.to_string(),
        campaign_id: c.id.clone(),
        target: c.target.clone(),
        seed: c.seed,
        totals: c.totals(),
        runs: c
            .runs
            .iter()
            .map(|r| RunSummary {
                scenario_id: r.scenario_id.clone(),
                name: r.scenario.name.clone(),
                outcome: r.outcome,
                setup_error: r.setup_error.clone(),
                finding_ids: r.findings.iter().map(|f| f.id.clone()).collect(),
            })
            .collect(),
        findings: c.findings().cloned().collect(),
    }
}

/// Render the JSON report.
pub fn render_json(c: &Campaign) -> String {
    serde_json::to_string_pretty(&build_report(c)).expect("report serialization is infallible")
        + "\n"
}

fn outcome_label(o: OutcomeClass) -> &'static str {
    match o {
        OutcomeClass::Pass => "pass",
        OutcomeClass::SetupError => "setup-error",
        OutcomeClass::ExecutionFailure => "execution-failure",
        OutcomeClass::InvariantViolation => "invariant-violation",
        OutcomeClass::SuspectedVulnerability => "suspected-vulnerability",
    }
}

/// Render one finding for humans.
pub fn render_finding_text(f: &Finding) -> String {
    let mut s = format!(
        "{} {} [{:?}] target={} scenario={} seed={}\n  expected: {}\n  observed: {}\n",
        f.id, f.invariant_id, f.severity, f.target, f.scenario_id, f.seed, f.expected, f.observed
    );
    s.push_str("  evidence:\n");
    for d in &f.deltas {
        s.push_str(&format!(
            "    {} {} -> {} ({})\n",
            d.label, d.before, d.after, d.account
        ));
    }
    s.push_str(&format!("  severity rationale: {}\n", f.severity_rationale));
    s.push_str(&format!("  reproduce: {}\n", f.reproduction.command));
    if let Some(m) = &f.minimized {
        s.push_str(&format!(
            "  minimized scenario: {} ({} action(s))\n",
            m.id(),
            m.actions.len()
        ));
    }
    s
}

/// Render the human-readable campaign report.
pub fn render_text(c: &Campaign) -> String {
    let t = c.totals();
    let mut s = format!(
        "TokenScope campaign {}\ntarget: {}   seed: {}\nexecuted {} | pass {} | execution failures {} | invariant violations {} | setup errors {} | findings {}\n\nRuns:\n",
        c.id,
        c.target,
        c.seed,
        t.executed,
        t.passed,
        t.execution_failures,
        t.invariant_violations,
        t.setup_errors,
        t.findings
    );
    for r in &c.runs {
        s.push_str(&format!(
            "  {:<22} {:<32} {}",
            outcome_label(r.outcome),
            r.scenario.name,
            r.scenario_id
        ));
        if let Some(e) = &r.setup_error {
            s.push_str(&format!("  ({e})"));
        }
        s.push('\n');
    }
    if t.findings > 0 {
        s.push_str("\nFindings:\n");
        for f in c.findings() {
            s.push_str(&render_finding_text(f));
            s.push('\n');
        }
    }
    s.push_str("\nNote: an invariant violation is not proof of exploitability.\n");
    s
}

fn write(path: &Path, content: &str) -> Result<(), ReportError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|source| ReportError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| ReportError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Files written for a campaign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifacts {
    /// Directory holding everything.
    pub dir: PathBuf,
    /// Full campaign (with traces) for `report` and the TUI.
    pub campaign: PathBuf,
    /// JSON report.
    pub report_json: PathBuf,
    /// Text report.
    pub report_text: PathBuf,
    /// One file per finding (`findings/<id>.json`), usable with `tokenscope minimize`.
    pub findings: Vec<PathBuf>,
    /// One file per distinct finding scenario (`scenarios/<id>.json`), usable with `replay`.
    pub scenarios: Vec<PathBuf>,
}

/// Write `<dir>/<campaign-id>/{campaign.json,report.json,report.txt,findings/*,scenarios/*}`.
pub fn write_artifacts(c: &Campaign, out_dir: &Path) -> Result<Artifacts, ReportError> {
    let dir = out_dir.join(&c.id);
    let campaign = dir.join("campaign.json");
    let report_json = dir.join("report.json");
    let report_text = dir.join("report.txt");
    write(&campaign, &(c.to_json() + "\n"))?;
    write(&report_json, &render_json(c))?;
    write(&report_text, &render_text(c))?;
    let mut findings = Vec::new();
    let mut scenarios = Vec::new();
    for f in c.findings() {
        let p = dir.join("findings").join(format!("{}.json", f.id));
        write(&p, &(f.to_json() + "\n"))?;
        findings.push(p);
        let sp = dir
            .join("scenarios")
            .join(format!("{}.json", f.scenario_id));
        if !scenarios.contains(&sp) {
            write(&sp, &(f.reproduction.scenario.to_json() + "\n"))?;
            scenarios.push(sp);
        }
    }
    Ok(Artifacts {
        dir,
        campaign,
        report_json,
        report_text,
        findings,
        scenarios,
    })
}

#[cfg(test)]
mod tests;
