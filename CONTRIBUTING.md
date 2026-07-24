# Contributing

This repository does **not** accept external pull requests or unsolicited
patches.

SpaceXAI develops this software internally. The public tree is published for
source transparency and local builds under the terms of the Apache License,
Version 2.0 (see [`LICENSE`](LICENSE)).

## Internal Development Workflow

### Code Style
- Follow the formatting rules defined in `rustfmt.toml`
- Run `cargo fmt --all` before committing
- Follow clippy linting rules defined in `clippy.toml`
- Run `cargo clippy --all-targets --workspace` before committing

### Development Commands
```bash
# Format code
cargo fmt --all

# Check code
cargo check --all-targets --workspace

# Run linter
cargo clippy --all-targets --workspace

# Run tests
cargo test --workspace

# Build release
cargo build --release --workspace
```

### Commit Guidelines
- Use clear, descriptive commit messages
- Reference relevant issue numbers when applicable
- Keep commits atomic and focused

## Security reports

Please report security issues through the process described in
[`SECURITY.md`](SECURITY.md). Do not open a public issue for vulnerabilities.

## Licensing of this source

By downloading or using this source, you agree that your use is governed by
the Apache License, Version 2.0. No contributor license agreement is offered
because external contributions are not accepted.
