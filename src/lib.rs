use serde::Deserialize;
use serde::Serialize;

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
