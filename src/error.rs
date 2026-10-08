//! Error types and exit codes for League Mod Fixer.

use thiserror::Error;

/// Exit codes returned by the CLI executable.
#[allow(dead_code)]
pub mod exit_codes {
    /// Everything completed successfully (healthy mods untouched, repairs applied, check clean).
    pub const SUCCESS: i32 = 0;
    /// Process finished, but one or more mods had unrepairable issues or repair failures.
    pub const REPAIRABLE_OR_FAILED: i32 = 1;
    /// Fatal initialization error (e.g., hashtables missing and unable to sync).
    pub const INITIALIZATION_ERROR: i32 = 2;
    /// Invalid arguments or path.
    pub const INVALID_ARGUMENTS: i32 = 3;
}

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum FixerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Replacement error: {0}")]
    Replacement(String),

    #[error("Archive error: {0}")]
    Archive(String),

    #[error("Format error: {0}")]
    Format(String),

    #[error("Health check error: {0}")]
    HealthCheck(String),

    #[error("Repair error: {0}")]
    Repair(String),

    #[error("Verification error: {0}")]
    Verification(String),

    #[error("Hashtable error: {0}")]
    Hashtables(String),

    #[error("League path error: {0}")]
    LeaguePath(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("LTK Manager error: {0}")]
    Ltk(String),

    #[error("Cancelled by user")]
    Cancelled,
}

impl From<ltk_manager_base::error::AppError> for FixerError {
    fn from(err: ltk_manager_base::error::AppError) -> Self {
        Self::Ltk(err.to_string())
    }
}

pub type FixerResult<T> = Result<T, FixerError>;
