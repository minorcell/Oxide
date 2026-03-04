# 11 Release Plan

## 1. 目标与职责

- 定义 crates.io 可发布标准与发布流水线门禁。
- 固化 feature matrix、semver 策略、文档与示例要求。
- 确保发布产物可被外部项目直接消费且行为稳定。

## 2. Public API/类型签名（最终形态）

发布约束的公开 API（semver 保护）：

- `AiClient` / `AiClientBuilder`
- `generate_text` / `stream_text` / `run_tools`
- `ProviderKind` / `ModelRef` / `Message` / `StreamEvent`
- `ToolDescriptor` / `Tool` / `ToolExecutor` / `ToolRegistry`
- `AiError` / `AiErrorCode`

`Cargo.toml` 目标结构：

```toml
[package]
name = "oxide-ai-sdk"
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
description = "Rust AI SDK with unified OpenAI/Anthropic APIs"
repository = "https://github.com/<org>/oxide-ai-sdk"
readme = "README.md"

[features]
default = ["openai", "anthropic"]
openai = []
anthropic = []
axum = ["dep:axum"]
```

## 3. 输入输出与数据流

输入：

- 代码与文档、测试结果、示例程序。
  输出：
- 可发布 crate 包、release notes、版本标签。

发布数据流：

1. 本地/CI 执行格式与测试门禁。
2. 运行 `cargo package --allow-dirty`（仅本地验证，不用于正式发布）。
3. 验证 README 示例可编译。
4. 生成 changelog/notes。
5. 执行 `cargo publish`。

## 4. 核心算法/状态机（含伪代码）

发布状态机：

```text
Preflight -> TestPass -> PackageCheck -> Publish -> PostVerify
```

伪代码：

```text
if !docs_complete or !tests_green:
    block_release
if public_api_changed and version_not_bumped:
    block_release
run cargo package
run cargo publish
verify crate install + smoke test
```

版本策略：

- `0.y.z` 阶段遵循“尽量 semver”。
- 公开类型或方法签名变更视为 breaking，至少提升 `y`。
- 非破坏新增能力提升 `z`。

## 5. 边界条件与失败模式

- 边界：
- 首发版本不包含 CLI 二进制。
- `axum` 为可选 feature，不得影响默认编译。
- 失败模式：
- feature 组合编译失败（默认/最小/axum）。
- README 示例过时导致用户无法快速起步。
- 公开 API 隐式变更未记录。

## 6. 错误码与错误映射

发布阶段错误（流程层）：

- `ReleaseBlocked::TestsFailed`：测试未通过。
- `ReleaseBlocked::FeatureMatrixFailed`：feature 组合失败。
- `ReleaseBlocked::SemverViolation`：版本号与 API 变更不匹配。
- `ReleaseBlocked::PackageValidationFailed`：`cargo package` 失败。

注：以上为发布流程错误，不替代运行时 `AiErrorCode`。

## 7. 测试用例列表（成功/失败/边界）

- 成功：
- 默认 feature 下 `cargo test` 通过。
- `--no-default-features --features openai` 可编译。
- `--no-default-features --features anthropic` 可编译。
- `--features axum` 可编译并通过相关测试。
- 失败：
- 破坏性 API 变更但版本未提升时被门禁阻断。
- README 示例编译失败时阻断发布。
- 边界：
- crates.io 元信息缺失（license/readme/repository）时阻断。
- 文档中的默认值与实现不一致时阻断（通过审查脚本或人工检查）。

## 8. 与其他模块的依赖契约

- 发布对象必须实现 `03-client-api.md` 定义的 API。
- 工具相关公开契约必须符合 `12-tool-definition.md`。
- 错误语义必须符合 `08-error-handling.md`。
- 测试门禁与用例来源于 `10-testing-plan.md`。
- 可选 feature 行为必须符合 `09-axum-adapter.md`。

## 9. 非目标与后续扩展点

- 非目标：
- 当前不做 nightly-only 功能发布。
- 当前不集成自动化多平台性能基准流水线。
- 扩展点：
- 增加 `cargo-semver-checks` 门禁。
- 增加自动生成 API diff 与 release note 模板。
