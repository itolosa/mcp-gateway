# Contributing to MCP Gateway

Thank you for your interest in contributing to MCP Gateway.

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [cargo-nextest](https://nexte.st/) — test runner
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — code coverage
- [cargo-mutants](https://mutants.rs/) — mutation testing
- [cargo-deny](https://embarkstudios.github.io/cargo-deny/) — dependency policy
- [cargo-audit](https://rustsec.org/) — vulnerability auditing

## Development Workflow

```bash
cargo build                    # build
cargo nextest run              # run all tests (2s timeout per test)
cargo fmt --all --check        # check formatting
cargo clippy --all-targets -- -D warnings  # lint (zero warnings)
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 100 --fail-under-functions 100  # coverage
cargo mutants -vV --in-place   # mutation testing
cargo deny check               # dependency policy
cargo audit                    # vulnerability audit
```

## Quality Gates

All of the following must pass before a PR can be merged:

1. **Formatting** — `cargo fmt --all --check`
2. **Lint** — zero clippy warnings (`-D warnings`)
3. **Coverage** — 100% line and function coverage (excluding `main.rs`)
4. **Mutation testing** — all mutants caught
5. **Dependency policy** — `cargo deny check` passes

## Code Style

- Write boring, simple code — no clever tricks
- Use descriptive names that show intent
- Tests follow BDD: `should [behavior] when [context]`
- One execution per test case, sociable unit tests
- Avoid mocks; prefer real collaborators or narrow integration tests

## Pull Requests

1. Fork the repo and create a branch from `main`
2. Make your changes, ensuring all quality gates pass locally
3. Write or update tests for any changed behavior
4. Open a PR with a clear description of what and why

## License

By contributing, you agree that your contributions will be licensed under the [Apache 2.0](LICENSE) license. All contributions must include a Developer Certificate of Origin (DCO) sign-off (`git commit -s`).
