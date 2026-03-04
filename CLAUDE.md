# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MCP Gateway is a Rust-based proxy/firewall for Model Context Protocol (MCP) servers. It sits between a sandboxed Claude instance (in Docker) and upstream MCP servers, exposing only an allowed subset of tools for read-only or controlled access. It can also expose host CLI tools (e.g., `gh`) as MCP calls.

## Build & Test Commands

```bash
cargo build                    # build
cargo nextest run              # run all tests (with 2s timeout per test)
cargo nextest run -E 'test(name)'  # run a single test
cargo fmt --all --check        # check formatting
cargo clippy --all-targets -- -D warnings  # lint (zero warnings enforced)
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 100 --fail-under-functions 100  # coverage (100% required, main.rs excluded)
cargo mutants -vV --in-place   # mutation testing
```

Build target is `/tmp/mcp-gateway-target` (set in `.cargo/config.toml`).

## Quality Gates (non-negotiable)

1. **Format**: `cargo fmt --all --check` must pass
2. **Lint**: zero clippy warnings (`-D warnings`)
3. **Coverage**: 100% line and function coverage (excluding `main.rs`)
4. **Mutation testing**: all mutants must be caught; config in `.cargo/mutants.toml` (excludes `main.rs`). **Never skip or exclude mutants** — if a mutant survives, fix the code or tests to kill it properly
5. **Tests follow BDD**: SUT executed once per test case, sociable unit tests + narrow integration tests (mock upstream)
6. **Test timeouts**: Enforced globally via `cargo-nextest` config in `.config/nextest.toml`. Tests exceeding 2s are flagged slow; tests exceeding 10s are killed. No test may hang. If a test is too slow, fix the code, not the timeout.

## Architecture Principles

- **Ports & Adapters** (hexagonal): core domain has no external dependencies
- **Vertical Slicing**: each feature is an independent slice
- **Compile-time DI**: use generics and trait bounds, no `dyn Trait` / vtable overhead
- **Strategy pattern via generics**: differentiation through composition, not inheritance
- **Low coupling over high cohesion** initially; tighten cohesion as the project grows
- **OCP**: open for extension, closed for modification
- **Minimal code**: nothing can be removed without breaking the system

## Development Motto

**RTFM** - Search the docs first. Always verify against official documentation for the version we are using. We use latest stable versions and update frequently.

**Boy Scout Rule** - Leave the place better than we found it. No leftover files, no garbage. If something is generated/temporary, either `.gitignore` it or delete it. Clean as you go.

**Don't reinvent the wheel** - Use established libraries for complex protocols (OAuth, HTTP, JSON-RPC). Focus our code on the gateway-specific logic, not reimplementing standards.

**Problems are OUR problems** - Never blame external tools, "pre-existing" issues, other people's code, or CI infrastructure. If something is broken, it's our responsibility to fix it. No excuses, no dismissing failures. We own the entire state of the project — code, tests, CI, tooling — and we leave everything green.

## Workflow Rules

- **Task loop**: Always check `prd.json` for the highest-priority unblocked pending milestone. Implement it, update `progress.txt` with the completion note, mark the milestone as `"completed"` in `prd.json`, commit, then repeat with the next milestone.
- **Commit after every win**: Always commit after each significant progress (milestone complete, quality gates passing, major refactor done). Small frequent commits let us roll back safely.
- **Quality gates before commit**: Always run and pass ALL quality gates (fmt, clippy, 100% coverage, 100% mutation coverage) before committing a completed milestone. Never commit with failing gates. This means the FULL suite — not just the files you changed. No shortcuts, no scoping to individual files.
- **Every failure is our failure**: Never dismiss a failing test, surviving mutant, or CI failure as "pre-existing" or "not related to our changes." If it's broken, we own it and fix it before committing.
- Spawn multiple agents in parallel for wide research tasks
- Incremental approach: build by milestones, verify each before moving on
- All config files are JSON
- Never relax quality guardrails
- Use expressive names; code should be boring and easy to understand
- Use generic, reusable, composable building blocks; express intent through composition

## MCP Protocol Reference

- Transport: stdio (local), SSE, HTTP Streamable
- Wire format: JSON-RPC 2.0
- Key methods: `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`
- Config format follows `mcpServers` structure from Claude Code (`.claude.json` style)

## CI/CD

- **CI** (`.github/workflows/ci.yml`): fmt, clippy, build, coverage on push/PR to main; updates coverage badge SVG on main pushes
- **Mutation** (`.github/workflows/mutants.yml`): cargo-mutants with dynamic sharding (~30 mutants/shard), mold linker, cargo-nextest; updates mutation badge SVG on main pushes when all shards pass
- **Release** (`.github/workflows/release.yml`): multi-target builds on push to main (linux-gnu, linux-musl, macOS x86/arm) with CalVer tags
- **Badges**: SVGs generated by `action-badges/core` and pushed to orphan `badges` branch using `GITHUB_TOKEN` — no external services, no PATs
