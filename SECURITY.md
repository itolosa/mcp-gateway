# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| Latest  | Yes       |

## Reporting a Vulnerability

**Please do not open a public issue for security vulnerabilities.**

Instead, report vulnerabilities through [GitHub Security Advisories](https://github.com/itolosa/mcp-gateway/security/advisories/new). This allows us to discuss and fix the issue privately before public disclosure.

When reporting, please include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

## Response Timeline

- **Acknowledgement**: within 48 hours
- **Initial assessment**: within 1 week
- **Fix or mitigation**: as soon as possible, targeting 30 days for critical issues

## Security Measures

This project enforces:

- `#[forbid(unsafe_code)]` — no unsafe Rust
- Strict clippy lints with `-D warnings`
- 100% test coverage and mutation testing
- Dependency auditing via `cargo-audit` and `cargo-deny` in CI
- Static analysis in CI
