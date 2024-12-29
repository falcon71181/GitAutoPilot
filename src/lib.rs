use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use config::{ConfigError, Message, SYSTEM_VARIABLES};
use error::GitAutoPilotError;
use git::FileChangeStats;
use git2::{Repository, Status};
use log::{debug, error, info, trace};
use notify::Event;
use notify::EventKind;
use notify::RecursiveMode;
use serde::Deserialize;
use serde::Serialize;
use tokio::task;

mod config;
mod error;
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
        let mut config = load_or_create_config(&dot_file)?;

        // check and populate git credentials
        helper::populate_git_credentials(&mut config)?;

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
        let ignored_dirs: &Vec<String> = &self.config.ignored_dirs;

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
                    // Check if the event is in an ignored directory
                    if event.paths.iter().any(|path| {
                        ignored_dirs.iter().any(|ignored| {
                            path.to_string_lossy().contains(&format!("/{}", ignored))
                        })
                    }) {
                        continue;
                    }

                    debug!("Handling event: {:?}", event);
                    trace!("Finding correct repo that triggered event");

                    if let Some(repo) =
                        helper::get_matching_repository(&event.paths[0], &self.config.repos)
                    {
                        debug!("Matched repository for event: {:?}", repo);
                        let _ = Self::handle_event(&self, &event, &repo);
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

    /// Handles a single file system event by analyzing changes in the corresponding Git repository.
    ///
    /// # Arguments
    /// - `event` - The file system event to be handled.
    /// - `repo` - The path to the Git repository related to the event.
    ///
    /// # Behavior
    /// - Analyzes repository changes for specified file paths.
    /// - Logs detailed information about the changes.
    fn handle_event(&self, event: &Event, repo: &Path) -> Result<(), GitAutoPilotError> {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                for path in &event.paths {
                    trace!("Path  - {}", &path.display());
                    let repo = match Repository::open(repo) {
                        Ok(repo) => repo,
                        Err(e) => {
                            error!("Failed to open repository: {}", e);
                            continue; // Skip to the next event
                        }
                    };
                    if let Some(ref cred) = self.config.git_credentials {
                        trace!("Custom user.name: {:#?}", &cred.username);
                        trace!("Custom user.email: {:#?}", &cred.email);
                        // Set user configuration (username and email)
                        let mut config = repo.config()?;
                        config.set_str("user.name", &cred.username)?;
                        config.set_str("user.email", &cred.email)?;
                    }
                    let git_changes = git::analyze_repository_changes(&repo)?;
                    if git_changes.is_empty() {
                        trace!("No git changes found");
                        continue;
                    }
                    debug!("git_changes={:#?}", git_changes);
                    let file_name = path
                        .display()
                        .to_string()
                        .strip_prefix(repo.path().parent().unwrap().to_str().unwrap_or_default())
                        .unwrap_or_default()
                        .to_string()[1..]
                        .to_string();
                    if let Some(stats) = git_changes
                        .get(&file_name)
                        // NOTE: in case of rename operation, take first value
                        .or_else(|| git_changes.values().next())
                    {
                        if let Some(file_changes) = stats.first() {
                            match file_changes.status {
                                Status::WT_RENAMED => {
                                    trace!("Rename operation found");
                                    let _take_git_action = Self::take_action(
                                        self,
                                        &repo,
                                        file_changes,
                                        git_changes.keys().next().unwrap(),
                                        &format!(
                                            "{}/{}",
                                            path.to_str()
                                                .unwrap_or_default()
                                                .split("/")
                                                .collect::<Vec<&str>>()[..path
                                                .to_str()
                                                .unwrap_or_default()
                                                .split("/")
                                                .count()
                                                - 1]
                                                .join("/"),
                                            git_changes.keys().next().unwrap()
                                        ),
                                    );
                                }
                                _ => {
                                    let _take_git_action = Self::take_action(
                                        self,
                                        &repo,
                                        file_changes,
                                        &file_name,
                                        path.to_str().unwrap_or(&file_name),
                                    );
                                }
                            }
                        }
                    } else {
                        continue;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn take_action(
        &self,
        repo: &Repository,
        file_change_stats: &FileChangeStats,
        short_file_name: &str,
        full_file_name: &str,
    ) -> Result<(), GitAutoPilotError> {
        debug!("full_file_name={:#?}", full_file_name);
        debug!("short_file_name={:#?}", short_file_name);
        trace!("{:#?} staging", full_file_name);
        let repo_branch = git::get_current_branch(repo).unwrap_or("master".to_string());
        let dynamic_values = Self::prepare_dynamic_values(
            self,
            &repo_branch,
            short_file_name.to_string(),
            full_file_name.to_string(),
            file_change_stats,
        );
        match file_change_stats.status {
            Status::WT_NEW | Status::INDEX_NEW => {
                let _git_stage_file = git::stage_file(&repo, short_file_name, false)?;
                let (message, description) = get_commit_summary(
                    dynamic_values,
                    &self.config.message.create,
                    &self.config.description.create,
                );
                let _git_commit_stagged_change = git::commit(&repo, &message, Some(&description))?;
                if let Some(git_credentials) = self.config.git_credentials.as_ref() {
                    let username = git_credentials.login_username.as_ref().unwrap();
                    let password = git_credentials.password.as_ref().unwrap();

                    git::push(repo, username, password, "origin", &repo_branch)?;
                } else {
                    error!("Git credentials are not set");
                    return Err(GitAutoPilotError::ConfigError(ConfigError::FileError(
                        "Git credentials are not set".to_string(),
                    )));
                }
            }
            Status::WT_RENAMED => {
                if let Some(old_name) = file_change_stats.old_name.as_ref() {
                    let _git_stage_file = git::stage_file(&repo, old_name, true)?;
                }
                let _git_stage_file = git::stage_file(&repo, short_file_name, false)?;
                let (message, description) = get_commit_summary(
                    dynamic_values,
                    &self.config.message.rename,
                    &self.config.description.rename,
                );
                let _git_commit_stagged_change = git::commit(&repo, &message, Some(&description))?;
                if let Some(git_credentials) = self.config.git_credentials.as_ref() {
                    let username = git_credentials.login_username.as_ref().unwrap();
                    let password = git_credentials.password.as_ref().unwrap();

                    git::push(repo, username, password, "origin", &repo_branch)?;
                } else {
                    error!("Git credentials are not set");
                    return Err(GitAutoPilotError::ConfigError(ConfigError::FileError(
                        "Git credentials are not set".to_string(),
                    )));
                }
            }
            Status::WT_DELETED => {
                let _git_stage_file = git::stage_file(&repo, short_file_name, true)?;
                let (message, description) = get_commit_summary(
                    dynamic_values,
                    &self.config.message.remove,
                    &self.config.description.remove,
                );
                let _git_commit_stagged_change = git::commit(&repo, &message, Some(&description))?;
                if let Some(git_credentials) = self.config.git_credentials.as_ref() {
                    let username = git_credentials.login_username.as_ref().unwrap();
                    let password = git_credentials.password.as_ref().unwrap();

                    git::push(repo, username, password, "origin", &repo_branch)?;
                } else {
                    error!("Git credentials are not set");
                    return Err(GitAutoPilotError::ConfigError(ConfigError::FileError(
                        "Git credentials are not set".to_string(),
                    )));
                }
            }
            // NOTE: else modified
            _ => {
                let _git_stage_file = git::stage_file(&repo, short_file_name, false)?;
                let (message, description) = get_commit_summary(
                    dynamic_values,
                    &self.config.message.modify,
                    &self.config.description.modify,
                );
                let _git_commit_stagged_change = git::commit(&repo, &message, Some(&description))?;
                if let Some(git_credentials) = self.config.git_credentials.as_ref() {
                    let username = git_credentials.login_username.as_ref().unwrap();
                    let password = git_credentials.password.as_ref().unwrap();

                    git::push(repo, username, password, "origin", &repo_branch)?;
                } else {
                    error!("Git credentials are not set");
                    return Err(GitAutoPilotError::ConfigError(ConfigError::FileError(
                        "Git credentials are not set".to_string(),
                    )));
                }
            }
        }
        Ok(())
    }

    fn prepare_dynamic_values(
        &self,
        branch: &str,
        short_file_name: String,
        full_file_name: String,
        file_change_stats: &FileChangeStats,
    ) -> HashMap<String, String> {
        let mut dynamic_values: HashMap<String, String> = HashMap::new();
        dynamic_values.insert("BRANCH".to_string(), branch.to_owned());
        dynamic_values.insert(
            "STATUS".to_string(),
            helper::status_to_string(file_change_stats.status),
        );
        dynamic_values.insert("FILE_NAME_SHORT".to_string(), short_file_name.to_owned());
        dynamic_values.insert("FILE_NAME_FULL".to_string(), full_file_name.to_owned());
        match file_change_stats.status {
            Status::WT_RENAMED => {
                dynamic_values.insert(
                    "FILE_OLD_NAME".to_string(),
                    file_change_stats
                        .old_name
                        .clone()
                        .unwrap_or_else(|| short_file_name),
                );
            }
            _ => {
                dynamic_values.insert("FILE_OLD_NAME".to_string(), short_file_name);
            }
        }
        dynamic_values.insert(
            "DELETIONS".to_string(),
            file_change_stats.lines_deleted.to_string(),
        );
        dynamic_values.insert(
            "LINES_MODIFIED".to_string(),
            file_change_stats.lines_modified.to_string(),
        );
        dynamic_values.insert(
            "INSERTIONS".to_string(),
            file_change_stats.lines_added.to_string(),
        );

        // Insert system variables into the HashMap
        for &(key, value) in SYSTEM_VARIABLES {
            dynamic_values.insert(
                key.to_string(),
                byteutils::string::replace_multiple_placeholders(
                    &format!("{{{{{}}}}}", value),
                    &dynamic_values,
                ),
            );
        }

        if let serde_json::Value::Object(config_map) = &self.config.variables {
            for (key, value) in config_map {
                if let serde_json::Value::String(ref val) = value {
                    if !dynamic_values.contains_key(key) {
                        dynamic_values.insert(key.to_string(), val.to_string());
                    }
                }
            }
        }
        trace!("dynamic_values={:#?}", dynamic_values);
        dynamic_values
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
    helper::get_git_path(DOT_DIR)
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

fn get_commit_summary(
    dynamic_values: HashMap<String, String>,
    message: &Message,
    description: &Message,
) -> (String, String) {
    let commit_message = format!(
        "{}{}{}",
        byteutils::string::replace_multiple_placeholders(&message.prefix, &dynamic_values),
        byteutils::string::replace_multiple_placeholders(&message.comment, &dynamic_values),
        byteutils::string::replace_multiple_placeholders(&message.suffix, &dynamic_values)
    );
    let commit_description = format!(
        "{}{}{}",
        byteutils::string::replace_multiple_placeholders(&description.prefix, &dynamic_values),
        byteutils::string::replace_multiple_placeholders(&description.comment, &dynamic_values),
        byteutils::string::replace_multiple_placeholders(&description.suffix, &dynamic_values)
    );

    (commit_message, commit_description)
}
