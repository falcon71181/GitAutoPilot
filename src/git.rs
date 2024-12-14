use git2::{DiffOptions, Error as GitError, Repository, Status, StatusOptions};
use log::debug;
use std::{collections::HashMap, process::Command};

/// Detailed information about changes in a file
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileChangeStats {
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub lines_modified: usize,
    pub status: Status,
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
/// * `Result<HashMap<String, Vec<(Status, FileChangeStats)>>, git2::Error>` - Comprehensive changes grouped by file type
pub fn analyze_repository_changes(
    repo: &Repository,
) -> Result<HashMap<String, Vec<(Status, FileChangeStats)>>, git2::Error> {
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
    debug!("Total statuses found: {}", statuses.len());

    // Analyze changes for each file
    let mut repository_changes: HashMap<String, Vec<(Status, FileChangeStats)>> = HashMap::new();

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
                    // TODO: make it safer
                    let stats = diff.stats().unwrap();
                    // let stats = diff.stats().unwrap_or_else(|_| {
                    //     debug!("No diff stats found for path: {}", path);
                    //     DiffStats { raw: "" }
                    // });
                    FileChangeStats {
                        lines_added: stats.insertions(),
                        lines_deleted: stats.deletions(),
                        lines_modified: stats.insertions() + stats.deletions(),
                        status,
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
                .push((status, file_stats));
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
