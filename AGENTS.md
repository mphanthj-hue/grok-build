# Grok Build (grok CLI) — Agent Instructions

## Build & run

- **Toolchain**: pinned at `rust-toolchain.toml` (currently 1.92.0, edition 2024)
- **Binary**: `cargo run -p xai-grok-pager-bin` (artifact: `xai-grok-pager`, ships as `grok`).
- **Always target specific crates** with `-p <crate>` — full-workspace builds are very slow.
- **Prerequisites**: `dotslash` on `PATH` (for `bin/protoc` hermetic download); `rustup` auto-installs the pinned toolchain.
- **Release**: `cargo build -p xai-grok-pager-bin --release`; distribution builds use `--profile release-dist` (thin LTO, debug symbols, no strip).

## Development commands

```sh
cargo check -p <crate>           # fast validation
cargo test -p <crate>            # per-crate tests
cargo clippy -p <crate>          # clippy config: clippy.toml at repo root
cargo fmt --all                  # rustfmt.toml at repo root (use_field_init_shorthand)
```

- **Root `Cargo.toml` is auto-generated** — never edit it. Modify per-crate `Cargo.toml` files instead.
- After bumping toolchain version, run `cargo check --all-targets --workspace && cargo clippy --all-targets --workspace`.
- `cargo clippy` uses the **nearest** `clippy.toml` (no merging). The repo-root `clippy.toml` bans `std::fs::canonicalize`, `std::path::Path::canonicalize`, and `tokio::fs::canonicalize` — use `dunce::canonicalize` instead.

## Repository structure

```
crates/
  codegen/   ~55 crates — the CLI closure (pager, shell, tools, workspace, config, MCP, agent, ...)
  common/    ~11 shared leaf crates (tool-runtime, test-utils, tracing, circuit-breaker, ...)
  build/     xai-proto-build (protobuf codegen)
prod/mc/     cli-chat-proxy-types
third_party/ vendored Mermaid diagram stack (dagre_rust, graphlib_rust, mermaid-to-svg, ordered_hashmap)
bin/         hermetically managed tools (protoc via dotslash)
```

Key crates:
- `xai-grok-pager-bin` — composition-root binary
- `xai-grok-pager` — TUI library (scrollback, prompt, modals, rendering)
- `xai-grok-shell` — agent runtime + leader/stdio/headless entry points
- `xai-grok-tools` — tool implementations (terminal, file edit, search, etc.)
- `xai-grok-workspace` — host filesystem, VCS, execution, checkpoints

## Testing

- **Per-crate tests**: `cargo test -p <crate>`.
- Tests use `serial_test`, `tempfile`, `pretty_assertions`, `insta` (snapshot testing), `wiremock`, `mockito`.
- Some crates expose `test-support` features (e.g. `xai-grok-shell-base`, `xai-grok-workspace`, `xai-grok-memory`).
- `xai-grok-shell` uses `tokio::test(start_paused = true)` for deterministic timer tests.
- Benchmarks use `criterion` (e.g. `xai-grok-shell` has `session_list` bench).

## Key conventions & quirks

- **No external contributions accepted** — see `CONTRIBUTING.md`.
- **`SOURCE_REV`** at root records the monorepo commit SHA this tree was synced from.
- **Profile `release-dist`** is the hardened release profile (thin LTO, codegen-units=1, debug symbols). `release-dist-jemalloc` is an alias (used by desktop workflow).
- **jemalloc** is the default allocator on Unix (gated by the `jemalloc` feature in `xai-grok-pager-bin`).
- **Panic = abort** in both `dev` and `release` profiles.
- **No GitHub Actions CI** in this tree — CI is internal to the monorepo.
- **Binary hardening**: `.cargo/config.toml` applies per-target rustflags (force-unwind-tables, RELRO/NX on musl, macOS link-args).
- **Proto codegen**: `bin/protoc` (dotslash) resolved at build time via `xai-proto-build`.
- **macOS/Linux** are supported build hosts; Windows builds are best-effort.
- **`skills/`** directory exists but is currently empty.