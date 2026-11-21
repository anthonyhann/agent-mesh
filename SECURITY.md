# Security Policy / 安全政策

## 报告漏洞 / Reporting a Vulnerability

请**不要**在公开 Issue 中报告安全漏洞。
Please **do not** report security vulnerabilities in public Issues.

联系方式 / Contact：开 GitHub Security Advisory（仓库 Security 标签页 → Report a vulnerability），或在 Issue 中仅说明"有安全问题需私聊"并留下联系方式。

响应承诺 / SLA：72 小时内确认，14 天内给出修复或缓解方案。

## 安全设计基线 / Baseline

- 发布管道默认脱敏（`redact.py`：绝对路径/用户名不出现在公开产物中）
- 凭证永不入库：`claude.env` 等在 `.gitignore`；任务单与 ledger 不应包含密钥
- 所有 worker 调用为只读优先；迁移类脚本（examples/skills-governance）默认 dry-run 且先备份
- 破坏性清理走系统废纸篓，不使用不可恢复的强制删除
