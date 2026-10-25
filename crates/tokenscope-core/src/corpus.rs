//! On-disk regression corpus: one JSON file per case (`<case-id>.json`).

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;
use tokenscope_model::{CorpusCase, Expectation, ModelError};

use crate::Engine;

/// Corpus errors.
#[derive(Debug, Error)]
pub enum CorpusError {
    /// Filesystem failure.
    #[error("corpus I/O error at {path}: {source}")]
    Io {
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        source: io::Error,
    },
    /// A case file could not be parsed or failed validation.
    #[error("invalid corpus file {path}: {source}")]
    Invalid {
        /// File path.
        path: PathBuf,
        /// Validation error.
        source: ModelError,
    },
    /// No case with this ID exists.
    #[error("no corpus case `{0}`")]
    NotFound(String),
}

/// Result of replaying one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    /// Case identifier.
    pub case_id: String,
    /// Whether the actual run matched the expectation.
    pub ok: bool,
    /// What the replay produced.
    pub actual: Expectation,
}

/// A directory of regression cases.
#[derive(Debug, Clone)]
pub struct Corpus {
    dir: PathBuf,
}

impl Corpus {
    /// Open (without creating) a corpus directory.
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The corpus directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Save a case (creating the directory if needed). Returns the file path.
    pub fn add(&self, case: &CorpusCase) -> Result<PathBuf, CorpusError> {
        case.validate().map_err(|source| CorpusError::Invalid {
            path: self.path_for(&case.id),
            source,
        })?;
        fs::create_dir_all(&self.dir).map_err(|source| CorpusError::Io {
            path: self.dir.clone(),
            source,
        })?;
        let path = self.path_for(&case.id);
        fs::write(&path, case.to_json() + "\n").map_err(|source| CorpusError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    /// Load every `*.json` case, sorted by ID. A missing directory is an empty corpus.
    pub fn list(&self) -> Result<Vec<CorpusCase>, CorpusError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
            Err(source) => {
                return Err(CorpusError::Io {
                    path: self.dir.clone(),
                    source,
                });
            }
        };
        let mut cases = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|source| CorpusError::Io {
                    path: self.dir.clone(),
                    source,
                })?
                .path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path).map_err(|source| CorpusError::Io {
                path: path.clone(),
                source,
            })?;
            cases.push(
                CorpusCase::from_json(&text)
                    .map_err(|source| CorpusError::Invalid { path, source })?,
            );
        }
        cases.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(cases)
    }

    /// Load one case by ID.
    pub fn get(&self, id: &str) -> Result<CorpusCase, CorpusError> {
        let path = self.path_for(id);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(CorpusError::NotFound(id.to_string()));
            }
            Err(source) => return Err(CorpusError::Io { path, source }),
        };
        CorpusCase::from_json(&text).map_err(|source| CorpusError::Invalid { path, source })
    }
}

/// Replay one case on `engine` and compare with its expectation.
pub fn replay_case(engine: &Engine, case: &CorpusCase) -> CaseResult {
    let actual = Expectation::from_run(&engine.run_scenario(&case.scenario));
    CaseResult {
        case_id: case.id.clone(),
        ok: actual == case.expect,
        actual,
    }
}
