let meta = #{
    name: "init-guide",
    description: "Interactive project initialization — interview, scan, and generate `.grok/` configuration",
    phases: [
        #{ title: "Interview", detail: "Learn about the project purpose and preferences" },
        #{ title: "Scan", detail: "Explore the project structure and detect technologies" },
        #{ title: "Generate", detail: "Create `.grok/AGENTS.md`, `config.toml`, and `sandbox.toml`" },
        #{ title: "Setup", detail: "Initialize git, update `.gitignore`, and summarize" },
    ],
};

// ─── Helper ────────────────────────────────────────────────────────────────
fn trimmed(s) {
    if type_of(s) == "string" {
        s.trim();
        s
    } else {
        ""
    }
}

fn join(arr, sep) {
    let result = "";
    let i = 0;
    for item in arr {
        if i > 0 {
            result += sep;
        }
        result += item;
        i += 1;
    }
    result
}

// ─── Phase 1: Interview ──────────────────────────────────────────────────
phase("Interview");

let interview_prompt = `You are starting an interactive project initialization session.
Your task is to interview the user (using the ask_user_question tool) about their project,
then generate a comprehensive project profile.

Follow this interview flow — ask ONE question at a time using ask_user_question:

1. What is the project name and a short description of what it does?
2. What type of project is this? (app, library, CLI tool, web service, game, etc.)
3. What is the primary tech stack? (programming languages, frameworks, databases)
4. Do you need a git repository initialized? (if not already in one)
5. Are there any specific coding conventions or style preferences?
6. Do you need any project-specific MCP servers or tools configured?
7. Should we set up a sandbox profile to restrict filesystem or network access?

After asking all questions, output a JSON object with the collected information:
{
    "project_name": "...",
    "description": "...",
    "project_type": "...",
    "tech_stack": ["..."],
    "init_git": true/false,
    "conventions": ["..."],
    "mcp_servers": ["..."],
    "sandbox_profile": "none" | "restrictive" | "isolated"
}

Return ONLY the JSON object, no other text.`;

let interview = agent(
    interview_prompt,
    #{
        label: "init-interviewer",
        capability_mode: "read-write",
        phase: "Interview",
    },
);

let project_name = "my-project";
let description = "";
let project_type = "";
let tech_stack = [];
let init_git = false;
let conventions = [];
let mcp_servers = [];
let sandbox_profile = "none";

if interview.success && interview.output != () {
    let out = interview.output;
    if out.project_name != () { project_name = trimmed(out.project_name); }
    if out.description != () { description = trimmed(out.description); }
    if out.project_type != () { project_type = trimmed(out.project_type); }
    if out.tech_stack != () { tech_stack = out.tech_stack; }
    if out.init_git != () { init_git = out.init_git; }
    if out.conventions != () { conventions = out.conventions; }
    if out.mcp_servers != () { mcp_servers = out.mcp_servers; }
    if out.sandbox_profile != () { sandbox_profile = trimmed(out.sandbox_profile); }
} else {
    log("Interview agent did not return structured output; proceeding with defaults.");
}

log("Interview complete. Project: " + project_name);

// ─── Phase 2: Scan ────────────────────────────────────────────────────────
phase("Scan");

let scan_prompt = `You are scanning a project directory to understand its structure.
Use run_terminal_command (ls, find, cat, etc.) to explore the project and collect:

1. Top-level directory listing
2. Root manifest files (Cargo.toml, package.json, pyproject.toml, go.mod, etc.)
3. README content if any
4. Existing configuration files (.gitignore, CI configs, etc.)
5. Source directories and their purpose
6. Any existing .grok/, .claude/, .cursor/, or similar AI config directories
7. Whether a .git directory already exists

After scanning, output a JSON object:
{
    "has_git": true/false,
    "has_grok_dir": true/false,
    "manifest_type": "cargo" | "npm" | "python" | "go" | "none",
    "manifest_name": "...",
    "top_level_dirs": ["..."],
    "existing_configs": ["..."],
    "readme_content_snippet": "...",
    "languages_detected": ["..."]
}

Return ONLY the JSON object, no other text.`;

let scan = agent(
    scan_prompt,
    #{
        label: "init-scanner",
        capability_mode: "execute",
        phase: "Scan",
    },
);

let has_git = false;
let has_grok_dir = false;
let manifest_type = "none";
let manifest_name = "";
let top_level_dirs = [];
let existing_configs = [];
let languages_detected = [];

if scan.success && scan.output != () {
    let out = scan.output;
    if out.has_git != () { has_git = out.has_git; }
    if out.has_grok_dir != () { has_grok_dir = out.has_grok_dir; }
    if out.manifest_type != () { manifest_type = trimmed(out.manifest_type); }
    if out.manifest_name != () { manifest_name = trimmed(out.manifest_name); }
    if out.top_level_dirs != () { top_level_dirs = out.top_level_dirs; }
    if out.existing_configs != () { existing_configs = out.existing_configs; }
    if out.languages_detected != () { languages_detected = out.languages_detected; }
} else {
    log("Scan agent did not return structured output; proceeding with defaults.");
}

// Merge interview + scan data
let effective_name = if project_name != "" && project_name != "my-project" {
    project_name
} else if manifest_name != "" {
    manifest_name
} else {
    "my-project"
};

log("Scan complete. Languages: " + join(languages_detected, ", "));

// ─── Phase 3: Generate ────────────────────────────────────────────────────
phase("Generate");

let gen_effective_name = json_encode(effective_name);
let gen_description = json_encode(description);
let gen_project_type = json_encode(project_type);
let gen_tech_stack = json_encode(tech_stack);
let gen_languages = json_encode(languages_detected);
let gen_manifest_type = json_encode(manifest_type);
let gen_conventions = json_encode(conventions);
let gen_sandbox = json_encode(sandbox_profile);
let gen_has_git = json_encode(has_git);

let gen_prompt = "You are generating a `.grok/` project configuration directory for project "
    + gen_effective_name + ".\n\n"
    + "Based on this data:\n"
    + "- Project description: " + gen_description + "\n"
    + "- Project type: " + gen_project_type + "\n"
    + "- Tech stack: " + gen_tech_stack + "\n"
    + "- Languages detected: " + gen_languages + "\n"
    + "- Manifest type: " + gen_manifest_type + "\n"
    + "- Conventions: " + gen_conventions + "\n"
    + "- Sandbox profile: " + gen_sandbox + "\n"
    + "- Has git: " + gen_has_git + "\n\n"
    + "Create these files in the current project root:\n\n"
    + "### 1. `.grok/AGENTS.md`\n"
    + "YAML frontmatter with name, description, and model preference, followed by structured markdown:\n"
    + "- Overview (tech stack, build system, languages)\n"
    + "- Project structure (key directories and their purpose)\n"
    + "- Commands (build, test, lint)\n"
    + "- Conventions from the interview\n"
    + "- Any project-specific instructions\n\n"
    + "### 2. `.grok/config.toml`\n"
    + "Project-level config with:\n"
    + "- [permission] section (allow Read, Grep, Edit, Write, and build/git commands)\n"
    + "- [mcp_servers] section (project-specific MCP servers if any)\n"
    + "- [plugins] section (project-specific plugins if any)\n\n"
    + "### 3. `.grok/sandbox.toml` (if sandbox_profile is not \"none\")\n"
    + "Sandbox restrictions based on profile choice.\n\n"
    + "Create each file using the Write tool. After creating all files, output a JSON summary:\n"
    + "{\n"
    + "    \"agents_md_created\": true/false,\n"
    + "    \"config_toml_created\": true/false,\n"
    + "    \"sandbox_toml_created\": true/false,\n"
    + "    \"files_created\": [\"...\"]\n"
    + "}\n\n"
    + "Return ONLY the JSON object.";

let gen = agent(
    gen_prompt,
    #{
        label: "init-generator",
        capability_mode: "read-write",
        phase: "Generate",
    },
);

let agents_md_created = false;
let config_toml_created = false;
let sandbox_toml_created = false;
let files_created = [];

if gen.success && gen.output != () {
    let out = gen.output;
    if out.agents_md_created != () { agents_md_created = out.agents_md_created; }
    if out.config_toml_created != () { config_toml_created = out.config_toml_created; }
    if out.sandbox_toml_created != () { sandbox_toml_created = out.sandbox_toml_created; }
    if out.files_created != () { files_created = out.files_created; }
}

log("Generate complete. Files: " + join(files_created, ", "));

// ─── Phase 4: Setup ──────────────────────────────────────────────────────
phase("Setup");

let setup_effective_name = json_encode(effective_name);

let setup_prompt = "You are finalizing project setup for "
    + setup_effective_name + ".\n\n"
    + "Tasks:\n"
    + "1. If a git repo doesn't exist yet and init_git was requested, run: git init\n"
    + "2. Create or update .gitignore to include .grok/ and common patterns\n"
    + "3. If a .git directory exists but no initial commit, stage all files and commit\n"
    + "4. Add .grok/ to .gitignore if not already present\n\n"
    + "Check current state first, then make changes.\n"
    + "After setup, output a JSON summary:\n"
    + "{\n"
    + "    \"git_inited\": true/false,\n"
    + "    \"gitignore_updated\": true/false,\n"
    + "    \"initial_commit_made\": true/false\n"
    + "}\n\n"
    + "Return ONLY the JSON object.";

let setup = agent(
    setup_prompt,
    #{
        label: "init-setup",
        capability_mode: "execute",
        phase: "Setup",
    },
);

let git_inited = false;
let gitignore_updated = false;
let initial_commit_made = false;

if setup.success && setup.output != () {
    let out = setup.output;
    if out.git_inited != () { git_inited = out.git_inited; }
    if out.gitignore_updated != () { gitignore_updated = out.gitignore_updated; }
    if out.initial_commit_made != () { initial_commit_made = out.initial_commit_made; }
}

// ─── Complete ─────────────────────────────────────────────────────────────
let summary = "# Init Guide Complete\n\n";
summary += "## Project: " + effective_name + "\n\n";
summary += "### Interview\n";
summary += "- Description: " + description + "\n";
summary += "- Type: " + project_type + "\n";
if tech_stack.len() > 0 {
    summary += "- Tech stack: " + join(tech_stack, ", ") + "\n";
}
summary += "\n### Scan\n";
summary += "- Languages detected: " + join(languages_detected, ", ") + "\n";
summary += "- Git repo: " + (if has_git || git_inited { "initialized" } else { "not configured" }) + "\n";
summary += "\n### Generated\n";
if agents_md_created {
    summary += "- ✅ `.grok/AGENTS.md` created\n";
}
if config_toml_created {
    summary += "- ✅ `.grok/config.toml` created\n";
}
if sandbox_toml_created {
    summary += "- ✅ `.grok/sandbox.toml` created\n";
}
summary += "\n### Setup\n";
if git_inited {
    summary += "- ✅ Git repo initialized\n";
}
if gitignore_updated {
    summary += "- ✅ `.gitignore` updated\n";
}
if initial_commit_made {
    summary += "- ✅ Initial commit made\n";
}

let path = write_scratch_file("init-summary.md", summary);

complete(#{
    path: path,
    summary: summary,
    project_name: effective_name,
    files_created: files_created,
    git_inited: git_inited || has_git,
});
