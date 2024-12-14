use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use config::ConfigError;
use git2::Repository;
use log::{debug, error, info, trace, warn};
use notify::Event;
use notify::EventKind;
use notify::RecursiveMode;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use tokio::task;
use tokio::task::JoinError;

mod config;
mod git;
mod helper;
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

    /// Wrapper for standard notify errors
    #[error(transparent)]
    NotifyError(#[from] notify::Error),

    /// Wrapper for standard tokio join errors
    #[error(transparent)]
    TokioJoinError(#[from] JoinError),
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

    /// Watches file system changes in specified repositories and processes the events.
    ///
    /// # Arguments
    /// - `self` - The current instance of the struct containing configuration and other details.
    ///
    /// # Returns
    /// - `Result<(), GitAutoPilotError>` - Returns `Ok(())` if successful, otherwise an error of type `GitAutoPilotError`.
    ///
    /// # Behavior
    /// 1. Creates a standard library channel and a Tokio channel for event handling.
    /// 2. Configures a file watcher for directories specified in the configuration.
    /// 3. Bridges events from the standard channel to the Tokio channel.
    /// 4. Processes events asynchronously to handle file system changes.
    ///
    /// # Errors
    /// - Returns an error if the watcher setup or event processing fails.
    pub async fn watch(self) -> Result<(), GitAutoPilotError> {
        trace!("Starting watch function...");

        // Create a standard library channel for file system events
        let (tx, rx) = mpsc::channel();

        // Tokio channel for async processing
        let (async_tx, mut async_rx) = tokio::sync::mpsc::channel(100);

        // Configure watcher
        let mut watcher = helper::create_watcher(tx)?;

        // Directories to watch
        let watch_paths = &self.config.repos;

        // Ignored directories
        let ignored_dirs: Vec<String> = self.config.ignored_dirs;

        // Watch multiple directories
        for path in watch_paths {
            info!("Adding watch for path: {:#?}", path);
            watcher.watch(Path::new(path), RecursiveMode::Recursive)?;
        }

        // Spawn a task to bridge standard channel to Tokio channel
        let bridge_handle = task::spawn(async move {
            for event in rx {
                trace!("Received event: {:?}", event);
                if let Err(_) = async_tx.send(event).await {
                    error!("Failed to send event through async channel");
                    break;
                }
            }
        });

        // Process events
        while let Some(result) = async_rx.recv().await {
            match result {
                Ok(event) => {
                    debug!("Handling event: {:?}", event);
                    trace!("Finding correct repo that triggered event");

                    // Check if the event is in an ignored directory
                    if event.paths.iter().any(|path| {
                        ignored_dirs.iter().any(|ignored| {
                            path.to_string_lossy().contains(&format!("/{}", ignored))
                        })
                    }) {
                        trace!("Ignoring event for paths: {:?}", event.paths);
                        continue;
                    }

                    if let Some(repo) = get_matching_repository(&event.paths[0], &self.config.repos)
                    {
                        debug!("Matched repository for event: {:?}", repo);
                        handle_event(&event, &repo);
                    } else {
                        debug!("No matching repository found for paths: {:?}", event.paths);
                    }
                }
                Err(e) => error!("Watch error: {:?}", e),
            }
        }

        // Wait for the bridge task to complete
        bridge_handle.await?;
        info!("Watch function completed successfully.");
        Ok(())
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

    let home_dir_from_dirs = dir::home_dir();

    let dot_dir = home_dir_from_dirs
        .map(|path| format!("{}/{}", path.display(), DOT_DIR))
        .or_else(|| {
            warn!("Could not retrieve home directory via dirs");
            std::env::var("HOME")
                .map(|home| format!("{}/{}", home, DOT_DIR))
                .ok()
        })
        .ok_or_else(|| {
            error!("Failed to determine home directory through both dirs and HOME environment variable");
            GitAutoPilotError::HomeDirError
        })?;

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

/// Handles a single file system event by analyzing changes in the corresponding Git repository.
///
/// # Arguments
/// - `event` - The file system event to be handled.
/// - `repo` - The path to the Git repository related to the event.
///
/// # Behavior
/// - Analyzes repository changes for specified file paths.
/// - Logs detailed information about the changes.
fn handle_event(event: &Event, repo: &Path) {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
            for path in &event.paths {
                trace!("Path  - {}", path.display());
                info!(
                    "{:#?}",
                    git::analyze_repository_changes(&Repository::open(repo).unwrap())
                );
            }
        }
        _ => {}
    }
}

/// Finds the repository that matches a given file path.
///
/// # Arguments
/// - `path` - The file system path to match.
/// - `repos` - A list of repository paths to search.
///
/// # Returns
/// - `Option<&Path>` - Returns a reference to the matching repository path, or `None` if no match is found.
///
/// # Behavior
/// - Checks if the given path is contained within any of the repository paths.
fn get_matching_repository<P: AsRef<Path>>(path: P, repos: &[PathBuf]) -> Option<&Path> {
    repos
        .iter()
        .find(|r| {
            r.to_str().map_or(false, |r_str| {
                path.as_ref().to_string_lossy().contains(r_str)
            })
        })
        .map(|r| r.as_path())
}
