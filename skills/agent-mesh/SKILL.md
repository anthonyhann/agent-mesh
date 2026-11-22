---
name: agent-mesh
description: Multi-agent CLI orchestration. Use when dispatching tasks to multiple AI CLI workers (dsh/codex/opencode/hermes/arkcli/claude), building pipelines or parallel DAGs across agents, verifying outputs with machine-checkable criteria, or reviewing run history/flow diagrams. Triggers: 派单, dispatcher, run_worker, agent mesh, multi-agent, 多智能体协作, /agent-mesh.
---

# agent-mesh — 多 Agent CLI 编排

本 skill 教你使用 agent-mesh 编排层（仓库脚本自定位，直接在仓库目录下调用）。

## 快速判断

- 单任务 → `run_worker.sh <task.json>`
- 串行多步（A 产出喂 B）→ `run_pipeline.sh <pipeline.json>`（brief 里用 `{{prev_output}}` 引用上游）
- 并行/分支汇聚 → `python3 run_dag.py <dag.json>`（`depends_on`、`{{nodes.N.output}}`）

## 任务单最小形态

```json
{"id": "my-task", "worker": "dsh", "brief": "任务描述",
 "done_criteria": {"stdout_regex": "验收正则"}}
```

完整 schema 见仓库 `run_worker.sh` 头部注释（唯一事实源）。

## 工人路由（规则，勿即兴）

| 任务类型 | worker |
|---|---|
| 重型 agent / 需要浏览器 | dsh |
| 编码 / 代码 review | codex |
| 需要指定模型/provider | opencode |
| 轻量问答 / 快速任务 | hermes |
| 豆包系 / json_schema 强结构化输出 / 多模态生成 | arkcli |
| 需官方 Claude 能力（已配 ANTHROPIC_API_KEY） | claude |

## 结果读取

`results/<id>/result.json`：status（success/failed/timeout/criteria_failed）、answer_preview、tokens、session_id（续接下轮用）、cost_usd（claude）。
全部运行流水在 `results/ledger.jsonl`；看板：`python3 mesh_board.py`；流转图：pipeline/DAG 跑完自动生成 `flow.html`。

## 纪律

- 验收必须机器可判；criteria_failed = 答案不对（改 brief，别重试）
- 双引擎复核是默认质量门：重要产出派第二家复核
- 工人缺凭证会秒级明确失败，不会挂起
