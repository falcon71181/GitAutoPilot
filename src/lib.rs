use config::ConfigError;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

mod config;
mod logger;

/// Represents the Git Auto Pilot configuration and file management
#[derive(Debug, Serialize, Deserialize)]
pub struct GitAutoPilot {
    /// Configuration settings for the Git Auto Pilot
    pub config: config::Config,

    /// Location of the dot directory
    pub dot_dir_location: String,

    /// Location of the configuration file
    pub dot_file_location: String,
}

/// Constant for the default dot directory path
const DOT_DIR: &str = ".config/git-auto-pilot";

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
}
