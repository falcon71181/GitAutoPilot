use thiserror::Error;
use tokio::task::JoinError;

use log::error;

use crate::config::ConfigError;

/// Custom error types for GitAutoPilot operations
#[derive(Error, Debug)]
pub enum GitAutoPilotError {
    /// Error when home directory cannot be determined
    #[error("Unable to determine home directory")]
    HomeDirError,

    /// Error during directory creation
    #[error("Failed to create dot directory: {0}")]
    DirCreationError(String),

    /// Errors related to configuration file and parsing
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),

    /// Wrapper for standard IO errors
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    /// Wrapper for standard notify errors
    #[error(transparent)]
    NotifyError(#[from] notify::Error),

    /// Wrapper for standard tokio join errors
    #[error(transparent)]
    TokioJoinError(#[from] JoinError),

    /// Wrapper for standard git2 errors
    #[error(transparent)]
    Git2Error(#[from] git2::Error),
}

// Log the error details when the GitAutoPilotError is being dropped
impl Drop for GitAutoPilotError {
    fn drop(&mut self) {
        error!("{}", self);
    }
}
