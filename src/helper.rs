use git2::Status;
use log::{debug, error, trace, warn};
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, Watcher, WatcherKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::config::{Config, ConfigError, GitCred};
use crate::error::GitAutoPilotError;

/// Constant for the default git credentials file
const DOT_GIT_CREDENTIALS: &str = ".git-credentials";

/// Constant for the default git config file
const DOT_GIT_CONFIG: &str = ".gitconfig";

/// Creates a file system watcher with optimized configuration based on the recommended watcher type.
///
/// This function initializes a file system watcher that can detect changes in the file system.
/// It adapts the watcher configuration based on the detected watcher kind, providing
/// a custom polling interval for poll-based watchers.
///
/// # Parameters
/// - `tx`: A channel sender for broadcasting file system events or errors
///
/// # Returns
/// A boxed file system watcher implementing the `Watcher` trait
///
/// # Errors
/// Returns a `notify::Error` if the watcher fails to initialize
///
/// # Examples
/// ```
/// let (tx, rx) = mpsc::channel();
/// let watcher = create_watcher(tx)?;
///
pub fn create_watcher(
    tx: mpsc::Sender<Result<Event, notify::Error>>,
) -> Result<Box<dyn Watcher>, notify::Error> {
    log::trace!("Initializing file system watcher...");

    let watcher: Box<dyn Watcher> = if RecommendedWatcher::kind() == WatcherKind::PollWatcher {
        log::info!("Detected PollWatcher kind. Applying custom polling interval.");
        let config = NotifyConfig::default()
            .with_poll_interval(Duration::from_secs(1))
            .with_compare_contents(true);

        Box::new(RecommendedWatcher::new(tx, config)?)
    } else {
        log::info!("Detected default watcher kind. Using default configuration.");
        Box::new(RecommendedWatcher::new(tx, NotifyConfig::default())?)
    };

    log::debug!("File system watcher created successfully.");
    Ok(watcher)
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
pub fn get_matching_repository<P: AsRef<Path>>(path: P, repos: &[PathBuf]) -> Option<&Path> {
    repos
        .iter()
        .find(|r| {
            r.to_str().map_or(false, |r_str| {
                path.as_ref().to_string_lossy().contains(r_str)
            })
        })
        .map(|r| r.as_path())
}

/// Returns the path to a git-related file in the user's home directory
///
/// # Arguments
/// * `filename` - Name of the file to locate (e.g., ".git-credentials", ".gitconfig")
///
/// # Returns
/// * `Result<String, GitAutoPilotError>` - Full path to the file if successful
///
/// # Errors
/// * `GitAutoPilotError::HomeDirError` - If home directory cannot be determined
pub fn get_git_path(filename: &str) -> Result<String, GitAutoPilotError> {
    trace!("Attempting to locate {}", filename);

    dir::home_dir()
        .map(|path| format!("{}/{}", path.display(), filename))
        .or_else(|| {
            warn!("Could not retrieve home directory via dirs");
            std::env::var("HOME")
                .map(|home| format!("{}/{}", home, filename))
                .ok()
        })
        .ok_or_else(|| {
            error!("Failed to determine home directory");
            GitAutoPilotError::HomeDirError
        })
}

/// Reads and populates Git credentials from the user's .git-credentials and .gitconfig files
///
/// # Arguments
/// * `config` - Mutable reference to the configuration struct that will store the credentials
///
/// # Returns
/// * `Result<(), GitAutoPilotError>` - Ok(()) if successful, or appropriate error if failed
///
/// # Errors
/// * `GitAutoPilotError::HomeDirError` - If home directory cannot be determined
/// * `GitAutoPilotError::ConfigError::FileError` - If credentials file cannot be read or parsed
///
/// This function will:
/// 1. Skip if credentials are already populated
/// 2. Locate and read .git-credentials file
/// 3. Parse GitHub credentials (username and password)
/// 4. Read git config for email and username
/// 5. Populate the config struct with all credentials
pub fn populate_git_credentials(config: &mut Config) -> Result<(), GitAutoPilotError> {
    // Initialize git_credentials if None
    if config.git_credentials.is_none() {
        config.git_credentials = Some(GitCred {
            login_username: None,
            password: None,
            email: String::new(),
            username: String::new(),
        });
    }

    let git_cred = config.git_credentials.as_mut().unwrap();

    // Check if we need to parse .git-credentials
    let needs_git_credentials = git_cred
        .login_username
        .as_ref()
        .map_or(true, |username| username.is_empty())
        || git_cred
            .password
            .as_ref()
            .map_or(true, |password| password.is_empty());

    if needs_git_credentials {
        debug!("Attempting to populate git credentials from .git-credentials");
        let dot_git_credentials = get_git_path(DOT_GIT_CREDENTIALS)?;

        // Read credentials file
        let credentials_path = Path::new(&dot_git_credentials);
        let credentials_content = std::fs::read_to_string(credentials_path).map_err(|err| {
            error!(
                "Failed to read .git-credentials at {}: {}",
                credentials_path.display(),
                err
            );
            GitAutoPilotError::ConfigError(ConfigError::FileError(format!(
                "Failed to read .git-credentials at: {}",
                credentials_path.display()
            )))
        })?;

        // Parse GitHub credentials
        let (username, password) =
            parse_specific_domain_credentials(&credentials_content, "github.com")?;

        // Only update if values are None or empty
        if git_cred
            .login_username
            .as_ref()
            .map_or(true, |login_username| login_username.is_empty())
        {
            git_cred.login_username = Some(username);
        }
        if git_cred
            .password
            .as_ref()
            .map_or(true, |password| password.is_empty())
        {
            git_cred.password = Some(password);
        }
    }

    // Check if we need to parse .gitconfig
    let needs_git_config = git_cred.username.is_empty() || git_cred.email.is_empty();

    if needs_git_config {
        debug!("Attempting to populate git config values");
        let dot_git_config = get_git_path(DOT_GIT_CONFIG)?;
        let config_path = Path::new(&dot_git_config);
        let config_content = std::fs::read_to_string(config_path).map_err(|err| {
            error!(
                "Failed to read .gitconfig at {}: {}",
                config_path.display(),
                err
            );
            GitAutoPilotError::ConfigError(ConfigError::FileError(format!(
                "Failed to read .gitconfig at: {}",
                config_path.display()
            )))
        })?;

        let (git_email, git_username) = parse_git_config(&config_content)?;

        // Only update if values are empty
        if git_cred.email.is_empty() {
            git_cred.email = git_email;
        }
        if git_cred.username.is_empty() {
            git_cred.username = git_username;
        }
    }

    trace!(
        "Git credentials status - Username: {}, Email: {}, Login Username: {}, Password: {}",
        if git_cred.username.is_empty() {
            "not set"
        } else {
            &git_cred.username
        },
        if git_cred.email.is_empty() {
            "not set"
        } else {
            &git_cred.email
        },
        git_cred.login_username.as_deref().unwrap_or("not set"),
        if git_cred.password.is_some() {
            "*******"
        } else {
            "not set"
        }
    );

    Ok(())
}

/// Helper function to parse specific domain credentials from .git-credentials content
pub fn parse_specific_domain_credentials(
    content: &str,
    domain: &str,
) -> Result<(String, String), GitAutoPilotError> {
    for line in content.lines() {
        if line.contains(domain) {
            if let Some(credentials) = line.strip_prefix("https://") {
                if let Some((user_pass, _)) = credentials.split_once('@') {
                    if let Some((user, pass)) = user_pass.split_once(':') {
                        return Ok((user.trim().to_string(), pass.trim().to_string()));
                    }
                }
            }
        }
    }

    error!("Failed to parse GitHub credentials");
    Err(GitAutoPilotError::ConfigError(ConfigError::FileError(
        "Failed to parse username or password for github.com".to_string(),
    )))
}

/// Helper function to parse email and username from .gitconfig content
pub fn parse_git_config(content: &str) -> Result<(String, String), GitAutoPilotError> {
    let mut email = String::new();
    let mut username = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("email = ") {
            email = line.trim_start_matches("email = ").trim().to_string();
        } else if line.starts_with("name = ") {
            username = line.trim_start_matches("name = ").trim().to_string();
        }
    }

    if email.is_empty() || username.is_empty() {
        error!("Failed to parse git config - email or username missing");
        return Err(GitAutoPilotError::ConfigError(ConfigError::FileError(
            "Failed to parse email or username from .gitconfig".to_string(),
        )));
    }

    Ok((email, username))
}

pub fn status_to_string(status: Status) -> String {
    match status {
        Status::WT_NEW => "WT_NEW".to_string(),
        Status::WT_MODIFIED => "WT_MODIFIED".to_string(),
        Status::WT_DELETED => "WT_DELETED".to_string(),
        Status::WT_RENAMED => "WT_RENAMED".to_string(),
        Status::WT_TYPECHANGE => "WT_TYPECHANGE".to_string(),
        Status::INDEX_NEW => "INDEX_NEW".to_string(),
        Status::INDEX_MODIFIED => "INDEX_MODIFIED".to_string(),
        Status::INDEX_DELETED => "INDEX_DELETED".to_string(),
        Status::INDEX_RENAMED => "INDEX_RENAMED".to_string(),
        Status::INDEX_TYPECHANGE => "INDEX_TYPECHANGE".to_string(),
        Status::CONFLICTED => "CONFLICTED".to_string(),
        Status::IGNORED => "IGNORED".to_string(),
        _ => "UNKNOWN".to_string(),
    }
}

