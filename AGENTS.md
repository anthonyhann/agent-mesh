# AGENTS.md — 给 AI Agent 的项目指南

> 你正在进入 **agent-mesh** 仓库：一个把多台 Agent CLI 编排成协作网络的调度层。
> 本文件是你使用本项目的最短路径。

## 一分钟上手

```bash
./scripts/setup.sh          # 环境自检（依赖/CLI 可用性）
./run_worker.sh examples/tasks/002-ping-dsh.json    # 派一个真实任务验证链路
```

## 你的典型用法（如果你是调用方 Agent）

1. **派单**：写任务单 JSON → `./run_worker.sh <task.json>` → 读 `results/<id>/result.json`（status/answer/tokens/session_id）
2. **多步任务**：写 pipeline JSON（`{{prev_output}}` 传递上游正文）→ `./run_pipeline.sh`
3. **并行任务**：写 DAG JSON（`depends_on` + `{{nodes.N.output}}`）→ `python3 run_dag.py`
4. **需要记忆的多轮**：第一轮跑完取 `result.json.session_id`，第二轮任务单带上 `session_id`

## 纪律（违反会产出坏结果）

- **按任务类型规则路由 worker**，不要用你的判断现场"选模型"：重型/浏览器→dsh，编码/review→codex，多模型→opencode，轻量→hermes，豆包系/结构化输出→arkcli，claude 需 API key
- **done_criteria 必须机器可判**（stdout_regex），禁止"看起来对"
- brief 里**只传必要信息**；节点间上下文经 `answer.txt` 归一化，超长自动转文件引用
- 看到 `criteria_failed` 说明答案不对不是执行错——重试无意义，改 brief

## 验证你的改动

```bash
./smoke.sh --quick   # 必过（语法+单元）
./smoke.sh           # 涉及 worker 行为时跑全量
./mem_test.sh        # 改了会话逻辑时跑
```

## 仓库地图

内核脚本在根目录；任务单样例在 `examples/tasks/`；真实应用案例在 `examples/skills-governance/`；接入新 worker 指南在 `docs/WORKERS.md`；架构决策在 `docs/ARCHITECTURE.md`。**任务单 schema 的唯一事实源是 `run_worker.sh` 头部注释**。
