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

/// Render the Markdown campaign report.
pub fn render_markdown(c: &Campaign) -> String {
    let t = c.totals();
    let mut s = format!(
        "# TokenScope Campaign Report\n\n\
        | Metric | Value |\n\
        |---|---|\n\
        | **Campaign ID** | `{}` |\n\
        | **Target** | `{}` |\n\
        | **Deterministic Seed** | `{}` |\n\
        | **Tool Version** | `tokenscope {}` |\n\
        | **Total Executed** | {} |\n\
        | **Passed** | {} |\n\
        | **Execution Failures** | {} |\n\
        | **Invariant Violations** | {} |\n\
        | **Setup Errors** | {} |\n\
        | **Findings** | {} |\n\n\
        ## Runs\n\n\
        | Scenario ID | Name | Outcome | Detail |\n\
        |---|---|---|---|\n",
        c.id,
        c.target,
        c.seed,
        TOOL_VERSION,
        t.executed,
        t.passed,
        t.execution_failures,
        t.invariant_violations,
        t.setup_errors,
        t.findings
    );

    for r in &c.runs {
        let detail = r.setup_error.as_deref().unwrap_or("-");
        s.push_str(&format!(
            "| `{}` | {} | `{}` | {} |\n",
            r.scenario_id,
            r.scenario.name.replace('|', "\\|"),
            outcome_label(r.outcome),
            detail.replace('|', "\\|")
        ));
    }

    if t.findings > 0 {
        s.push_str("\n## Invariant Violations and Findings\n\n");
        for f in c.findings() {
            s.push_str(&format!(
                "### `{}`: {} [{:?}]\n\n\
                * **Target:** `{}`\n\
                * **Scenario:** `{}` (seed: `{}`)\n\
                * **Expected:** {}\n\
                * **Observed:** {}\n\
                * **Severity Rationale:** {}\n\n",
                f.id,
                f.invariant_id,
                f.severity,
                f.target,
                f.scenario_id,
                f.seed,
                f.expected,
                f.observed,
                f.severity_rationale
            ));

            if !f.deltas.is_empty() {
                s.push_str("#### State Deltas\n\n| Account | Metric | Before | After | Delta |\n|---|---|---|---|---|\n");
                for d in &f.deltas {
                    let diff = (d.after as i128) - (d.before as i128);
                    s.push_str(&format!(
                        "| `{}` | `{}` | {} | {} | {} |\n",
                        d.account, d.label, d.before, d.after, diff
                    ));
                }
                s.push('\n');
            }

            s.push_str(&format!(
                "#### Reproduction\n\n```sh\n{}\n```\n\n",
                f.reproduction.command
            ));

            if let Some(m) = &f.minimized {
                s.push_str(&format!(
                    "**Minimized Scenario:** `{}` ({} actions)\n\n",
                    m.id(),
                    m.actions.len()
                ));
            }
        }
    }

    s.push_str("\n---\n*Note: An invariant violation indicates an on-chain divergence under execution, but is not automatic proof of exploitability.*\n");
    s
}

/// Render the SARIF 2.1.0 report for code-scanning and CI integration.
pub fn render_sarif(c: &Campaign) -> String {
    use std::collections::BTreeMap;

    let mut rules_map = BTreeMap::new();
    for f in c.findings() {
        rules_map.entry(&f.invariant_id).or_insert_with(|| {
            serde_json::json!({
                "id": f.invariant_id,
                "name": f.invariant_id,
                "shortDescription": {
                    "text": format!("Token-2022 invariant violation: {}", f.invariant_id)
                },
                "defaultConfiguration": {
                    "level": match f.severity {
                        tokenscope_model::Severity::High => "error",
                        tokenscope_model::Severity::Medium => "warning",
                        tokenscope_model::Severity::Low | tokenscope_model::Severity::Info => "note",
                    }
                }
            })
        });
    }
    let rules: Vec<_> = rules_map.into_values().collect();

    let results: Vec<_> = c.findings().map(|f| {
        let level = match f.severity {
            tokenscope_model::Severity::High => "error",
            tokenscope_model::Severity::Medium => "warning",
            tokenscope_model::Severity::Low | tokenscope_model::Severity::Info => "note",
        };
        serde_json::json!({
            "ruleId": f.invariant_id,
            "level": level,
            "message": {
                "text": format!("Invariant {}: expected {}, observed {}", f.invariant_id, f.expected, f.observed)
            },
            "locations": [
                {
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": format!("scenarios/{}.json", f.scenario_id)
                        }
                    }
                }
            ],
            "properties": {
                "findingId": f.id,
                "target": f.target,
                "scenarioId": f.scenario_id,
                "seed": f.seed,
                "severityRationale": f.severity_rationale,
                "reproductionCommand": f.reproduction.command
            }
        })
    }).collect();

    let sarif = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "TokenScope",
                        "version": TOOL_VERSION,
                        "informationUri": "https://github.com/integral-830/tokenscope",
                        "rules": rules
                    }
                },
                "results": results
            }
        ]
    });

    serde_json::to_string_pretty(&sarif).expect("sarif serialization is infallible") + "\n"
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
    /// Markdown report.
    pub report_markdown: PathBuf,
    /// SARIF 2.1.0 report.
    pub report_sarif: PathBuf,
    /// One file per finding (`findings/<id>.json`), usable with `tokenscope minimize`.
    pub findings: Vec<PathBuf>,
    /// One file per distinct finding scenario (`scenarios/<id>.json`), usable with `replay`.
    pub scenarios: Vec<PathBuf>,
}

/// Write `<dir>/<campaign-id>/{campaign.json,report.json,report.txt,report.md,report.sarif,findings/*,scenarios/*}`.
pub fn write_artifacts(c: &Campaign, out_dir: &Path) -> Result<Artifacts, ReportError> {
    let dir = out_dir.join(&c.id);
    let campaign = dir.join("campaign.json");
    let report_json = dir.join("report.json");
    let report_text = dir.join("report.txt");
    let report_markdown = dir.join("report.md");
    let report_sarif = dir.join("report.sarif");
    write(&campaign, &(c.to_json() + "\n"))?;
    write(&report_json, &render_json(c))?;
    write(&report_text, &render_text(c))?;
    write(&report_markdown, &render_markdown(c))?;
    write(&report_sarif, &render_sarif(c))?;
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
        report_markdown,
        report_sarif,
        findings,
        scenarios,
    })
}

#[cfg(test)]
mod tests;
