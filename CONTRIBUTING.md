# Contributing to Oxide

Thanks for your interest in contributing.

## Before You Start

- Search existing issues and pull requests before opening a new one.
- For bugs, include reproducible steps and expected vs actual behavior.
- For feature requests, explain the use case and API impact.

## Development Setup

```bash
cargo fmt
cargo test
cargo check --examples
cargo check --no-default-features
cargo check --no-default-features --features openai
cargo check --no-default-features --features anthropic
cargo check --features axum
```

## Pull Request Guidelines

- Keep PRs focused and small when possible.
- Include or update tests for behavior changes.
- Update docs/examples when public APIs or behavior changes.
- Write clear commit messages and PR descriptions.

## Code Style

- Follow Rust formatting via `cargo fmt`.
- Prefer explicit, readable code over clever shortcuts.
- Keep public APIs provider-agnostic and consistent across adapters.

## Review Process

- Maintainers review for correctness, API consistency, and test coverage.
- Feedback may request changes before merge.
- By submitting a contribution, you agree it can be distributed under the MIT license in this repository.
