# Repository Guidelines

## Project Structure & Module Organization

- `src/`: Rust library code. Keep shared logic in core modules and provider mappings in `src/model_adapters/*`.
- `tests/`: integration tests by behavior (`generate`, `stream`, `tools`, `providers`, `agent`, `error_mapping`).
- `examples/`: runnable reference programs.
- `ai-sdk/`: separate TypeScript/pnpm workspace.

## Build, Test, and Development Commands

- `cargo fmt`: format Rust code with standard style.
- `cargo test`: run the full Rust test suite.
- `cargo check --examples`: verify all examples compile.
- `cargo check --no-default-features`: validate the minimal feature set.
- `cargo check --no-default-features --features openai` (or `anthropic`): verify provider-specific paths.
- `cargo check --features axum`: validate optional SSE integration.
- `cargo run --example basic_stream`: run a local example.

For `ai-sdk/` work only:

- `cd ai-sdk && pnpm build && pnpm lint && pnpm type-check`

## Coding Style & Naming Conventions

- Rust edition is `2024`; follow `rustfmt` defaults (4-space indentation, trailing commas where applicable).
- Use `snake_case` for files/modules/functions, `PascalCase` for structs/enums/traits, and `SCREAMING_SNAKE_CASE` for constants.
- Keep provider-neutral behavior in shared layers and isolate provider-specific mapping in adapters.
- Prefer small, focused functions and explicit error mapping to `AiErrorCode`.

## Documentation Standards

- Keep docs aligned with code changes in the same PR.
- Update `README.md`, `README_CN.md`, `examples/README.md`, or `docs/` whenever APIs or behavior changes.
- Prefer short, task-oriented text with runnable commands and required environment variables.
- Add migration notes for breaking changes.

## Open-Source Commenting Norms

- Write comments and docstrings in clear English for global contributor readability.
- Explain `why` and constraints, not obvious `what`.
- Add Rust doc comments (`///`) for public APIs; include usage notes when non-trivial.
- Fix stale comments in the same PR.

## Testing Guidelines

- Use `#[tokio::test]` for async flows and `wiremock` to simulate provider APIs.
- Add integration tests for any behavior change; place them in the closest domain file.
- Integration tests should be comprehensive: cover happy path, error mapping (`401/429/5xx`), and multi-step tool or stream flows when relevant.
- Use descriptive names such as `openai_401_maps_to_auth_failed`.
- No strict coverage threshold is enforced, but feature/bug PRs should include regression tests.

## Commit & Pull Request Guidelines

- Follow Conventional Commit style (`feat: ...`, `docs: ...`, `fix: ...`).
- Keep commits and PRs scoped to one logical change.
- PRs should include purpose, behavior changes, linked issue(s), and test evidence.
- Update `README.md` and/or `examples/` when public API behavior changes.
