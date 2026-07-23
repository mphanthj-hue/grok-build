//! # xai-grok-init
//!
//! Native Rust codebase scanner and `.grok/` project scaffolder for Grok Build's `/init` command.
//!
//! ## Flow
//!
//! 1. **`scanner`** — Walks the project tree (gitignore-aware), reads manifest files,
//!    README, CI configs, and existing instruction files.
//! 2. **`analyzer`** — Detects languages, build systems, test frameworks, linters,
//!    and infers conventions.
//! 3. **`renderer`** — Generates `.grok/AGENTS.md` markdown and `.grok/config.toml`.
//! 4. **`export`** — Writes `.grok/` files and manages `.gitignore`.
//! 5. **`git`** — Git repo initialization helpers.

pub mod analyzer;
pub mod export;
pub mod git;
pub mod renderer;
pub mod scanner;

use std::path::Path;

use anyhow::Result;
pub use analyzer::{Analysis, BuildSystem, DetectedLanguage, LinterFormatter, TestFramework};
pub use scanner::ScannedProject;

/// Options controlling what `/init` does.
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// If true, always update existing files rather than creating new ones.
    pub update_only: bool,
    /// If true, show the generated content without writing.
    pub dry_run: bool,
    /// If true, launch the interactive guide workflow instead of quick scan.
    pub guide: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            update_only: false,
            dry_run: false,
            guide: false,
        }
    }
}

/// Result returned to the user after a successful init.
#[derive(Debug)]
pub struct InitReport {
    /// Path to the `.grok/` directory (empty for dry-run).
    pub grok_dir: Option<String>,
    /// Path to `.grok/AGENTS.md` if generated.
    pub agents_md_path: Option<String>,
    /// Path to `.grok/config.toml` if generated.
    pub config_toml_path: Option<String>,
    /// Full AGENTS.md preview.
    pub preview: String,
    /// Number of conventions/rules discovered.
    pub rule_count: usize,
    /// Whether `.gitignore` was updated.
    pub gitignore_updated: bool,
    /// Whether git repo was initialized.
    pub git_inited: bool,
    /// Human-readable summary line.
    pub summary: String,
}

/// Main entry point: scan, analyze, generate, and write `.grok/` files.
///
/// Called from `slash_exec.rs` when the user runs `/init`.
pub async fn analyze_and_generate(cwd: &Path, opts: InitOptions) -> Result<InitReport> {
    // 1. SCAN
    let project = scanner::scan_project(cwd).await?;

    // 2. ANALYZE
    let analysis = analyzer::analyze(&project).await?;

    // 3. RENDER
    let agents_md = renderer::generate_grok_agents_md(&analysis, &project);
    let config_toml = renderer::generate_grok_config(&analysis, &project);

    // 4. CHECK EXISTING
    let grok_dir = project.root.join(".grok");
    let agents_md_path = grok_dir.join("AGENTS.md");
    let config_toml_path = grok_dir.join("config.toml");
    let grok_dir_exists = grok_dir.exists();
    let agents_exists = agents_md_path.exists();
    let config_exists = config_toml_path.exists();

    // If update_only and nothing exists, warn
    if opts.update_only && !agents_exists && !config_exists {
        return Ok(InitReport {
            grok_dir: None,
            agents_md_path: None,
            config_toml_path: None,
            preview: agents_md.clone(),
            rule_count: analysis.conventions.len(),
            gitignore_updated: false,
            git_inited: false,
            summary: "No existing `.grok/` found. Run without --update-only to create one.".into(),
        });
    }

    // 5. EXPORT (write if not dry_run)
    if opts.dry_run {
        return Ok(InitReport {
            grok_dir: None,
            agents_md_path: None,
            config_toml_path: None,
            preview: agents_md,
            rule_count: analysis.conventions.len(),
            gitignore_updated: false,
            git_inited: false,
            summary: format!(
                "Dry-run complete. Would create/update `.grok/` with {} conventions.",
                analysis.conventions.len(),
            ),
        });
    }

    // 5a. Git init if needed
    let git_inited = git::git_init_if_needed(&project.root).await?;

    // 5b. Write files
    let export_result = export::write_grok_files(&project.root, &agents_md, &config_toml).await?;

    // 5c. Add .grok/ to .gitignore
    let gitignore_updated = export::add_to_gitignore(
        &project.root,
        &[".grok/", "AGENTS.local.md"],
    )
    .await?;

    let summary = if !grok_dir_exists {
        format!(
            "Created `.grok/` with AGENTS.md ({} conventions) and config.toml. {}",
            analysis.conventions.len(),
            if gitignore_updated {
                "Added `.grok/` and AGENTS.local.md to .gitignore."
            } else {
                ""
            }
        )
    } else {
        format!(
            "Updated `.grok/`. Found {} conventions.",
            analysis.conventions.len(),
        )
    };

    Ok(InitReport {
        grok_dir: Some(export_result.grok_dir),
        agents_md_path: export_result.agents_md_path,
        config_toml_path: export_result.config_toml_path,
        preview: agents_md,
        rule_count: analysis.conventions.len(),
        gitignore_updated,
        git_inited,
        summary,
    })
}
