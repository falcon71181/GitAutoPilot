use git2::{Error as GitError, Repository};
use std::process::Command;

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

/// Lists newly added files (files that are unstaged for commit).
///
/// # Arguments
///
/// * `repo` - A reference to the `git2::Repository` object.
///
/// # Returns
///
/// * `Result<Vec<String>, git2::Error>` - A list of paths to the newly added files, or an error.
pub fn list_newly_added_files(repo: &Repository) -> Result<Vec<String>, git2::Error> {
    // Get the repository's status
    let statuses = repo.statuses(None)?;

    let newly_added_files: Vec<String> = statuses
        .iter()
        .filter(|entry| entry.status().is_wt_new())
        .map(|entry| entry.path().unwrap_or_default().to_string())
        .collect();

    Ok(newly_added_files)
}

/// Lists modified files (files that are unstaged for commit).
///
/// # Arguments
///
/// * `repo` - A reference to the `git2::Repository` object.
///
/// # Returns
///
/// * `Result<Vec<String>, git2::Error>` - A list of paths to the modified files, or an error.
pub fn list_modified_files(repo: &Repository) -> Result<Vec<String>, git2::Error> {
    // Get the repository's status
    let statuses = repo.statuses(None)?;

    let modified_files: Vec<String> = statuses
        .iter()
        .filter(|entry| entry.status().is_wt_modified())
        .map(|entry| entry.path().unwrap_or_default().to_string())
        .collect();

    Ok(modified_files)
}

/// Lists deleted files (files that are unstaged for commit).
///
/// # Arguments
///
/// * `repo` - A reference to the `git2::Repository` object.
///
/// # Returns
///
/// * `Result<Vec<String>, git2::Error>` - A list of paths to the deleted files, or an error.
pub fn list_deleted_files(repo: &Repository) -> Result<Vec<String>, git2::Error> {
    // Get the repository's status
    let statuses = repo.statuses(None)?;

    let deleted_files: Vec<String> = statuses
        .iter()
        .filter(|entry| entry.status().is_wt_deleted())
        .map(|entry| entry.path().unwrap_or_default().to_string())
        .collect();

    Ok(deleted_files)
}
