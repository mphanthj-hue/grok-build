//! Git initialization helpers for `/init`.
//!
//! Provides `git_init_if_needed()` to initialize a git repository if the project
//! root doesn't already have one, and `ensure_gitignore()` to create a minimal
//! `.gitignore` if one doesn't exist.

use std::path::Path;

use anyhow::{Context, Result};
use tokio::fs;

/// Initialize a git repository at `project_root` if one doesn't already exist.
///
/// Returns `true` if a new repo was initialized, `false` if one already existed.
pub async fn git_init_if_needed(project_root: &Path) -> Result<bool> {
    // Check if .git already exists (either as dir or file for worktrees)
    let git_path = project_root.join(".git");
    if git_path.exists() {
        return Ok(false);
    }

    // Use git2 to init the repo
    let result = tokio::task::spawn_blocking({
        let root = project_root.to_path_buf();
        move || -> Result<bool> {
            match git2::Repository::init(&root) {
                Ok(_) => Ok(true),
                Err(e) => {
                    // If git2 fails (e.g., no git config), fall back to CLI
                    tracing::warn!("git2 init failed ({}), falling back to CLI", e);
                    Err(anyhow::anyhow!("git init failed: {}", e))
                }
            }
        }
    })
    .await
    .context("Failed to spawn blocking task for git init")??;

    if result {
        tracing::info!("Initialized git repo at {}", project_root.display());
    }

    Ok(result)
}

/// Ensure a `.gitignore` exists at `project_root` with basic defaults.
///
/// Only writes if no `.gitignore` exists yet. Returns `true` if created.
pub async fn ensure_gitignore(project_root: &Path) -> Result<bool> {
    let gitignore_path = project_root.join(".gitignore");
    if gitignore_path.exists() {
        return Ok(false);
    }

    let default_gitignore = "# Dependencies\nnode_modules/\ntarget/\n\n"
        .to_string();

    fs::write(&gitignore_path, &default_gitignore)
        .await
        .context("Failed to write .gitignore")?;

    tracing::info!("Created .gitignore at {}", gitignore_path.display());
    Ok(true)
}
