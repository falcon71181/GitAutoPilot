use git2::{DiffOptions, Error as GitError, IndexAddOption, Repository, Status, StatusOptions};
use log::{debug, error, info, trace};
use std::{collections::HashMap, path::Path, process::Command};

/// Detailed information about changes in a file
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChangeStats {
    /// Number of lines added in the file
    pub lines_added: usize,
    /// Number of lines deleted from the file
    pub lines_deleted: usize,
    /// Number of lines deleted from the file
    pub lines_modified: usize,
    /// Overall status of the file change (e.g., added, deleted, modified)
    pub status: Status,
    /// Original name of the file if renamed
    pub old_name: Option<String>,
}

/// Gets the name of the currently checked-out branch.
/// If no branch is found (e.g., in a detached HEAD state), defaults to "master".
///
/// # Arguments
///
/// * `repo` - A reference to the `git2::Repository` object.
///
/// # Returns
///
/// * `Ok(String)` - The name of the current branch.
/// * `Err(GitError)` - In case of any error accessing the repository.
pub fn get_current_branch(repo: &Repository) -> Result<String, GitError> {
    let head = repo.head()?;
    let head_name = head
        .name()
        .ok_or_else(|| GitError::from_str("Failed to get HEAD name"))?;

    let mut path: Vec<&str> = head_name.split('/').collect();
    let branch_name = path.pop().unwrap_or("master");

    Ok(branch_name.to_string())
}

/// Updates a Git repository located at a given path.
/// Optionally forces a reset to the remote repository if `force_update` is `true`.
///
/// # Arguments
///
/// * `repo` - A reference to the `git2::Repository` object.
/// * `force_update` - A boolean flag indicating whether to discard local changes and force an update.
///
/// # Returns
///
/// * `Ok(())` - On success.
/// * `Err(GitError)` - In case of any error accessing or modifying the repository.
pub fn update_repo(repo: &Repository, force_update: bool) -> Result<(), GitError> {
    // Get the current branch name
    let branch_name = get_current_branch(repo)?;

    // Get the directory path for the repository
    let repo_path = repo.path();
    let path = repo_path
        .parent()
        .ok_or_else(|| GitError::from_str("Failed to determine repository path"))?;

    if force_update {
        // Force reset to the remote branch (discard local changes)
        let ref_name = format!("refs/remotes/origin/{}", branch_name);
        let oid = repo.refname_to_id(&ref_name)?;
        let object = repo.find_object(oid, None)?;
        repo.reset(&object, git2::ResetType::Hard, None)?;
    }

    // Pull from the origin repository (using Git CLI)
    let output = Command::new("git")
        .current_dir(path)
        .arg("pull")
        .output()
        .map_err(|e| GitError::from_str(&format!("Failed to execute git pull: {}", e)))?;

    if !output.status.success() {
        return Err(GitError::from_str(&format!(
            "Git pull failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

/// Comprehensive repository change analysis
///
/// # Arguments
///
/// * `repo` - A reference to the `git2::Repository` object.
///
/// # Returns
///
/// * `Result<HashMap<String, Vec<FileChangeStats>>, git2::Error>` - Comprehensive changes grouped by file type
pub fn analyze_repository_changes(
    repo: &Repository,
) -> Result<HashMap<String, Vec<FileChangeStats>>, git2::Error> {
    // Create status options
    let mut status_opts = StatusOptions::new();
    status_opts.include_untracked(true);
    status_opts.recurse_untracked_dirs(true);
    status_opts.include_unmodified(true);

    // Create diff options for additional details
    let mut diff_options = DiffOptions::new();
    diff_options.context_lines(0);

    // Get repository status to capture all changes
    let statuses = repo.statuses(Some(&mut status_opts))?;

    // Analyze changes for each file
    let mut repository_changes: HashMap<String, Vec<FileChangeStats>> = HashMap::new();

    for entry in statuses.iter() {
        let status = entry.status();

        // Skip entries with zero status or ignored files
        if status.is_empty() || status.is_ignored() {
            continue;
        }

        if let Some(path) = entry.path() {
            debug!("Processing path: {} - Status: {:?}", path, status);

            // Try to get more detailed diff information
            let file_stats = match repo.diff_index_to_workdir(None, Some(&mut diff_options)) {
                Ok(diff) => {
                    let stats = diff.stats().map_err(|e| {
                        error!("Error retrieving stats: {:?}", e);
                        e
                    })?;

                    FileChangeStats {
                        lines_added: stats.insertions(),
                        lines_deleted: stats.deletions(),
                        lines_modified: stats.insertions() + stats.deletions(),
                        status,
                        old_name: None,
                    }
                }
                Err(e) => {
                    debug!("Error getting diff for path {}: {:?}", path, e);
                    continue;
                }
            };

            repository_changes
                .entry(path.to_string())
                .or_default()
                .push(file_stats);
        }
    }

    if repository_changes.len() == 2 {
        let keys: Vec<&String> = repository_changes.keys().collect();
        if keys.len() == 2 {
            let first_key = keys[0];
            let second_key = keys[1];

            if let (Some(first_changes), Some(second_changes)) = (
                repository_changes.get(first_key),
                repository_changes.get(second_key),
            ) {
                // Borrow references without cloning
                let old_path_changes = HashMap::from([(first_key.as_str(), &first_changes[0])]);
                let new_path_changes = HashMap::from([(second_key.as_str(), &second_changes[0])]);

                if let Some(renamed_changes) =
                    are_files_renamed(repo, &old_path_changes, &new_path_changes)
                {
                    // Replace the entire repository_changes with the renamed changes
                    repository_changes = renamed_changes
                        .into_iter()
                        .map(|(k, v)| (k, vec![v]))
                        .collect();
                }
            }
        }
    }
    debug!("Repository changes found: {}", repository_changes.len());

    Ok(repository_changes)
}

/// Helper function to filter files by status
pub fn filter_files_by_status<F>(
    repo: &Repository,
    status_check: F,
) -> Result<Vec<String>, git2::Error>
where
    F: Fn(Status) -> bool, // This allows the closure to capture variables
{
    let statuses = repo.statuses(None)?;

    let filtered_files: Vec<String> = statuses
        .iter()
        .filter(|entry| status_check(entry.status()))
        .filter_map(|entry| entry.path().map(|path| path.to_string()))
        .collect();

    Ok(filtered_files)
}

/// Get files with specific status
pub fn get_files_with_status(
    repo: &Repository,
    status: Status,
) -> Result<Vec<String>, git2::Error> {
    filter_files_by_status(repo, |file_status| file_status == status)
}

/// Check if two files are likely a result of a rename operation
fn are_files_renamed<'a>(
    repo: &Repository,
    old_path_changes: &HashMap<&str, &FileChangeStats>,
    new_path_changes: &HashMap<&str, &FileChangeStats>,
) -> Option<HashMap<String, FileChangeStats>> {
    // Early return if either map is empty
    if old_path_changes.is_empty() || new_path_changes.is_empty() {
        return None;
    }

    let old_path = *old_path_changes.keys().next()?;
    let new_path = *new_path_changes.keys().next()?;

    trace!("Checking if files are a result of a rename operation");

    match (
        repo.status_file(Path::new(old_path)),
        repo.status_file(Path::new(new_path)),
    ) {
        (Ok(Status::WT_DELETED), Ok(Status::WT_NEW)) => {
            let old_stats = old_path_changes.get(old_path)?;
            let new_stats = new_path_changes.get(new_path)?;

            // Compare file change statistics with more explicit conditions
            if are_stats_equivalent(old_stats, new_stats) {
                debug!("Changes are the result of rename operation");

                let mut renamed_changes = HashMap::new();
                renamed_changes.insert(
                    new_path.to_string(),
                    FileChangeStats {
                        lines_added: old_stats.lines_added,
                        lines_deleted: old_stats.lines_deleted,
                        lines_modified: old_stats.lines_modified,
                        status: Status::WT_RENAMED,
                        old_name: Some(old_path.to_string()),
                    },
                );

                return Some(renamed_changes);
            }
        }
        _ => {}
    }

    None
}

/// Helper function to check if file change statistics are equivalent
fn are_stats_equivalent(old_stats: &FileChangeStats, new_stats: &FileChangeStats) -> bool {
    old_stats.lines_added == new_stats.lines_added
        && old_stats.lines_deleted == new_stats.lines_deleted
        && old_stats.lines_modified == new_stats.lines_modified
}

/// Stages files in a Git repository matching a given pattern.
///
/// This function is best used when you need to stage multiple files at once
/// or when using wildcards (e.g., "*.rs", "src/*").
///
/// # Arguments
/// * `repo_path` - Path to the Git repository
/// * `file_pattern` - Pattern to match files (e.g., "*", "*.rs", "src/")
///
/// # Errors
/// Returns `GitError` if:
/// * Repository cannot be opened
/// * Index cannot be accessed
/// * Pattern is invalid
/// * Writing to index fails
pub fn add_files(repo_path: impl AsRef<Path>, file_pattern: &str) -> Result<(), GitError> {
    let repo = Repository::open(repo_path)?;
    let mut index = repo.index()?;

    // Use a transaction-like approach for atomic operations
    index.add_all(
        [file_pattern].iter(),
        IndexAddOption::DEFAULT | IndexAddOption::CHECK_PATHSPEC,
        None,
    )?;

    index.write()?;
    info!("Added files matching pattern: {}", file_pattern);
    Ok(())
}
