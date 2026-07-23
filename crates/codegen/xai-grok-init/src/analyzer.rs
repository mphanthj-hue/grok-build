//! Project analyzer — detects languages, build systems, test frameworks, linters,
//! and infers code conventions from scan data.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::scanner::{Manifest, RuleSource, ScannedProject};

// ─── Types ─────────────────────────────────────────────────────────────────

/// Complete analysis result from a scanned project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    /// Detected programming languages, ordered by prevalence.
    pub languages: Vec<DetectedLanguage>,
    /// Primary build system.
    pub build_system: Option<BuildSystem>,
    /// Test framework(s) detected.
    pub test_frameworks: Vec<TestFramework>,
    /// Linter/formatter tools detected.
    pub linter_formatters: Vec<LinterFormatter>,
    /// High-level frameworks (web framework, UI library, etc.).
    pub frameworks: Vec<String>,
    /// Inferred code conventions.
    pub conventions: Vec<Convention>,
    /// Whether this is a monorepo (workspaces, crate members).
    pub monorepo: bool,
    /// Build/test/lint commands discovered from CI and manifests.
    pub commands: Commands,
    /// Top-level source directories.
    pub source_dirs: Vec<String>,
}

/// A programming language detected in the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectedLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Java,
    Kotlin,
    Ruby,
    CSharp,
    Cpp,
    C,
    Swift,
    Zig,
    Shell,
    Dockerfile,
    Other(String),
}

impl DetectedLanguage {
    pub fn as_str(&self) -> &str {
        match self {
            DetectedLanguage::Rust => "Rust",
            DetectedLanguage::TypeScript => "TypeScript",
            DetectedLanguage::JavaScript => "JavaScript",
            DetectedLanguage::Python => "Python",
            DetectedLanguage::Go => "Go",
            DetectedLanguage::Java => "Java",
            DetectedLanguage::Kotlin => "Kotlin",
            DetectedLanguage::Ruby => "Ruby",
            DetectedLanguage::CSharp => "C#",
            DetectedLanguage::Cpp => "C++",
            DetectedLanguage::C => "C",
            DetectedLanguage::Swift => "Swift",
            DetectedLanguage::Zig => "Zig",
            DetectedLanguage::Shell => "Shell",
            DetectedLanguage::Dockerfile => "Docker",
            DetectedLanguage::Other(s) => s.as_str(),
        }
    }
}

/// Detected build system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildSystem {
    Cargo,
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Pip,
    Poetry,
    Uv,
    GoBuild,
    Make,
    Maven,
    Gradle,
    Mix,
    Other(String),
}

impl BuildSystem {
    pub fn as_str(&self) -> &str {
        match self {
            BuildSystem::Cargo => "Cargo",
            BuildSystem::Npm => "npm",
            BuildSystem::Pnpm => "pnpm",
            BuildSystem::Yarn => "yarn",
            BuildSystem::Bun => "bun",
            BuildSystem::Pip => "pip",
            BuildSystem::Poetry => "Poetry",
            BuildSystem::Uv => "uv",
            BuildSystem::GoBuild => "go build",
            BuildSystem::Make => "make",
            BuildSystem::Maven => "Maven",
            BuildSystem::Gradle => "Gradle",
            BuildSystem::Mix => "mix",
            BuildSystem::Other(s) => s.as_str(),
        }
    }

    /// Shell-friendly command string for building.
    pub fn build_command(&self) -> &str {
        match self {
            BuildSystem::Cargo => "cargo build",
            BuildSystem::Npm => "npm run build",
            BuildSystem::Pnpm => "pnpm run build",
            BuildSystem::Yarn => "yarn build",
            BuildSystem::Bun => "bun run build",
            BuildSystem::Pip => "pip install -e .",
            BuildSystem::Poetry => "poetry install",
            BuildSystem::Uv => "uv sync",
            BuildSystem::GoBuild => "go build ./...",
            BuildSystem::Make => "make",
            BuildSystem::Maven => "mvn compile",
            BuildSystem::Gradle => "gradle build",
            BuildSystem::Mix => "mix compile",
            BuildSystem::Other(s) => s.as_str(),
        }
    }
}

/// Detected test framework.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestFramework {
    CargoTest,
    Jest,
    Vitest,
    Mocha,
    Playwright,
    Cypress,
    Pytest,
    Unittest,
    GoTest,
    Rspec,
    Minitest,
    Other(String),
}

impl TestFramework {
    pub fn as_str(&self) -> &str {
        match self {
            TestFramework::CargoTest => "cargo test",
            TestFramework::Jest => "jest",
            TestFramework::Vitest => "vitest",
            TestFramework::Mocha => "mocha",
            TestFramework::Playwright => "Playwright",
            TestFramework::Cypress => "Cypress",
            TestFramework::Pytest => "pytest",
            TestFramework::Unittest => "unittest",
            TestFramework::GoTest => "go test",
            TestFramework::Rspec => "RSpec",
            TestFramework::Minitest => "Minitest",
            TestFramework::Other(s) => s.as_str(),
        }
    }

    /// Shell-friendly command string for running tests.
    pub fn test_command(&self) -> &str {
        match self {
            TestFramework::CargoTest => "cargo test",
            TestFramework::Jest => "npm test",
            TestFramework::Vitest => "npx vitest run",
            TestFramework::Mocha => "npm test",
            TestFramework::Playwright => "npx playwright test",
            TestFramework::Cypress => "npx cypress run",
            TestFramework::Pytest => "pytest",
            TestFramework::Unittest => "python -m pytest",
            TestFramework::GoTest => "go test ./...",
            TestFramework::Rspec => "bundle exec rspec",
            TestFramework::Minitest => "bundle exec rake test",
            TestFramework::Other(s) => s.as_str(),
        }
    }
}

/// Detected linter or formatter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LinterFormatter {
    Clippy,
    Rustfmt,
    ESLint,
    Prettier,
    Biome,
    Ruff,
    Black,
    GoLint,
    GoFmt,
    RuboCop,
    Other(String),
}

impl LinterFormatter {
    pub fn as_str(&self) -> &str {
        match self {
            LinterFormatter::Clippy => "clippy",
            LinterFormatter::Rustfmt => "rustfmt",
            LinterFormatter::ESLint => "ESLint",
            LinterFormatter::Prettier => "Prettier",
            LinterFormatter::Biome => "Biome",
            LinterFormatter::Ruff => "ruff",
            LinterFormatter::Black => "black",
            LinterFormatter::GoLint => "golangci-lint",
            LinterFormatter::GoFmt => "gofmt",
            LinterFormatter::RuboCop => "RuboCop",
            LinterFormatter::Other(s) => s.as_str(),
        }
    }

    pub fn lint_command(&self) -> Option<&str> {
        match self {
            LinterFormatter::Clippy => Some("cargo clippy"),
            LinterFormatter::Rustfmt => None, // formatter
            LinterFormatter::ESLint => Some("npx eslint ."),
            LinterFormatter::Prettier => None, // formatter
            LinterFormatter::Biome => Some("npx biome check ."),
            LinterFormatter::Ruff => Some("ruff check ."),
            LinterFormatter::Black => None, // formatter
            LinterFormatter::GoLint => Some("golangci-lint run"),
            LinterFormatter::GoFmt => None, // formatter
            LinterFormatter::RuboCop => Some("bundle exec rubocop"),
            LinterFormatter::Other(_) => None,
        }
    }
}

/// Inferred code convention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Convention {
    pub description: String,
    pub certainty: Certainty,
}

/// How confident we are in a detected convention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Certainty {
    /// Directly from config files (eslint config, rustfmt, etc.)
    High,
    /// Inferred from file patterns or dependency names.
    Medium,
    /// Educated guess from language defaults.
    Low,
}

/// Build/test/lint commands discovered.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Commands {
    pub build: Vec<String>,
    pub test: Vec<String>,
    pub lint: Vec<String>,
    pub format: Vec<String>,
    pub other: Vec<String>,
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Extension to language mapping.
const EXT_TO_LANG: &[(&[&str], DetectedLanguage)] = &[
    (&["rs"], DetectedLanguage::Rust),
    (&["ts", "tsx"], DetectedLanguage::TypeScript),
    (&["js", "jsx", "mjs", "cjs"], DetectedLanguage::JavaScript),
    (&["py", "pyx"], DetectedLanguage::Python),
    (&["go"], DetectedLanguage::Go),
    (&["java"], DetectedLanguage::Java),
    (&["kt", "kts"], DetectedLanguage::Kotlin),
    (&["rb"], DetectedLanguage::Ruby),
    (&["cs"], DetectedLanguage::CSharp),
    (&["cpp", "cc", "cxx", "hpp", "hxx"], DetectedLanguage::Cpp),
    (&["c", "h"], DetectedLanguage::C),
    (&["swift"], DetectedLanguage::Swift),
    (&["zig"], DetectedLanguage::Zig),
    (&["sh", "bash", "zsh"], DetectedLanguage::Shell),
    (&["dockerfile"], DetectedLanguage::Dockerfile),
];

/// Framework detection from crate/package names.
const FRAMEWORK_INDICATORS: &[(&[&str], &str)] = &[
    (&["react", "react-dom"], "React"),
    (&["vue", "vue-router"], "Vue.js"),
    (&["svelte", "@sveltejs"], "Svelte"),
    (&["next", "next.js"], "Next.js"),
    (&["nuxt"], "Nuxt.js"),
    (&["express", "axum", "actix-web", "rocket", "warp"], "Web framework"),
    (&["django", "flask", "fastapi", "starlette"], "Python web framework"),
    (&["spring", "spring-boot"], "Spring"),
    (&["turbo", "nx"], "Monorepo tool"),
];

/// Cargo.toml dependencies that indicate linter/formatter config.
const _RUST_LINT_DEPS: &[&str] = &["clippy", "rustfmt"];
const _TS_LINT_DEPS: &[&str] = &["eslint", "prettier", "biome"];
const _PY_LINT_DEPS: &[&str] = &["ruff", "black", "pylint", "mypy"];

// ─── Analyzer ──────────────────────────────────────────────────────────────

/// Analyze a scanned project and produce structured findings.
pub async fn analyze(project: &ScannedProject) -> Result<Analysis, anyhow::Error> {
    let languages = detect_languages(project);
    let build_system = detect_build_system(project);
    let test_frameworks = detect_test_frameworks(project);
    let linter_formatters = detect_linter_formatters(project);
    let frameworks = detect_frameworks(project);
    let monorepo = detect_monorepo(project);
    let source_dirs = infer_source_dirs(project, &languages);
    let commands = extract_commands(project, &build_system, &test_frameworks, &linter_formatters);
    let conventions = infer_conventions(
        project,
        &build_system,
        &test_frameworks,
        &linter_formatters,
        &languages,
        monorepo,
    );

    Ok(Analysis {
        languages,
        build_system,
        test_frameworks,
        linter_formatters,
        frameworks,
        conventions,
        monorepo,
        commands,
        source_dirs,
    })
}

// ─── Language detection ────────────────────────────────────────────────────

fn detect_languages(project: &ScannedProject) -> Vec<DetectedLanguage> {
    // Detect from file extensions
    let mut lang_scores: HashMap<&str, usize> = HashMap::new();

    for (exts, lang) in EXT_TO_LANG {
        let count: usize = exts
            .iter()
            .map(|e| project.extension_counts.get(*e).copied().unwrap_or(0))
            .sum();
        if count > 0 {
            *lang_scores.entry(lang.as_str()).or_default() += count;
        }
    }

    // Also check from manifests
    for manifest in &project.manifests {
        match manifest {
            Manifest::CargoToml { .. } => {
                *lang_scores.entry("Rust").or_default() += 10;
            }
            Manifest::PackageJson { .. } => {
                // JS/TS is already counted by extensions, but boost it
            }
            Manifest::PyProjectToml { .. } => {
                *lang_scores.entry("Python").or_default() += 10;
            }
            Manifest::GoMod { .. } => {
                *lang_scores.entry("Go").or_default() += 10;
            }
            _ => {}
        }
    }

    // Check for Dockerfile
    if project.files.iter().any(|f| {
        f.file_name()
            .map_or(false, |n| n.eq_ignore_ascii_case("dockerfile"))
    }) {
        *lang_scores.entry("Docker").or_default() += 1;
    }

    // Sort by score descending
    let mut sorted: Vec<(String, usize)> = lang_scores
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    // Map back to DetectedLanguage
    sorted
        .into_iter()
        .filter_map(|(name, _)| {
            // Match by name
            for lang in EXT_TO_LANG.iter().map(|(_, l)| l) {
                if lang.as_str() == name {
                    return Some(lang.clone());
                }
            }
            match name.as_str() {
                "Docker" => Some(DetectedLanguage::Dockerfile),
                "Shell" => Some(DetectedLanguage::Shell),
                _ => Some(DetectedLanguage::Other(name)),
            }
        })
        .collect()
}

// ─── Build system detection ────────────────────────────────────────────────

fn detect_build_system(project: &ScannedProject) -> Option<BuildSystem> {
    for manifest in &project.manifests {
        match manifest {
            Manifest::CargoToml { .. } => return Some(BuildSystem::Cargo),
            Manifest::PackageJson { scripts: _, .. } => {
                // Check for specific package manager
                let has_lock = project.files.iter().any(|f| {
                    f.relative_path == "pnpm-lock.yaml"
                        || f.relative_path == "yarn.lock"
                        || f.relative_path == "bun.lockb"
                });
                if has_lock {
                    if project.files.iter().any(|f| f.relative_path == "pnpm-lock.yaml") {
                        return Some(BuildSystem::Pnpm);
                    }
                    if project.files.iter().any(|f| f.relative_path == "yarn.lock") {
                        return Some(BuildSystem::Yarn);
                    }
                    if project.files.iter().any(|f| f.relative_path == "bun.lockb") {
                        return Some(BuildSystem::Bun);
                    }
                }
                return Some(BuildSystem::Npm);
            }
            Manifest::PyProjectToml { .. } => {
                // Check for Poetry vs pip vs uv
                let content = project
                    .manifests
                    .iter()
                    .find(|m| matches!(m, Manifest::PyProjectToml { .. }));
                if let Some(Manifest::PyProjectToml { deps, .. }) = content {
                    if deps.iter().any(|d| d == "poetry") {
                        return Some(BuildSystem::Poetry);
                    }
                }
                // Check for uv.lock
                if project.files.iter().any(|f| f.relative_path == "uv.lock" || f.relative_path == ".uv.lock") {
                    return Some(BuildSystem::Uv);
                }
                return Some(BuildSystem::Pip);
            }
            Manifest::GoMod { .. } => return Some(BuildSystem::GoBuild),
            Manifest::Makefile { .. } => return Some(BuildSystem::Make),
            _ => {}
        }
    }

    None
}

// ─── Test framework detection ──────────────────────────────────────────────

fn detect_test_frameworks(project: &ScannedProject) -> Vec<TestFramework> {
    let mut frameworks = Vec::new();

    for manifest in &project.manifests {
        match manifest {
            Manifest::CargoToml { deps, .. } => {
                if deps.iter().any(|d| d == "cargo-test" || d == "tests" || d == "test") {
                    frameworks.push(TestFramework::CargoTest);
                }
                // Rust projects almost always use cargo test
                if frameworks.is_empty() {
                    frameworks.push(TestFramework::CargoTest);
                }
            }
            Manifest::PackageJson { scripts, deps, dev_deps, .. } => {
                // Check scripts
                for (_, cmd) in scripts {
                    let cmd_lower = cmd.to_lowercase();
                    if cmd_lower.contains("jest") && !frameworks.contains(&TestFramework::Jest) {
                        frameworks.push(TestFramework::Jest);
                    }
                    if cmd_lower.contains("vitest") && !frameworks.contains(&TestFramework::Vitest) {
                        frameworks.push(TestFramework::Vitest);
                    }
                    if cmd_lower.contains("mocha") && !frameworks.contains(&TestFramework::Mocha) {
                        frameworks.push(TestFramework::Mocha);
                    }
                    if cmd_lower.contains("playwright") && !frameworks.contains(&TestFramework::Playwright) {
                        frameworks.push(TestFramework::Playwright);
                    }
                    if cmd_lower.contains("cypress") && !frameworks.contains(&TestFramework::Cypress) {
                        frameworks.push(TestFramework::Cypress);
                    }
                }

                // Check dependencies
                let all_deps: Vec<&str> = deps.iter().chain(dev_deps.iter()).map(|s| s.as_str()).collect();
                let check_dep = |name: &str| all_deps.iter().any(|d| d.contains(name));

                if check_dep("jest") && !frameworks.contains(&TestFramework::Jest) {
                    frameworks.push(TestFramework::Jest);
                }
                if check_dep("vitest") && !frameworks.contains(&TestFramework::Vitest) {
                    frameworks.push(TestFramework::Vitest);
                }
                if check_dep("mocha") && !frameworks.contains(&TestFramework::Mocha) {
                    frameworks.push(TestFramework::Mocha);
                }
                if check_dep("playwright") && !frameworks.contains(&TestFramework::Playwright) {
                    frameworks.push(TestFramework::Playwright);
                }
                if check_dep("cypress") && !frameworks.contains(&TestFramework::Cypress) {
                    frameworks.push(TestFramework::Cypress);
                }
            }
            Manifest::PyProjectToml { deps, .. } => {
                if deps.iter().any(|d| d == "pytest") {
                    frameworks.push(TestFramework::Pytest);
                }
            }
            Manifest::GoMod { .. } => {
                frameworks.push(TestFramework::GoTest);
            }
            _ => {}
        }
    }

    // Look for test files as fallback
    if frameworks.is_empty() {
        if project.files.iter().any(|f| f.relative_path.contains("test_") || f.relative_path.ends_with("_test.go")) {
            // Could be pytest, Go test, etc.
        }
    }

    frameworks
}

// ─── Linter / formatter detection ─────────────────────────────────────────

fn detect_linter_formatters(project: &ScannedProject) -> Vec<LinterFormatter> {
    let mut tools = Vec::new();

    // Check for config files
    for file in &project.files {
        let name = file.file_name();
        let path = &file.relative_path;

        // Rust
        if name == Some("clippy.toml") || name == Some(".clippy.toml") || path.contains("rustfmt.toml") || path.contains("rustfmt") {
            if name == Some(".rustfmt.toml") || path.contains("rustfmt") {
                tools.push(LinterFormatter::Rustfmt);
            }
        }

        // Rust — Cargo.toml deps
        if path == "Cargo.toml" || path.ends_with("/Cargo.toml") {
            // Already handled via manifest deps below
        }

        // JS/TS
        if name == Some(".eslintrc") || name == Some(".eslintrc.json") || name == Some(".eslintrc.js") || name == Some(".eslintrc.yaml") || name == Some(".eslintrc.yml") || path.contains(".eslintrc") {
            tools.push(LinterFormatter::ESLint);
        }
        if name == Some(".prettierrc") || name == Some(".prettierrc.json") || name == Some(".prettierrc.js") || name == Some(".prettierrc.yaml") || name == Some(".prettierrc.toml") || path.contains(".prettierrc") {
            tools.push(LinterFormatter::Prettier);
        }
        if name == Some("biome.json") || path.contains("biome.json") {
            tools.push(LinterFormatter::Biome);
        }

        // Python
        if name == Some("ruff.toml") || name == Some(".ruff.toml") || path.contains("ruff") {
            tools.push(LinterFormatter::Ruff);
        }
        if name == Some("pyproject.toml") {
            // Checked via manifest deps
        }
        if name == Some(".pylintrc") || name == Some("pylintrc") {
            if !tools.contains(&LinterFormatter::Ruff) {
                tools.push(LinterFormatter::ESLint); // placeholder — no Python variant in enum
            }
        }
    }

    // Check manifest deps for lint tools
    for manifest in &project.manifests {
        match manifest {
            Manifest::CargoToml { deps, .. } => {
                if deps.iter().any(|d| d == "clippy") {
                    if !tools.contains(&LinterFormatter::Clippy) {
                        tools.push(LinterFormatter::Clippy);
                    }
                }
            }
            Manifest::PackageJson { deps, dev_deps, .. } => {
                let all_deps: Vec<&str> = deps.iter().chain(dev_deps.iter()).map(|s| s.as_str()).collect();
                if all_deps.iter().any(|d| *d == "eslint") && !tools.contains(&LinterFormatter::ESLint) {
                    tools.push(LinterFormatter::ESLint);
                }
                if all_deps.iter().any(|d| *d == "prettier") && !tools.contains(&LinterFormatter::Prettier) {
                    tools.push(LinterFormatter::Prettier);
                }
                if all_deps.iter().any(|d| *d == "@biomejs/biome" || *d == "biome") && !tools.contains(&LinterFormatter::Biome) {
                    tools.push(LinterFormatter::Biome);
                }
            }
            Manifest::PyProjectToml { deps, .. } => {
                if deps.iter().any(|d| d == "ruff") && !tools.contains(&LinterFormatter::Ruff) {
                    tools.push(LinterFormatter::Ruff);
                }
            }
            _ => {}
        }
    }

    // Deduplicate
    tools.sort();
    tools.dedup();
    tools
}

// ─── Framework detection ───────────────────────────────────────────────────

fn detect_frameworks(project: &ScannedProject) -> Vec<String> {
    let mut frameworks = Vec::new();
    let mut dep_set: HashSet<String> = HashSet::new();

    for manifest in &project.manifests {
        match manifest {
            Manifest::CargoToml { deps, .. } => {
                for d in deps {
                    dep_set.insert(d.to_lowercase());
                }
            }
            Manifest::PackageJson { deps, dev_deps, .. } => {
                let mut all = deps.clone();
                all.extend(dev_deps.clone());
                for d in all {
                    dep_set.insert(d.to_lowercase());
                }
            }
            Manifest::PyProjectToml { deps, .. } => {
                for d in deps {
                    dep_set.insert(d.to_lowercase());
                }
            }
            _ => {}
        }
    }

    for (indicators, framework_name) in FRAMEWORK_INDICATORS {
        if indicators.iter().any(|ind| dep_set.contains(&ind.to_lowercase())) {
            frameworks.push(framework_name.to_string());
        }
    }

    frameworks.sort();
    frameworks.dedup();
    frameworks
}

// ─── Monorepo detection ────────────────────────────────────────────────────

fn detect_monorepo(project: &ScannedProject) -> bool {
    for manifest in &project.manifests {
        match manifest {
            Manifest::CargoToml { members, .. } => {
                if !members.is_empty() {
                    return true;
                }
            }
            Manifest::PackageJson { workspaces, .. } => {
                if workspaces.is_some() {
                    return true;
                }
            }
            _ => {}
        }
    }

    // Check for multiple manifest files in subdirectories
    let manifest_count = project
        .files
        .iter()
        .filter(|f| {
            let name = f.file_name();
            name == Some("Cargo.toml") || name == Some("package.json")
        })
        .count();

    manifest_count > 2 // root + at least one child
}

// ─── Source directory inference ────────────────────────────────────────────

fn infer_source_dirs(project: &ScannedProject, _languages: &[DetectedLanguage]) -> Vec<String> {
    let mut dirs = Vec::new();

    let known_src_dirs = [
        "src", "lib", "app", "packages", "crates", "components",
        "api", "routes", "pages", "services", "cmd", "internal",
        "pkg", "backend", "frontend", "web", "server", "client",
    ];

    for dir in &project.top_level_dirs {
        if known_src_dirs.contains(&dir.as_str()) {
            dirs.push(dir.clone());
        }
    }

    dirs
}

// ─── Command extraction ────────────────────────────────────────────────────

fn extract_commands(
    project: &ScannedProject,
    build_system: &Option<BuildSystem>,
    test_frameworks: &[TestFramework],
    linter_formatters: &[LinterFormatter],
) -> Commands {
    let mut cmds = Commands::default();

    // Build commands
    if let Some(bs) = build_system {
        cmds.build.push(bs.build_command().to_string());

        // From package.json scripts
        for manifest in &project.manifests {
            if let Manifest::PackageJson { scripts, .. } = manifest {
                if let Some(build) = scripts.get("build") {
                    if !build.starts_with("tsc") {
                        // Only add non-standard
                        let full = format!("npm run build  # {}", build);
                        if !cmds.build.contains(&full) {
                            cmds.build.push(full);
                        }
                    }
                }
                if let Some(dev) = scripts.get("dev") {
                    let full = format!("npm run dev  # {}", dev);
                    cmds.other.push(full);
                }
            }
        }
    }

    // Test commands
    for tf in test_frameworks {
        cmds.test.push(tf.test_command().to_string());
    }

    // From CI configs
    for ci in &project.ci_configs {
        let _content_lower = ci.content.to_lowercase();
        for line in ci.content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("run:") || trimmed.starts_with("- run:") {
                let cmd = trimmed
                    .trim_start_matches("run:")
                    .trim_start_matches("- run:")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if !cmd.is_empty() && !cmds.other.contains(&cmd) {
                    if cmd.contains("test") {
                        cmds.test.push(cmd);
                    } else if cmd.contains("lint") || cmd.contains("clippy") || cmd.contains("eslint") {
                        cmds.lint.push(cmd);
                    } else if cmd.contains("build") {
                        cmds.build.push(cmd);
                    } else {
                        cmds.other.push(cmd);
                    }
                }
            }
        }
    }

    // Lint commands
    for lf in linter_formatters {
        if let Some(cmd) = lf.lint_command() {
            cmds.lint.push(cmd.to_string());
        }
    }

    // Deduplicate
    cmds.build.sort();
    cmds.build.dedup();
    cmds.test.sort();
    cmds.test.dedup();
    cmds.lint.sort();
    cmds.lint.dedup();

    cmds
}

// ─── Convention inference ─────────────────────────────────────────────────

fn infer_conventions(
    project: &ScannedProject,
    build_system: &Option<BuildSystem>,
    test_frameworks: &[TestFramework],
    linter_formatters: &[LinterFormatter],
    languages: &[DetectedLanguage],
    monorepo: bool,
) -> Vec<Convention> {
    let mut conventions = Vec::new();

    // Language-specific conventions
    for lang in languages {
        match lang {
            DetectedLanguage::Rust => {
                conventions.push(Convention {
                    description: "Use Rust edition 2021 (or project's configured edition)".into(),
                    certainty: Certainty::Medium,
                });
                conventions.push(Convention {
                    description: "Run `cargo fmt` before committing to ensure consistent formatting".into(),
                    certainty: Certainty::High,
                });
            }
            DetectedLanguage::TypeScript | DetectedLanguage::JavaScript => {
                conventions.push(Convention {
                    description: "Use strict TypeScript mode with proper type annotations".into(),
                    certainty: Certainty::Medium,
                });
            }
            DetectedLanguage::Python => {
                conventions.push(Convention {
                    description: "Follow PEP 8 style guide for Python code".into(),
                    certainty: Certainty::Medium,
                });
            }
            DetectedLanguage::Go => {
                conventions.push(Convention {
                    description: "Run `go fmt ./...` before committing".into(),
                    certainty: Certainty::High,
                });
            }
            _ => {}
        }
    }

    // Monorepo conventions
    if monorepo {
        conventions.push(Convention {
            description: "This is a monorepo with multiple packages — scope changes to the relevant package".into(),
            certainty: Certainty::High,
        });
    }

    // Linter/formatter conventions
    for lf in linter_formatters {
        match lf {
            LinterFormatter::Clippy => {
                conventions.push(Convention {
                    description: "Run `cargo clippy` to catch common mistakes and enforce Rust idioms".into(),
                    certainty: Certainty::High,
                });
            }
            LinterFormatter::Rustfmt => {
                conventions.push(Convention {
                    description: "Use `rustfmt` for consistent Rust code formatting".into(),
                    certainty: Certainty::High,
                });
            }
            LinterFormatter::ESLint => {
                conventions.push(Convention {
                    description: "Follow project ESLint configuration for code style".into(),
                    certainty: Certainty::High,
                });
            }
            LinterFormatter::Prettier => {
                conventions.push(Convention {
                    description: "Use Prettier for consistent code formatting".into(),
                    certainty: Certainty::High,
                });
            }
            LinterFormatter::Ruff => {
                conventions.push(Convention {
                    description: "Run `ruff check .` to lint Python code".into(),
                    certainty: Certainty::High,
                });
            }
            _ => {}
        }
    }

    // Test conventions
    if !test_frameworks.is_empty() {
        conventions.push(Convention {
            description: format!(
                "Run `{}` before pushing changes to ensure tests pass",
                test_frameworks[0].test_command()
            ),
            certainty: Certainty::High,
        });
    }

    // Build command conventions
    if let Some(bs) = build_system {
        conventions.push(Convention {
            description: format!("Build with `{}` before committing", bs.build_command()),
            certainty: Certainty::Medium,
        });
    }

    // Existing rule file conventions
    for rule in &project.existing_rules {
        let source = match rule.source {
            RuleSource::Cursor => "Cursor",
            RuleSource::Copilot => "GitHub Copilot",
            RuleSource::Windsurf => "Windsurf",
            RuleSource::Cline => "Cline",
            RuleSource::Claude => "Claude Code",
            RuleSource::Grok => "Grok",
            RuleSource::Other => "other tools",
        };
        conventions.push(Convention {
            description: format!(
                "Respect existing instructions from {} configuration at `{}`",
                source,
                rule.path.display()
            ),
            certainty: Certainty::High,
        });
    }

    // Git conventions
    if project.is_worktree {
        conventions.push(Convention {
            description: "This is a git worktree — be aware of which branch it corresponds to".into(),
            certainty: Certainty::Medium,
        });
    }

    // README conventions
    for readme in &project.readme_files {
        if readme.content.contains("convention") || readme.content.contains("CONTRIBUTING") {
            conventions.push(Convention {
                description: "Follow conventions documented in the project README".into(),
                certainty: Certainty::Medium,
            });
        }
    }

    conventions
}

// ─── File name helper ─────────────────────────────────────────────────────

/// Trait to add `file_name` helper to ScannedFile.
trait ScannedFileExt {
    fn file_name(&self) -> Option<&str>;
}

impl ScannedFileExt for crate::scanner::ScannedFile {
    fn file_name(&self) -> Option<&str> {
        self.path.file_name().and_then(|n| n.to_str())
    }
}

impl ScannedFileExt for crate::scanner::ExistingRule {
    fn file_name(&self) -> Option<&str> {
        self.path.file_name().and_then(|n| n.to_str())
    }
}
