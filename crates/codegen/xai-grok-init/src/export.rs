//! Export module — writes `.grok/` files (AGENTS.md, config.toml) to disk,
//! manages `.gitignore` entries, and optionally initializes git.

use std::path::Path;

use anyhow::{Context, Result};
use tokio::fs;

/// Result of writing a file.
pub struct ExportResult {
    /// The absolute path of the written file.
    pub path: String,
}

/// Result of the full export operation.
#[derive(Debug)]
pub struct FullExportResult {
    /// Path to the `.grok/` directory.
    pub grok_dir: String,
    /// Whether `.grok/AGENTS.md` was written.
    pub agents_md_written: bool,
    /// Whether `.grok/config.toml` was written.
    pub config_toml_written: bool,
    /// Path to AGENTS.md if written.
    pub agents_md_path: Option<String>,
    /// Path to config.toml if written.
    pub config_toml_path: Option<String>,
}

// ─── Public API ────────────────────────────────────────────────────────────

/// Ensure the `.grok/` directory exists under `project_root`.
pub async fn ensure_grok_dir(project_root: &Path) -> Result<String> {
    let grok_dir = project_root.join(".grok");
    fs::create_dir_all(&grok_dir)
        .await
        .context("Failed to create .grok/ directory")?;
    Ok(grok_dir
        .canonicalize()
        .unwrap_or(grok_dir)
        .display()
        .to_string())
}

/// Write AGENTS.md into `.grok/`.
///
/// Creates the `.grok/` directory first if needed.
pub async fn write_grok_agents_md(project_root: &Path, content: &str) -> Result<ExportResult> {
    let grok_dir = project_root.join(".grok");
    fs::create_dir_all(&grok_dir)
        .await
        .context("Failed to create .grok/ directory")?;

    let path = grok_dir.join("AGENTS.md");
    fs::write(&path, content)
        .await
        .context("Failed to write .grok/AGENTS.md")?;

    Ok(ExportResult {
        path: path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .display()
            .to_string(),
    })
}

/// Write config.toml into `.grok/`.
pub async fn write_grok_config(project_root: &Path, content: &str) -> Result<ExportResult> {
    let grok_dir = project_root.join(".grok");
    fs::create_dir_all(&grok_dir)
        .await
        .context("Failed to create .grok/ directory")?;

    let path = grok_dir.join("config.toml");
    fs::write(&path, content)
        .await
        .context("Failed to write .grok/config.toml")?;

    Ok(ExportResult {
        path: path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .display()
            .to_string(),
    })
}

/// Write both AGENTS.md and config.toml into `.grok/`.
pub async fn write_grok_files(
    project_root: &Path,
    agents_md_content: &str,
    config_toml_content: &str,
) -> Result<FullExportResult> {
    let grok_dir = ensure_grok_dir(project_root).await?;

    let agents_result = write_grok_agents_md(project_root, agents_md_content).await?;
    let config_result = write_grok_config(project_root, config_toml_content).await?;

    Ok(FullExportResult {
        grok_dir,
        agents_md_written: true,
        config_toml_written: true,
        agents_md_path: Some(agents_result.path),
        config_toml_path: Some(config_result.path),
    })
}

/// Add one or more gitignore patterns to `.gitignore` if not already present.
///
/// Returns `true` if the file was modified.
pub async fn add_to_gitignore(project_root: &Path, patterns: &[&str]) -> Result<bool> {
    let gitignore_path = project_root.join(".gitignore");

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)
            .await
            .context("Failed to read .gitignore")?
    } else {
        String::new()
    };

    let mut modified = false;
    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();

    for pattern in patterns {
        if lines.iter().any(|line| line.trim() == *pattern) {
            continue;
        }
        lines.push(pattern.to_string());
        modified = true;
    }

    if modified {
        let updated = lines.join("\n") + "\n";
        fs::write(&gitignore_path, updated)
            .await
            .context("Failed to write .gitignore")?;
    }

    Ok(modified)
}
