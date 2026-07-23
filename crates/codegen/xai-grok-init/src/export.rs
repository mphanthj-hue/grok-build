//! Export module — writes AGENTS.md to disk, manages .gitignore entries.

use std::path::Path;

use anyhow::{Context, Result};
use tokio::fs;

/// Result of writing AGENTS.md.
pub struct ExportResult {
    /// The absolute path of the written file.
    pub path: String,
}

/// Write the generated markdown content to the given file path.
///
/// Creates parent directories if needed and overwrites any existing content.
pub async fn write_agents_md(path: &Path, content: &str) -> Result<ExportResult> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .context("Failed to create parent directories for AGENTS.md")?;
    }

    fs::write(path, content)
        .await
        .context("Failed to write AGENTS.md")?;

    Ok(ExportResult {
        path: path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .display()
            .to_string(),
    })
}

/// Add a gitignore pattern to `.gitignore` if it's not already present.
///
/// Returns `true` if the file was modified, `false` if the pattern already existed.
pub async fn add_to_gitignore(project_root: &Path, pattern: &str) -> Result<bool> {
    let gitignore_path = project_root.join(".gitignore");

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)
            .await
            .context("Failed to read .gitignore")?
    } else {
        String::new()
    };

    // Check if pattern is already present
    if existing.lines().any(|line| line.trim() == pattern) {
        return Ok(false);
    }

    // Append the pattern
    let new_entry = if existing.ends_with('\n') {
        format!("{}\n", pattern)
    } else if existing.is_empty() {
        format!("{}\n", pattern)
    } else {
        format!("\n{}\n", pattern)
    };

    let updated = existing + &new_entry;
    fs::write(&gitignore_path, updated)
        .await
        .context("Failed to write .gitignore")?;

    Ok(true)
}
