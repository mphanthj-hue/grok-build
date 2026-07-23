//! # xai-grok-init
//!
//! Native Rust codebase scanner and AGENTS.md generator for Grok Build's `/init` command.
//!
//! ## Flow
//!
//! 1. **`scanner`** — Walks the project tree (gitignore-aware), reads manifest files,
//!    README, CI configs, and existing instruction files.
//! 2. **`analyzer`** — Detects languages, build systems, test frameworks, linters,
//!    and infers conventions.
//! 3. **`renderer`** — Generates AGENTS.md markdown from the analysis data.
//! 4. **`export`** — Writes the file (or returns a preview) and manages `.gitignore`.

pub mod analyzer;
pub mod export;
pub mod renderer;
pub mod scanner;

use std::path::Path;

use anyhow::Result;
pub use scanner::ScannedProject;
pub use analyzer::{Analysis, BuildSystem, DetectedLanguage, LinterFormatter, TestFramework};

/// Options controlling what `/init` does.
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// If true, always update existing AGENTS.md rather than creating a new one.
    pub update_only: bool,
    /// If true, show the generated content without writing.
    pub dry_run: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            update_only: false,
            dry_run: false,
        }
    }
}

/// Result returned to the user after a successful init.
#[derive(Debug)]
pub struct InitReport {
    /// Path to the written file (empty for dry-run).
    pub written_to: Option<String>,
    /// Full markdown preview.
    pub preview: String,
    /// Whether the file was newly created (vs updated).
    pub created: bool,
    /// Number of conventions/rules discovered.
    pub rule_count: usize,
    /// Whether AGENTS.local.md was added to .gitignore.
    pub gitignore_updated: bool,
    /// Human-readable summary line.
    pub summary: String,
}

/// Main entry point: scan, analyze, generate, and write AGENTS.md.
///
/// Called from `slash_exec.rs` when the user runs `/init`.
///
/// Returns a user-facing report string that the shell sends back as
/// slash-command output.
pub async fn analyze_and_generate(cwd: &Path, opts: InitOptions) -> Result<InitReport> {
    // 1. SCAN
    let project = scanner::scan_project(cwd).await?;

    // 2. ANALYZE
    let analysis = analyzer::analyze(&project).await?;

    // 3. RENDER
    let preview = renderer::generate_agents_md(&analysis, &project);

    // 4. CHECK EXISTING
    let existing_path = project.root.join("AGENTS.md");
    let exists = existing_path.exists();

    // If update_only and no existing file, warn
    if opts.update_only && !exists {
        return Ok(InitReport {
            written_to: None,
            preview: preview.clone(),
            created: false,
            rule_count: analysis.conventions.len(),
            gitignore_updated: false,
            summary: "No existing AGENTS.md found. Run without --update to create one.".into(),
        });
    }

    // 5. EXPORT (write if not dry_run)
    if opts.dry_run {
        return Ok(InitReport {
            written_to: None,
            preview,
            created: !exists,
            rule_count: analysis.conventions.len(),
            gitignore_updated: false,
            summary: format!(
                "Dry-run complete. Would {} AGENTS.md with {} conventions.",
                if exists { "update" } else { "create" },
                analysis.conventions.len(),
            ),
        });
    }

    let export_result = export::write_agents_md(&existing_path, &preview).await?;

    // Optionally add AGENTS.local.md to .gitignore if it doesn't exist
    let local_path = project.root.join("AGENTS.local.md");
    let gitignore_updated = if !local_path.exists() {
        export::add_to_gitignore(&project.root, "AGENTS.local.md").await?
    } else {
        false
    };

    let summary = if !exists {
        format!(
            "Created AGENTS.md with {} conventions. {}",
            analysis.conventions.len(),
            if gitignore_updated {
                "Added AGENTS.local.md to .gitignore."
            } else {
                ""
            }
        )
    } else {
        format!(
            "Updated AGENTS.md. Found {} conventions.",
            analysis.conventions.len(),
        )
    };

    Ok(InitReport {
        written_to: Some(export_result.path),
        preview,
        created: !exists,
        rule_count: analysis.conventions.len(),
        gitignore_updated,
        summary,
    })
}
