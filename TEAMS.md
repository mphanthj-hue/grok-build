# Team Structure

## Grok Build Team

### Core Engineering
- **CLI & Agent Runtime** — `crates/codegen/` (pager, shell, tools, workspace, config, MCP, agent)
- **Shared Infrastructure** — `crates/common/` (tool-runtime, test-utils, tracing, circuit-breaker)
- **Protobuf Codegen** — `crates/build/` (xai-proto-build)

### Desktop & Distribution
- **Desktop Build** — `xai-grok-pager-bin` composition root, release profiles, binary hardening
- **Third-party Vendoring** — `third_party/` (Mermaid diagram stack)

### Platform Support
- **Linux** — primary target (musl, jemalloc, RELRO/NX)
- **macOS** — secondary target
- **Windows** — best-effort

### Security
- Security reports: HackerOne program at https://hackerone.com/x
- Contact: security@spacexai.com (internal)

### On-call Rotation
- Primary: #grok-build-oncall (Slack)
- Escalation: #grok-eng (Slack)

---

*This document is maintained by the Grok Build team.*
