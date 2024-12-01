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

/// Main configuration structure
///
/// This struct holds the entire configuration for generating commit messages
/// and descriptions. It includes:
/// - `message`: Commit summary message templates
/// - `description`: Detailed description templates
/// - `variables`: Custom variables for template substitution
/// - `repos`: List of repository paths to track
#[derive(Debug, Serialize, Deserialize)]
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
const SYSTEM_VARIABLES: &[(&str, &str)] = &[
    ("INSERTIONS", "insertions"),
    ("DELETIONS", "deletions"),
    ("LINES_MODIFIED", "lines_modified"),
    ("BRANCH", "branch"),
    ("STATUS", "status"),
    ("FILE_NAME_SHORT", "file_name_short"),
    ("FILE_NAME_FULL", "file_name_full"),
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

    // Optional: Add a custom variable example
    vars.insert(
        "example_var".to_string(),
        serde_json::Value::String("example_value".to_string()),
    );

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
    }
}
