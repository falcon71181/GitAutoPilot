use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use config::ConfigError;
use log::{debug, info, trace, warn, error};
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

// Log the error details when the GitAutoPilotError is being dropped
impl Drop for GitAutoPilotError {
    fn drop(&mut self) {
        error!("{}", self);
    }
}

impl GitAutoPilot {
    /// Creates a new GitAutoPilot instance
    ///
    /// # Returns
    /// A new GitAutoPilot instance with configuration and file paths
    ///
    /// # Errors
    /// Returns a `GitAutoPilotError` if initialization fails
    pub fn new(verbosity: u64) -> Result<Self, GitAutoPilotError> {
        let _ = logger::setup_logging(verbosity).or_else(|err| {
            error!("Logging initialize failed: {}", err);
            Ok::<(), ConfigError>(())
        });

        // Determine dot directory location
        let dot_dir = get_dot_dir_path()?;

        // Ensure dot directory exists
        ensure_dot_dir_exists(&dot_dir)?;

        // Construct dot file path
        let dot_file = format!("{}/config.json", &dot_dir);

        // Load or create configuration
        let config = load_or_create_config(&dot_file)?;

        info!("GitAutoPilot instance created successfully");
        Ok(GitAutoPilot {
            config,
            dot_dir_location: dot_dir,
            dot_file_location: dot_file,
        })
    }
}

/// Determines the path for the dot directory
///
/// # Returns
/// A `String` representing the full path to the dot directory
///
/// # Errors
/// Returns a `GitAutoPilotError` if home directory cannot be determined
fn get_dot_dir_path() -> Result<String, GitAutoPilotError> {
    trace!("Attempting to retrieve home directory");

    // Prefer dirs crate's home_dir, fallback to environment variable
    let dot_dir = dir::home_dir()
        .map(|path| format!("{}/{}", path.display(), DOT_DIR))
        .or_else(|| {
            warn!("Could not retrieve home directory via dirs");
            env::var("HOME")
                .map(|home| format!("{}/{}", home, DOT_DIR))
                .ok()
        })
        .ok_or(GitAutoPilotError::HomeDirError)?;

    trace!("Home directory for dot directory determined: {}", dot_dir);
    Ok(dot_dir)
}

/// Ensures the dot directory exists, creating it if necessary
///
/// # Arguments
/// * `dot_dir` - Path to the dot directory
///
/// # Errors
/// Returns a `GitAutoPilotError` if directory creation fails
fn ensure_dot_dir_exists(dot_dir: &str) -> Result<(), GitAutoPilotError> {
    trace!("Checking if dot directory exists");

    if !Path::new(dot_dir).exists() {
        debug!("Dot directory does not exist, creating: {}", dot_dir);

        fs::create_dir_all(dot_dir)
            .map_err(|e| GitAutoPilotError::DirCreationError(format!("{}: {}", dot_dir, e)))?;

        debug!("Dot directory created successfully");
    }

    Ok(())
}

/// Loads existing configuration or creates a default one
///
/// # Arguments
/// * `dot_file` - Path to the configuration file
///
/// # Returns
/// A `Config` instance, either loaded from file or default
///
/// # Errors
/// Returns a `GitAutoPilotError` if file operations fail
fn load_or_create_config(dot_file: &str) -> Result<config::Config, GitAutoPilotError> {
    trace!("Checking configuration file existence");

    let config_path = PathBuf::from(dot_file);

    if !config_path.exists() {
        debug!(
            "Configuration file does not exist, creating default: {}",
            dot_file
        );

        let default_config = config::Config::default();
        config::Config::save_to_file(&default_config, &config_path)
            .map_err(|e| GitAutoPilotError::ConfigError(ConfigError::FileError(e.to_string())))?;

        debug!("Default configuration file created");
        Ok(default_config)
    } else {
        debug!("Configuration file exists, loading: {}", dot_file);

        config::Config::load_from_file(&config_path).map_err(|e| GitAutoPilotError::ConfigError(e))
    }
}
