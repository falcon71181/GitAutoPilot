//! # Configuration Module for Git Commit and Description Management
//!
//! This module provides a flexible configuration system for generating
//! commit messages and descriptions with customizable templates and variables.
//!
//! ## Features
//! - Customizable commit message templates
//! - Flexible variable substitution
//! - Serializable and deserializable configuration
//! - Default configurations with easy customization

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Represents credentials for authenticating with a Git repository.
///
/// This structure is used to store and manage the authentication
/// details required for operations such as cloning, pushing, or pulling.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitCred {
    /// The username for committing.
    pub username: String,

    /// The email address associated with the Git user.
    pub email: String,

    /// The username for authentication.
    pub login_username: Option<String>,

    /// The password or personal access token for authentication.
    pub password: Option<String>,
}

/// Represents a message template with prefix, comment, and suffix
///
/// This struct defines the format for generating commit messages. It includes:
/// - `prefix`: Text that appears before the main comment (e.g., "[Create]").
/// - `comment`: The main body of the message, which may include placeholders for variables (e.g., "File {{FILE_NAME}} created").
/// - `suffix`: Text that appears after the main comment (e.g., a timestamp or additional info).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Prefix text for the message
    pub prefix: String,

    /// Main comment body with potential variable placeholders
    pub comment: String,

    /// Suffix text for the message
    pub suffix: String,
}

/// Defines commit summary message templates for different operation types
///
/// This struct contains templates for generating commit summaries based on
/// file operations. It supports three operations:
/// - `create`: Template for file creation events
/// - `modify`: Template for file modification events
/// - `remove`: Template for file removal events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSummary {
    /// Template for file creation events
    pub create: Message,

    /// Template for file modification events
    pub modify: Message,

    /// Template for file removal events
    pub remove: Message,

    /// Template for file rename events
    pub rename: Message,
}

/// Defines detailed description templates for different operation types
///
/// This struct contains templates for generating commit descriptions based on
/// file operations. It includes detailed information about the file, such as:
/// - `create`: Description template for file creation events
/// - `modify`: Description template for file modification events
/// - `remove`: Description template for file removal events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// Template for file creation descriptions
    pub create: Message,

    /// Template for file modification descriptions
    pub modify: Message,

    /// Template for file removal descriptions
    pub remove: Message,

    /// Template for file rename descriptions
    pub rename: Message,
}

/// Configuration error types
///
/// This enum defines the types of errors that may occur when working with the
/// configuration. These errors include:
/// - `JsonParseError`: Triggered when JSON parsing fails.
/// - `FileError`: Triggered when file operations (reading or writing) fail.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Occurs when JSON parsing fails
    #[error("Failed to parse configuration JSON: {0}")]
    JsonParseError(#[from] serde_json::Error),

    /// Occurs when file operations fail
    #[error("File operation error: {0}")]
    FileError(String),
}

// Log the error details when the ConfigError is being dropped
impl Drop for ConfigError {
    fn drop(&mut self) {
        log::error!("{}", self);
    }
}

/// Main configuration structure
///
/// This struct holds the entire configuration for generating commit messages
/// and descriptions. It includes:
/// - `message`: Commit summary message templates
/// - `description`: Detailed description templates
/// - `variables`: Custom variables for template substitution
/// - `repos`: List of repository paths to track
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// Commit summary message templates
    pub message: CommitSummary,

    /// Detailed description templates
    pub description: Description,

    /// Custom variables for template substitution
    #[serde(default = "default_variables")]
    pub variables: serde_json::Value,

    /// List of repository paths to track
    #[serde(default)]
    pub repos: Vec<PathBuf>,

    /// List of dirs to ignore events
    #[serde(default)]
    pub ignored_dirs: Vec<String>,

    /// contains git credentials
    #[serde(default)]
    pub git_credentials: Option<GitCred>,
}

/// Default system variables
///
/// These are system-defined variables that can be substituted in the message
/// and description templates. These include:
/// - `INSERTIONS`: Number of lines inserted
/// - `DELETIONS`: Number of lines deleted
/// - `LINES_MODIFIED`: Number of lines modified
/// - `BRANCH`: Current branch name
/// - `STATUS`: Current status (e.g., staged, modified)
/// - `FILE_NAME_SHORT`: Short file name
/// - `FILE_NAME_FULL`: Full file name
pub const SYSTEM_VARIABLES: &[(&str, &str)] = &[
    ("INSERTIONS", "INSERTIONS"),
    ("DELETIONS", "DELETIONS"),
    ("LINES_MODIFIED", "LINES_MODIFIED"),
    ("BRANCH", "BRANCH"),
    ("STATUS", "STATUS"),
    ("FILE_NAME_SHORT", "FILE_NAME_SHORT"),
    ("FILE_NAME_FULL", "FILE_NAME_FULL"),
    ("FILE_OLD_NAME", "FILE_OLD_NAME"),
];

/// Creates default variables with system and custom variables
///
/// This function initializes a `serde_json::Value::Object` that contains both
/// system-defined variables and any additional custom variables. By default,
/// an example custom variable (`example_var`) is included in the generated map.
///
/// # Returns
/// Returns a `serde_json::Value::Object` containing the default variables.
fn default_variables() -> serde_json::Value {
    let mut vars = serde_json::Map::new();

    // Add system variables
    for (key, value) in SYSTEM_VARIABLES {
        vars.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }

    serde_json::Value::Object(vars)
}

impl Default for Message {
    fn default() -> Self {
        Message {
            prefix: String::new(),
            comment: String::new(),
            suffix: String::new(),
        }
    }
}

impl Default for CommitSummary {
    fn default() -> Self {
        CommitSummary {
            create: Message::default(),
            modify: Message::default(),
            remove: Message::default(),
            rename: Message::default(),
        }
    }
}

impl CommitSummary {
    /// Provides a default configuration for commit summaries
    ///
    /// This function initializes a `CommitSummary` struct with default message
    /// templates for file creation, modification, and removal events. Each of
    /// these templates contains a simple comment with the placeholder `{{FILE_NAME_SHORT}}`.
    ///
    /// # Returns
    /// Returns a `CommitSummary` with the default commit message templates.
    pub fn default() -> Self {
        Self {
            create: Message {
                prefix: String::new(),
                comment: "New File Created: {{FILE_NAME_SHORT}}".to_string(),
                suffix: String::new(),
            },
            modify: Message {
                prefix: String::new(),
                comment: "File Modified: {{FILE_NAME_SHORT}}".to_string(),
                suffix: String::new(),
            },
            remove: Message {
                prefix: String::new(),
                comment: "File Removed: {{FILE_NAME_SHORT}}".to_string(),
                suffix: String::new(),
            },
            rename: Message {
                prefix: String::new(),
                comment: "File Renamed: {{FILE_NAME_SHORT}}".to_string(),
                suffix: String::new(),
            },
        }
    }
}

impl Description {
    /// Provides a default configuration for detailed descriptions
    ///
    /// This function initializes a `Description` struct with default message
    /// templates for file creation, modification, and removal events. Each of
    /// these templates provides detailed information, such as the file name
    /// and number of lines inserted, deleted, and modified.
    ///
    /// # Returns
    /// Returns a `Description` with the default detailed description templates.
    pub fn default() -> Self {
        Self {
            create: Message {
                prefix: String::new(),
                comment: concat!(
                    "New File Created\n",
                    "File short name: {{FILE_NAME_SHORT}}\n",
                    "File full name: {{FILE_NAME_FULL}}\n",
                    "No. of lines inserted: {{INSERTIONS}}\n",
                    "No. of lines deleted: {{DELETIONS}}\n",
                    "No. of lines modified: {{LINES_MODIFIED}}"
                )
                .to_string(),
                suffix: String::new(),
            },
            modify: Message {
                prefix: String::new(),
                comment: concat!(
                    "File Modified\n",
                    "File short name: {{FILE_NAME_SHORT}}\n",
                    "File full name: {{FILE_NAME_FULL}}\n",
                    "No. of lines inserted: {{INSERTIONS}}\n",
                    "No. of lines deleted: {{DELETIONS}}\n",
                    "No. of lines modified: {{LINES_MODIFIED}}"
                )
                .to_string(),
                suffix: String::new(),
            },
            remove: Message {
                prefix: String::new(),
                comment: concat!(
                    "File Removed\n",
                    "File short name: {{FILE_NAME_SHORT}}\n",
                    "File full name: {{FILE_NAME_FULL}}\n",
                    "No. of lines inserted: {{INSERTIONS}}\n",
                    "No. of lines deleted: {{DELETIONS}}\n",
                    "No. of lines modified: {{LINES_MODIFIED}}"
                )
                .to_string(),
                suffix: String::new(),
            },
            rename: Message {
                prefix: String::new(),
                comment: concat!(
                    "File Renamed\n",
                    "File short name: {{FILE_NAME_SHORT}}\n",
                    "File full name: {{FILE_NAME_FULL}}\n",
                    "No. of lines inserted: {{INSERTIONS}}\n",
                    "No. of lines deleted: {{DELETIONS}}\n",
                    "No. of lines modified: {{LINES_MODIFIED}}"
                )
                .to_string(),
                suffix: String::new(),
            },
        }
    }
}

impl Default for Config {
    /// Creates a default configuration with system and custom variables
    ///
    /// This function initializes a `Config` struct with default templates for
    /// commit messages and descriptions, default variables, and an empty list
    /// of repositories.
    ///
    /// # Returns
    /// Returns a `Config` struct with default settings for messages, descriptions,
    /// variables, and repositories.
    fn default() -> Self {
        Config {
            message: CommitSummary::default(),
            description: Description::default(),
            variables: default_variables(),
            repos: Vec::new(),
            ignored_dirs: vec![".git".to_string()],
            git_credentials: None,
        }
    }
}

impl Config {
    /// Loads configuration from a JSON file
    ///
    /// This function reads the configuration from the specified file and
    /// parses it into a `Config` struct. If an error occurs during reading or
    /// parsing, it returns a `ConfigError`.
    ///
    /// # Arguments
    /// - `path`: Path to the JSON file containing the configuration.
    ///
    /// # Errors
    /// Returns a `ConfigError` if the file cannot be read or parsed.
    pub fn load_from_file(path: &PathBuf) -> Result<Self, ConfigError> {
        let config_contents =
            std::fs::read_to_string(path).map_err(|e| ConfigError::FileError(e.to_string()))?;

        let config: Config = serde_json::from_str(&config_contents)?;
        Ok(config)
    }

    /// Saves the configuration to a JSON file
    ///
    /// This function serializes the `Config` struct into JSON format and writes it
    /// to the specified file. If an error occurs during writing, it returns a
    /// `ConfigError`.
    ///
    /// # Arguments
    /// - `path`: Path to the file where the configuration should be saved.
    ///
    /// # Errors
    /// Returns a `ConfigError` if the file cannot be written.
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), ConfigError> {
        let config_json = serde_json::to_string_pretty(self).map_err(ConfigError::from)?;

        std::fs::write(path, config_json).map_err(|e| ConfigError::FileError(e.to_string()))
    }

    /// Merges another configuration into the current one
    ///
    /// This function allows you to update an existing configuration with values from
    /// another configuration. It updates only the fields that are not empty in the
    /// provided configuration.
    ///
    /// # Arguments
    /// - `other`: The configuration to merge into the current one.
    pub fn merge(&mut self, other: Config) {
        if !other.message.create.comment.is_empty() {
            self.message.create = other.message.create;
        }
        if !other.message.modify.comment.is_empty() {
            self.message.modify = other.message.modify;
        }
        if !other.message.remove.comment.is_empty() {
            self.message.remove = other.message.remove;
        }

        if !other.description.create.comment.is_empty() {
            self.description.create = other.description.create;
        }
        if !other.description.modify.comment.is_empty() {
            self.description.modify = other.description.modify;
        }
        if !other.description.remove.comment.is_empty() {
            self.description.remove = other.description.remove;
        }

        // Merge variables
        if let serde_json::Value::Object(other_vars) = other.variables {
            if let serde_json::Value::Object(current_vars) = &mut self.variables {
                current_vars.extend(other_vars);
            }
        }

        // Merge repositories
        self.repos.extend(other.repos);
        self.ignored_dirs.extend(other.ignored_dirs);
    }
}

/// Example usage
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.repos.is_empty());
        assert!(!config.variables.is_null());
    }

    #[test]
    fn test_config_merge() {
        let mut base_config = Config::default();

        // Create a configuration with specific fields to update
        let update_config = Config {
            message: CommitSummary {
                create: Message {
                    comment: "Custom Create Message".to_string(),
                    ..Default::default() // Use default values for other fields
                },
                ..Default::default() // Use default values for other fields
            },
            variables: serde_json::json!({"new_var": "test_value"}),
            repos: vec![PathBuf::from("/test/repo")],
            ..Default::default() // Use default values for other fields
        };

        // Merge update_config into base_config
        base_config.merge(update_config);

        // Test that the "create" commit message was updated
        assert_eq!(base_config.message.create.comment, "Custom Create Message");

        // Test that the new variable "new_var" was added to variables
        assert!(base_config.variables["new_var"].as_str().is_some());
        assert_eq!(
            base_config.variables["new_var"].as_str().unwrap(),
            "test_value"
        );

        // Test that the repository was added
        assert_eq!(base_config.repos.len(), 1);
        assert_eq!(base_config.repos[0], PathBuf::from("/test/repo"));

        // Ensure that other fields are not overwritten by the merge
        // The default values should remain as-is for fields that are not updated in update_config
        assert_eq!(
            base_config.message.modify.comment,
            "File Modified: {{FILE_NAME_SHORT}}"
        );
        assert_eq!(
            base_config.message.remove.comment,
            "File Removed: {{FILE_NAME_SHORT}}"
        );

        // Test that variables not included in the update remain unchanged
        assert!(base_config.variables["INSERTIONS"].as_str().is_some());
    }
}
