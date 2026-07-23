let meta = #{
    name: "init-guide",
    description: "Interactive project initialization — interview, scan, generate `.grok/`, and setup git",
    phases: [
        #{ title: "Init Guide", detail: "Interview, scan, generate `.grok/`, and setup git" },
    ],
};

// ─── Single phase: one agent does everything ──────────────────────────────
phase("Init Guide");

let prompt = `You are running an interactive project initialization.
Your task is to do the following, IN ORDER:

## 1. Interview (using ask_user_question tool, ONE question at a time)
- Project name and short description
- Project type (app, library, CLI, web service, game, etc.)
- Primary tech stack (languages, frameworks, databases)
- Does the user want git initialized? (if not already in a repo)
- Any specific coding conventions or preferences?
- Any project-specific MCP servers or tools?
- Sandbox profile preference (none / restrictive / isolated)?

## 2. Scan (using run_terminal_command)
- List top-level directory
- Read root manifest files (Cargo.toml, package.json, etc.)
- Check if .git exists
- Check if .grok/ already exists
- Detect languages and frameworks

## 3. Generate .grok/ files (using Write tool)
Create these files:

### .grok/AGENTS.md
YAML frontmatter (name, description, model) followed by structured markdown:
- Overview (tech stack, languages, build system)
- Project structure
- Commands (build, test, lint)
- Conventions from interview
- Project-specific instructions

### .grok/config.toml
- [permission] section (Read, Grep, Edit, Write, plus build/git commands)
- [mcp_servers] section (if any from interview)
- [plugins] section (if any)

### .grok/sandbox.toml (if sandbox profile is not "none")
- Sandbox restrictions based on preference

## 4. Setup git (using run_terminal_command)
- git init if needed and user wants it
- Add .grok/ to .gitignore
- Stage and make initial commit if initialized

## 5. Final report
Tell the user what was created and any next steps.
`;

let result = agent(
    prompt,
    #{
        label: "init-guide",
        capability_mode: "all",
        phase: "Init Guide",
    },
);

// ─── Complete ─────────────────────────────────────────────────────────────
let summary = if result.success {
    "# Init Guide Complete\n\nWorkflow finished successfully. `.grok/` has been set up for this project."
} else {
    "# Init Guide Failed\n\nThe workflow encountered an issue. Check the output above for details."
};

let path = write_scratch_file("init-summary.md", summary);

complete(#{
    path: path,
    summary: summary,
    success: result.success,
});
