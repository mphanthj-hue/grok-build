//! Codebase scanner — walks project tree, reads manifests, detects existing configs.
//!
//! Uses `ignore::Walk` for gitignore-aware file listing and `git2` for repo root
//! detection and git worktree checks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;

/// The complete snapshot of a project's filesystem surface.
#[derive(Debug, Clone)]
pub struct ScannedProject {
    /// Git repo root (or CWD if not in a git repo).
    pub root: PathBuf,
    /// The directory where the user ran `/init` (may differ from root for subdir in repo).
    pub cwd: PathBuf,
    /// Files discovered (non-binary, non-ignored).
    pub files: Vec<ScannedFile>,
    /// Extension -> count map for quick language inference.
    pub extension_counts: HashMap<String, usize>,
    /// Parsed manifest files.
    pub manifests: Vec<Manifest>,
    /// Content of README files found.
    pub readme_files: Vec<ReadmeFile>,
    /// Top-level directory entries (for structure understanding).
    pub top_level_dirs: Vec<String>,
    /// CI/CD configuration files found.
    pub ci_configs: Vec<CiConfig>,
    /// Content of existing AGENTS.md, if any.
    pub existing_agents_md: Option<String>,
    /// Existing rule files from vendor dirs.
    pub existing_rules: Vec<ExistingRule>,
    /// Whether this is a git worktree.
    pub is_worktree: bool,
}

/// A non-ignored file in the project.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub size: u64,
    pub extension: String,
}

/// A README file found in the project.
#[derive(Debug, Clone)]
pub struct ReadmeFile {
    pub path: PathBuf,
    pub content: String,
}

/// A parsed manifest / package configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Manifest {
    CargoToml {
        name: String,
        deps: Vec<String>,
        members: Vec<String>,
        edition: Option<String>,
    },
    PackageJson {
        name: String,
        scripts: HashMap<String, String>,
        deps: Vec<String>,
        dev_deps: Vec<String>,
        workspaces: Option<Vec<String>>,
    },
    PyProjectToml {
        name: Option<String>,
        deps: Vec<String>,
        requires_python: Option<String>,
    },
    GoMod {
        module: String,
        go_version: Option<String>,
    },
    Makefile {
        targets: Vec<String>,
    },
    DockerCompose {
        services: Vec<String>,
    },
    UnknownToml {
        path: String,
        keys: Vec<String>,
    },
}

/// A CI/CD configuration found.
#[derive(Debug, Clone)]
pub struct CiConfig {
    pub path: PathBuf,
    pub content: String,
}

/// An existing rule file from a vendor directory.
#[derive(Debug, Clone)]
pub struct ExistingRule {
    pub path: PathBuf,
    pub content: String,
    pub source: RuleSource,
}

/// Where a rule came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleSource {
    /// `.cursor/rules/*.mdc` or `.cursorrules`
    Cursor,
    /// `.github/copilot-instructions.md`
    Copilot,
    /// `.windsurfrules` or `.windsurf/rules/`
    Windsurf,
    /// `.clinerules`
    Cline,
    /// `.claude/rules/`
    Claude,
    /// `.grok/rules/`
    Grok,
    /// Other
    Other,
}

// ─── Known manifest filenames ──────────────────────────────────────────────

const MANIFEST_FILES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "Makefile",
    "docker-compose.yml",
    "docker-compose.yaml",
];

const README_FILES: &[&str] = &["README.md", "README.rst", "README.txt", "README"];

const _CI_GLOB_DIRS: &[&str] = &[
    ".github/workflows/",
    ".gitlab-ci.yml",
    ".circleci/config.yml",
];

const RULE_FILES: &[(&str, RuleSource)] = &[
    (".cursorrules", RuleSource::Cursor),
    (".github/copilot-instructions.md", RuleSource::Copilot),
    (".windsurfrules", RuleSource::Windsurf),
    (".clinerules", RuleSource::Cline),
];

const RULE_DIRS: &[(&str, RuleSource)] = &[
    (".cursor/rules", RuleSource::Cursor),
    (".windsurf/rules", RuleSource::Windsurf),
    (".claude/rules", RuleSource::Claude),
    (".grok/rules", RuleSource::Grok),
];

/// Known binary file extensions to skip during analysis.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "mp3", "mp4", "avi", "mov", "woff", "woff2",
    "ttf", "eot", "pdf", "doc", "docx", "xls", "xlsx", "zip", "tar", "gz", "bz2", "7z", "rar", "o",
    "so", "dylib", "dll", "exe", "wasm", "pyc", "pyo", "class", "jar",
    "lock", // package-lock.json, Cargo.lock etc. — not useful for init
];

// ─── Public API ────────────────────────────────────────────────────────────

/// Scan a project from the given working directory.
///
/// Discovers the git root, walks files, reads manifests, READMEs, CI configs,
/// and existing rule files.
pub async fn scan_project(cwd: &Path) -> Result<ScannedProject> {
    // 1. Find repo root
    let root = discover_repo_root(cwd).unwrap_or_else(|| cwd.to_path_buf());

    // 2. Walk files (gitignore-aware, non-binary)
    let (files, extension_counts) = walk_files(&root).await?;

    // 3. Read manifests
    let manifests = read_manifests(&root).await;

    // 4. Read READMEs
    let readme_files = read_readmes(&root).await;

    // 5. Top-level dirs
    let top_level_dirs = list_top_level_dirs(&root).await;

    // 6. CI configs
    let ci_configs = read_ci_configs(&root).await;

    // 7. Existing AGENTS.md
    let existing_agents_md = read_file_if_exists(&root.join("AGENTS.md")).await;

    // 8. Existing rule files
    let existing_rules = read_existing_rules(&root).await;

    // 9. Git worktree check
    let is_worktree = check_worktree(&root);

    Ok(ScannedProject {
        root,
        cwd: cwd.to_path_buf(),
        files,
        extension_counts,
        manifests,
        readme_files,
        top_level_dirs,
        ci_configs,
        existing_agents_md,
        existing_rules,
        is_worktree,
    })
}

// ─── Git repo root ─────────────────────────────────────────────────────────

fn discover_repo_root(path: &Path) -> Option<PathBuf> {
    let repo = git2::Repository::discover(path).ok()?;
    repo.workdir().map(|p| p.to_path_buf())
}

fn check_worktree(root: &Path) -> bool {
    if let Ok(repo) = git2::Repository::discover(root) {
        repo.is_worktree()
    } else {
        false
    }
}

// ─── File walking ──────────────────────────────────────────────────────────

async fn walk_files(root: &Path) -> Result<(Vec<ScannedFile>, HashMap<String, usize>)> {
    let mut files = Vec::new();
    let mut extension_counts: HashMap<String, usize> = HashMap::new();

    // Use `ignore::Walk` which respects .gitignore — same pattern as xai-codebase-graph.
    let walker = ignore::WalkBuilder::new(root)
        .standard_filters(true) // respects .gitignore
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path().to_path_buf();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Skip hidden dotfiles (config files are found via explicit paths)
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.') && n != ".env.example")
        {
            continue;
        }

        // Check extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if BINARY_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let size = fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0);

        // Only include smallish files (skip huge files)
        if size > 500_000 {
            continue;
        }

        *extension_counts.entry(ext.clone()).or_default() += 1;

        files.push(ScannedFile {
            path,
            relative_path,
            size,
            extension: ext,
        });
    }

    Ok((files, extension_counts))
}

// ─── Manifest reading ──────────────────────────────────────────────────────

async fn read_manifests(root: &Path) -> Vec<Manifest> {
    let mut manifests = Vec::new();

    for filename in MANIFEST_FILES {
        let path = root.join(filename);
        if !path.exists() {
            continue;
        }

        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        let manifest = match *filename {
            "Cargo.toml" => parse_cargo_toml(&content, &path),
            "package.json" => parse_package_json(&content),
            "pyproject.toml" => parse_pyproject_toml(&content),
            "go.mod" => parse_go_mod(&content),
            "Makefile" => parse_makefile(&content),
            "docker-compose.yml" | "docker-compose.yaml" => parse_docker_compose(&content),
            _ => continue,
        };

        if let Some(m) = manifest {
            manifests.push(m);
        }
    }

    manifests
}

fn parse_cargo_toml(content: &str, _path: &Path) -> Option<Manifest> {
    // Use toml::from_str for proper TOML document parsing (toml 0.9+)
    let val: toml::Value = toml::from_str(content).ok()?;

    // Simple Cargo.toml parsing — look for package name and dependencies
    let name = val.get("package")?.get("name")?.as_str()?.to_string();

    let edition = val
        .get("package")
        .and_then(|p: &toml::Value| p.get("edition"))
        .and_then(|e: &toml::Value| e.as_str())
        .map(|s: &str| s.to_string());

    let deps = extract_toml_deps(&val, "dependencies");
    let build_deps = extract_toml_deps(&val, "build-dependencies");
    let dev_deps = extract_toml_deps(&val, "dev-dependencies");

    let all_deps: Vec<String> = deps.into_iter().chain(build_deps).chain(dev_deps).collect();

    let members = val
        .get("workspace")
        .and_then(|w: &toml::Value| w.get("members"))
        .and_then(|m: &toml::Value| m.as_array())
        .map(|arr: &Vec<toml::Value>| {
            arr.iter()
                .filter_map(|v: &toml::Value| v.as_str().map(|s: &str| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Some(Manifest::CargoToml {
        name,
        deps: all_deps,
        members,
        edition,
    })
}

fn extract_toml_deps(toml: &toml::Value, key: &str) -> Vec<String> {
    toml.get(key)
        .and_then(|d: &toml::Value| d.as_table())
        .map(|table: &toml::Table| {
            table
                .keys()
                .filter(|k| *k != "target") // skip target.'cfg(...)' tables
                .map(|k: &String| k.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_package_json(content: &str) -> Option<Manifest> {
    let json: serde_json::Value = content.parse().ok()?;
    let name = json.get("name")?.as_str()?.to_string();

    let scripts = json
        .get("scripts")
        .and_then(|s| s.as_object())
        .map(|s| {
            s.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();

    let deps = extract_json_deps(&json, "dependencies");
    let dev_deps = extract_json_deps(&json, "devDependencies");

    let workspaces = json
        .get("workspaces")
        .and_then(|w| {
            if let Some(arr) = w.as_array() {
                Some(
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                )
            } else if let Some(obj) = w.as_object() {
                obj.get("packages").and_then(|p| p.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
            } else {
                None
            }
        })
        .filter(|v: &Vec<String>| !v.is_empty());

    Some(Manifest::PackageJson {
        name,
        scripts,
        deps,
        dev_deps,
        workspaces,
    })
}

fn extract_json_deps(json: &serde_json::Value, key: &str) -> Vec<String> {
    json.get(key)
        .and_then(|d| d.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

fn parse_pyproject_toml(content: &str) -> Option<Manifest> {
    let val: toml::Value = toml::from_str(content).ok()?;

    // Try [project] (PEP 621) first, then [tool.poetry] (Poetry)
    let name = val
        .get("project")
        .and_then(|p: &toml::Value| p.get("name"))
        .or_else(|| {
            val.get("tool")
                .and_then(|t: &toml::Value| t.get("poetry"))
                .and_then(|p: &toml::Value| p.get("name"))
        })
        .and_then(|v: &toml::Value| v.as_str())
        .map(|s: &str| s.to_string());

    let requires_python = val
        .get("project")
        .and_then(|p: &toml::Value| p.get("requires-python"))
        .and_then(|v: &toml::Value| v.as_str())
        .map(|s: &str| s.to_string());

    let mut deps = Vec::new();

    // PEP 621 dependencies
    if let Some(arr) = val
        .get("project")
        .and_then(|p: &toml::Value| p.get("dependencies"))
        .and_then(|d: &toml::Value| d.as_array())
    {
        for dep in arr {
            if let Some(s) = dep.as_str() {
                // Extract just the package name, strip version constraints
                let pkg = s
                    .trim()
                    .split(|c: char| {
                        c.is_whitespace()
                            || c == '>'
                            || c == '<'
                            || c == '='
                            || c == '~'
                            || c == '^'
                            || c == '!'
                    })
                    .next()
                    .unwrap_or(s.trim())
                    .trim()
                    .to_string();
                if !pkg.is_empty() {
                    deps.push(pkg);
                }
            }
        }
    }

    // Poetry dependencies
    if let Some(table) = val
        .get("tool")
        .and_then(|t: &toml::Value| t.get("poetry"))
        .and_then(|p: &toml::Value| p.get("dependencies"))
        .and_then(|d: &toml::Value| d.as_table())
    {
        for key in table.keys() {
            if key != "python" {
                deps.push(key.clone());
            }
        }
    }

    Some(Manifest::PyProjectToml {
        name,
        deps,
        requires_python,
    })
}

fn parse_go_mod(content: &str) -> Option<Manifest> {
    let module = content
        .lines()
        .find(|l| l.starts_with("module "))
        .map(|l| l.trim_start_matches("module ").trim().to_string())?;

    let go_version = content
        .lines()
        .find(|l| l.starts_with("go "))
        .map(|l| l.trim_start_matches("go ").trim().to_string());

    Some(Manifest::GoMod { module, go_version })
}

fn parse_makefile(content: &str) -> Option<Manifest> {
    let targets: Vec<String> = content
        .lines()
        .filter_map(|l| {
            let trimmed = l.trim();
            // Match lines like "target:" or "target: deps"
            if let Some(name) = trimmed.split(':').next() {
                if !name.is_empty()
                    && !name.starts_with('.')
                    && !name.starts_with('#')
                    && !name.contains('=')
                    && !name.contains('$')
                    && name
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    return Some(name.to_string());
                }
            }
            None
        })
        .collect();

    if targets.is_empty() {
        return None;
    }

    Some(Manifest::Makefile { targets })
}

fn parse_docker_compose(content: &str) -> Option<Manifest> {
    // Quick check: parse services key
    let services_key = if content.contains("services:") {
        "services"
    } else {
        return None;
    };

    // Very rough parse: find top-level service names
    let services: Vec<String> = content
        .lines()
        .skip_while(|l| !l.trim().starts_with(services_key))
        .skip(1)
        .take_while(|l| l.starts_with("  ") && !l.trim().is_empty())
        .filter_map(|l| {
            let trimmed = l.trim();
            if trimmed.ends_with(':') {
                Some(trimmed.trim_end_matches(':').to_string())
            } else {
                None
            }
        })
        .collect();

    Some(Manifest::DockerCompose { services })
}

// ─── README reading ────────────────────────────────────────────────────────

async fn read_readmes(root: &Path) -> Vec<ReadmeFile> {
    let mut readmes = Vec::new();
    for name in README_FILES {
        let path = root.join(name);
        if let Ok(content) = fs::read_to_string(&path).await {
            readmes.push(ReadmeFile { path, content });
        }
    }
    readmes
}

// ─── Top-level directory listing ───────────────────────────────────────────

async fn list_top_level_dirs(root: &Path) -> Vec<String> {
    let mut dirs = match fs::read_dir(root).await {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut names = Vec::new();
    while let Ok(Some(entry)) = dirs.next_entry().await {
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                // Skip hidden dirs and common non-source dirs
                if !name.starts_with('.')
                    && name != "target"
                    && name != "node_modules"
                    && name != "__pycache__"
                {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    names
}

// ─── CI config reading ─────────────────────────────────────────────────────

async fn read_ci_configs(root: &Path) -> Vec<CiConfig> {
    let mut configs = Vec::new();

    // Check .github/workflows/
    let workflows_dir = root.join(".github/workflows");
    if workflows_dir.is_dir() {
        if let Ok(mut entries) = fs::read_dir(&workflows_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path
                    .extension()
                    .map_or(false, |e| e == "yml" || e == "yaml")
                {
                    if let Ok(content) = fs::read_to_string(&path).await {
                        configs.push(CiConfig { path, content });
                    }
                }
            }
        }
    }

    // Check other CI config paths
    for ci_path in &[".gitlab-ci.yml", ".circleci/config.yml"] {
        let full_path = root.join(ci_path);
        if let Ok(content) = fs::read_to_string(&full_path).await {
            configs.push(CiConfig {
                path: full_path,
                content,
            });
        }
    }

    configs
}

// ─── Existing rule files ──────────────────────────────────────────────────

async fn read_existing_rules(root: &Path) -> Vec<ExistingRule> {
    let mut rules = Vec::new();

    // Individual rule files
    for (filename, source) in RULE_FILES {
        let path = root.join(filename);
        if let Ok(content) = fs::read_to_string(&path).await {
            rules.push(ExistingRule {
                path,
                content,
                source: source.clone(),
            });
        }
    }

    // Rule directories
    for (subdir, source) in RULE_DIRS {
        let dir = root.join(subdir);
        if !dir.is_dir() {
            continue;
        }
        if let Ok(mut entries) = fs::read_dir(&dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "md" || e == "mdc") {
                    if let Ok(content) = fs::read_to_string(&path).await {
                        rules.push(ExistingRule {
                            path,
                            content,
                            source: source.clone(),
                        });
                    }
                }
            }
        }
    }

    rules
}

// ─── Utilities ─────────────────────────────────────────────────────────────

async fn read_file_if_exists(path: &Path) -> Option<String> {
    if path.exists() {
        fs::read_to_string(path).await.ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_simple() {
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1"
tokio = { version = "1", features = ["full"] }

[workspace]
members = ["crates/*"]
"#;
        // We can only test parse logic since no file I/O
        let manifest = parse_cargo_toml(content, Path::new("Cargo.toml"));
        assert!(manifest.is_some());
        if let Some(Manifest::CargoToml {
            name,
            deps,
            members,
            edition,
        }) = manifest
        {
            assert_eq!(name, "my-app");
            assert!(deps.contains(&"serde".to_string()));
            assert!(deps.contains(&"tokio".to_string()));
            assert!(members.contains(&"crates/*".to_string()));
            assert_eq!(edition, Some("2021".to_string()));
        } else {
            panic!("Expected CargoToml");
        }
    }

    #[test]
    fn test_parse_package_json_simple() {
        let content = r#"{
            "name": "my-app",
            "scripts": {
                "build": "tsc",
                "test": "vitest"
            },
            "dependencies": {
                "react": "^18"
            },
            "devDependencies": {
                "typescript": "^5"
            }
        }"#;
        let manifest = parse_package_json(content);
        assert!(manifest.is_some());
        if let Some(Manifest::PackageJson {
            name,
            scripts,
            deps,
            dev_deps,
            workspaces,
        }) = manifest
        {
            assert_eq!(name, "my-app");
            assert_eq!(scripts.get("build").map(|s| s.as_str()), Some("tsc"));
            assert_eq!(scripts.get("test").map(|s| s.as_str()), Some("vitest"));
            assert!(deps.contains(&"react".to_string()));
            assert!(dev_deps.contains(&"typescript".to_string()));
            assert!(workspaces.is_none());
        } else {
            panic!("Expected PackageJson");
        }
    }
}
