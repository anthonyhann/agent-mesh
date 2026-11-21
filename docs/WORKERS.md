# 接入新 Worker / Adding a Worker

agent-mesh 的 worker 适配只有一条规则：**一个 CLI 的非交互调用 = 一个 case 分支**。

## 三步接入

### 1. `run_worker.sh` 加分支

```zsh
myworker)
  CMD=(myworker run "$T_BRIEF")                    # 非交互入口
  [[ -n "$T_MODEL" ]]   && CMD+=(--model "$T_MODEL")
  [[ -n "$T_SESSION" ]] && CMD+=(--resume "$T_SESSION")   # 若有会话能力
  ;;
```

并在 `*) die` 的清单里登记名字。

### 2. 输出契约检查（验收段自动适配）

- 纯文本 stdout：零适配
- JSON 信封：若正文在 `content` 或 `result` 字段，自动识别；其他字段名在 `content_aware_target` 加一行映射
- session_id：信封字段直接映射；无信封时在"非信封提取段"加你的提取方式（文件扫描 / list 命令）

### 3. tokens 采集（可选但推荐）

在验收段的 `extract` 区加你的来源（响应 usage / stderr 统计行 / 会话库导出）。没有就留 `null`，看板自动跳过。

## 验收清单

- [ ] `./smoke.sh` 全量通过（把你的 ping 任务单加到 `examples/tasks/`）
- [ ] `./mem_test.sh` 两轮暗号复述通过（验证会话续接真实生效）
- [ ] 失败时不挂起：无凭证/无登录态时秒级明确报错（参考 claude 的凭据门控）
- [ ] README 工人矩阵更新
