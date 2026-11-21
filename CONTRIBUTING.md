# Contributing to agent-mesh / 参与贡献

感谢你的兴趣！Thanks for your interest!

## 中文

### 我能贡献什么
- **新 worker 适配**：`run_worker.sh` 里加一个 case 分支（见 docs/WORKERS.md）
- **bug 修复**：带复现步骤的 Issue 最受欢迎
- **文档与翻译**：README/文档勘误
- **应用示例**：`examples/` 下分享你的真实编排场景

### 流程
1. Fork 并新建分支：`feat/<topic>` 或 `fix/<topic>`
2. 改动后必须通过：`./smoke.sh --quick`（语法+单元）；涉及 worker 行为的改动请跑 `./smoke.sh` 全量
3. 提交 PR：描述动机、改动、验证方式；一个 PR 只做一件事
4. commit message：英文或中文均可，建议 `<type>: <summary>`（feat/fix/docs/refactor/test）

### 约定
- 任务单 schema 的权威定义在 `run_worker.sh` 头部注释——改字段必须同步注释与 README
- 不引入新依赖（zsh + python3 标准库 + Pillow 为限）；不收集任何遥测
- 凭证文件（`claude.env` 等）已被 gitignore，请勿提交任何 key/token

## English

### What to contribute
- **New worker adapters**: one `case` branch in `run_worker.sh` (see docs/WORKERS.md)
- **Bug fixes**: Issues with reproduction steps are most welcome
- **Docs & translations**
- **Use-case examples** under `examples/`

### Process
1. Fork and branch: `feat/<topic>` or `fix/<topic>`
2. Must pass `./smoke.sh --quick`; run full `./smoke.sh` for worker-behavior changes
3. PR with motivation, change, and verification; one PR does one thing
4. Commit messages: `<type>: <summary>`

### Conventions
- The task-card schema's single source of truth is the header comment of `run_worker.sh`
- No new dependencies (zsh + python3 stdlib + Pillow only); no telemetry
- Never commit credentials; `claude.env` is gitignored
